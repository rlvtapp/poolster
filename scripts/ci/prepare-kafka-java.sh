#!/usr/bin/env bash
set -euo pipefail
work="$RUNNER_TEMP/kafka-java-injector"
mkdir -p "$work"
cat > "$work/pom.xml" <<'XML'
<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>poolster.test</groupId><artifactId>wire-injector</artifactId><version>1</version><dependencies><dependency><groupId>org.apache.kafka</groupId><artifactId>kafka-clients</artifactId><version>3.9.1</version></dependency><dependency><groupId>org.slf4j</groupId><artifactId>slf4j-simple</artifactId><version>2.0.16</version></dependency></dependencies></project>
XML
mvn --batch-mode --no-transfer-progress -q -f "$work/pom.xml" dependency:build-classpath -Dmdep.outputFile=classpath.txt
printf 'POOLSTER_JAVA=%s\n' "$(command -v java)" >> "$GITHUB_ENV"
printf 'POOLSTER_KAFKA_JAVA_CLASSPATH=%s\n' "$(cat "$work/classpath.txt")" >> "$GITHUB_ENV"
