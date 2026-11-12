#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
php_binary="${PHP:-php}"
composer_binary="$(command -v composer)"

"$php_binary" "$composer_binary" install --no-dev --prefer-dist --classmap-authoritative --no-interaction
"$php_binary" optimize.php
