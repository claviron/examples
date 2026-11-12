#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
: "${ORBIT_JAVA_CLASSPATH:?Set ORBIT_JAVA_CLASSPATH to the prepared Orbit Java API JAR and optional native classifier JAR}"
javac="${JAVA_HOME:+$JAVA_HOME/bin/}javac"
jar="${JAVA_HOME:+$JAVA_HOME/bin/}jar"
mkdir -p build/classes
sources=()
while IFS= read -r -d '' source; do sources+=("$source"); done < <(find src/main/java -name '*.java' -print0)
"$javac" --release 25 -Xlint:all -Werror -cp "$ORBIT_JAVA_CLASSPATH" -d build/classes "${sources[@]}"
"$jar" --create --file build/java-link.jar --main-class example.javalink.Main \
  -C build/classes . -C src/main/resources .
