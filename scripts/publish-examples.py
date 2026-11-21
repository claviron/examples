#!/usr/bin/env python3
"""Assemble committed example trees and optionally publish the combined snapshot."""

import argparse
import base64
import json
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import tomllib

GITHUB = "https://github.com/claviron/examples.git"
EXAMPLES = (
    "axum-link", "c-link", "c-module", "demo-certs", "edge-server",
    "fastapi-link", "go-link", "java-link", "js-link", "octane-link",
    "pentaglot", "symfony-link", "uv-link",
)
REPOS = ("shared", *EXAMPLES)


def git(directory, *args):
    return subprocess.check_output(
        ["git", "-C", str(directory), *args], text=True
    ).strip()


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="published Claviron version for this snapshot")
    parser.add_argument(
        "--ref", metavar="REPO=REF", action="append", default=[],
        help="select a different commit for a source repo (default: origin/main)",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--dry-run", action="store_true", help="prepare an archive only (default)")
    mode.add_argument("--push", action="store_true", help="publish one commit to GitHub main")
    args = parser.parse_args()
    args.refs = dict.fromkeys(REPOS, "origin/main")
    selected = set()
    for entry in args.ref:
        repo, _, ref = entry.partition("=")
        if repo not in args.refs or not ref or repo in selected:
            parser.error(f"expected one REPO=REF override for a known repo: {entry!r}")
        args.refs[repo] = ref
        selected.add(repo)
    return args


def export(examples, tree, refs):
    revisions = {}
    for name, ref in refs.items():
        repo = examples / name
        if not (repo / ".git").exists():
            raise ValueError(f"missing source checkout: {repo}")
        commit = git(repo, "rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}")
        revisions[name] = commit
        entries = git(repo, "ls-tree", "-r", commit).splitlines()
        if any(line.startswith("160000 ") for line in entries):
            raise ValueError(f"{name} contains a Git submodule")
        prefix = "" if name == "shared" else f"{name}/"
        with tempfile.TemporaryFile() as archive:
            subprocess.run(
                ["git", "-C", str(repo), "archive", "--format=tar", f"--prefix={prefix}", commit],
                stdout=archive, check=True,
            )
            archive.seek(0)
            with tarfile.open(fileobj=archive) as source:
                source.extractall(tree, filter="data")
    return revisions


def prepare_layout(tree):
    workspace = tree / "examples.code-workspace"
    source = workspace.read_text()
    data = json.loads(source)
    for folder in data["folders"]:
        path = folder["path"]
        name = path.removeprefix("../")
        if name not in EXAMPLES:
            raise ValueError(f"invalid workspace folder: {path}")
        if name != path:
            source = source.replace(json.dumps(path), json.dumps(name))
    workspace.write_text(source)

    env = tree / "octane-link" / ".env"
    if env.is_file():
        # This value is public and only makes the demo runnable out of the box.
        demo_key = base64.b64encode(b"claviron-example-not-a-secret!!!").decode()
        env.write_text(re.sub(r"(?m)^APP_KEY=.*$", f"APP_KEY=base64:{demo_key}", env.read_text()))


def validate(tree, version):
    for path in tree.rglob("*"):
        if path.is_symlink() and (
            not path.resolve().is_relative_to(tree) or not path.exists()
        ):
            raise ValueError(f"broken or external symlink: {path.relative_to(tree)}")
        if any(part in {".git", ".idea", ".local", ".private", "AGENTS.md"} for part in path.relative_to(tree).parts):
            raise ValueError(f"local-only file: {path.relative_to(tree)}")
        if ".local." in path.name or path.suffix in {".pem", ".key"}:
            raise ValueError(f"local-only file: {path.relative_to(tree)}")

    def check_cargo(value, manifest):
        if isinstance(value, dict):
            if "git" in value and isinstance(value["git"], str):
                raise ValueError(f"Git dependency in {manifest.relative_to(tree)}")
            if isinstance(path := value.get("path"), str):
                target = (manifest.parent / path).resolve()
                if Path(path).is_absolute() or not target.is_relative_to(tree) or not target.exists():
                    raise ValueError(f"external or missing path in {manifest.relative_to(tree)}: {path}")
            for section, nested in value.items():
                if section.endswith("dependencies") and isinstance(nested, dict):
                    for name, dependency in nested.items():
                        if isinstance(dependency, str):
                            requirement = dependency
                        else:
                            name = dependency.get("package", name)
                            requirement = dependency.get("version")
                        if name == "claviron" and requirement and requirement.lstrip("=^~") != version:
                            raise ValueError(
                                f"{manifest.relative_to(tree)} selects Claviron {requirement}; expected {version}"
                            )
                check_cargo(nested, manifest)
        elif isinstance(value, list):
            for nested in value:
                check_cargo(nested, manifest)

    manifests = list(tree.rglob("Cargo.toml"))
    for manifest in manifests:
        check_cargo(tomllib.loads(manifest.read_text()), manifest)
    for manifest in tree.rglob(".cargo/config.toml"):
        check_cargo(tomllib.loads(manifest.read_text()).get("patch", {}), manifest)

    for manifest in tree.rglob("package.json"):
        data = json.loads(manifest.read_text())
        for section in ("dependencies", "devDependencies", "optionalDependencies", "peerDependencies"):
            for requirement in data.get(section, {}).values():
                if requirement.startswith(("file:", "link:", "workspace:", ".", "/", "git", "http:", "https:")):
                    raise ValueError(f"non-registry dependency in {manifest.relative_to(tree)}")
    for manifest in tree.rglob("composer.json"):
        repositories = json.loads(manifest.read_text()).get("repositories", [])
        if isinstance(repositories, dict):
            repositories = repositories.values()
        if any(isinstance(repo, dict) and repo.get("type") in {"path", "vcs", "git"} for repo in repositories):
            raise ValueError(f"non-registry repository in {manifest.relative_to(tree)}")
    for manifest in tree.rglob("go.mod"):
        if re.search(r"(?m)^\s*(?:replace\s+)?[^\n]*=>\s*(?:\.|/)", manifest.read_text()):
            raise ValueError(f"local replacement in {manifest.relative_to(tree)}")
    print(f"Validated snapshot dependencies, including {len(manifests)} Cargo manifests")


def publish(tree, version):
    git(tree, "remote", "add", "github", GITHUB)
    ref = "refs/heads/main"
    remote = git(tree, "ls-remote", "--refs", "github", ref)
    parent = []
    if remote:
        git(tree, "fetch", "--no-tags", "github", ref)
        parent = ["-p", "FETCH_HEAD"]

    snapshot = git(tree, "write-tree")
    if parent and snapshot == git(tree, "rev-parse", "FETCH_HEAD^{tree}"):
        print("GitHub main already has this snapshot.")
        return
    commit = git(tree, "commit-tree", snapshot, *parent, "-m", f"Publish Claviron {version} examples")
    git(tree, "push", "github", f"{commit}:{ref}")
    published = git(tree, "ls-remote", "--refs", "github", ref).split()
    if not published or published[0] != commit:
        raise ValueError("GitHub main did not resolve to the published commit")
    print(f"Published {commit} to GitHub main")


def main():
    args = arguments()
    examples = Path(__file__).resolve().parents[2]
    output = Path(tempfile.mkdtemp(prefix="claviron-examples-")).resolve()
    print(f"Output: {output}", flush=True)
    tree = output / "tree"
    tree.mkdir()
    revisions = export(examples, tree, args.refs)
    (output / "commits.txt").write_text(
        "".join(f"{name} {commit}\n" for name, commit in revisions.items())
    )
    prepare_layout(tree)
    validate(tree, args.version)
    files = sum(path.is_file() or path.is_symlink() for path in tree.rglob("*"))

    with tarfile.open(output / "claviron-examples.tar.gz", "w:gz") as archive:
        archive.add(tree, arcname=".")

    # Stage in the export only, honoring the source ignore rules.
    git(tree, "init", "--quiet", "--initial-branch=main")
    git(tree, "add", "--", ".")
    if git(tree, "ls-files", "-z").count("\0") != files:
        raise ValueError("Git ignore rules exclude files in the prepared archive")
    if args.push:
        publish(tree, args.version)
    else:
        print(f"Dry run: {len(EXAMPLES)} examples and {files} files")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, tarfile.TarError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"Examples publication failed: {error}") from error
