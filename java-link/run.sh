#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
: "${ORBIT_JAVA_CLASSPATH:?Set ORBIT_JAVA_CLASSPATH to the prepared Orbit Java API JAR and optional native classifier JAR}"
java="${JAVA_HOME:+$JAVA_HOME/bin/}java"
exec "$java" --enable-native-access=ALL-UNNAMED --illegal-native-access=deny \
  -cp "build/java-link.jar:$ORBIT_JAVA_CLASSPATH" example.javalink.Main "$@"
