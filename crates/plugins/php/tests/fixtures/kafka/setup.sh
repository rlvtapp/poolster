#!/usr/bin/env bash
# Reproducible, task-owned native PHP/Symfony Kafka test environment.
set -euo pipefail

php_container="${POOLSTER_KAFKA_PHP_CONTAINER:-poolster-events-php}"
broker_container="${POOLSTER_KAFKA_BROKER_CONTAINER:-poolster-alpha2-kafka}"
php_image='php:8.2-cli@sha256:185eb902246c5757dbbb7e439bf92dfea32f627a401b1e2785bee0770fe0c41b'
composer_sha='f446ea719708bb85fcbf4ef18def5d0515f1f9b4d703f6d820c9c1656e10a2f2'

docker inspect "$broker_container" >/dev/null
if ! docker inspect "$php_container" >/dev/null 2>&1; then
    # Sharing the broker network namespace makes its advertised 127.0.0.1 address reachable.
    # /private/tmp is the native Rust fixture output directory on every host.
    mkdir -p /private/tmp
    docker run -d --name "$php_container" --platform linux/arm64 \
        --label poolster.native-kafka-fixture=true \
        --network "container:$broker_container" \
        --mount type=bind,source=/private/tmp,target=/private/tmp \
        "$php_image" sleep infinity
    docker exec "$php_container" sh -ec '
        apt-get update
        apt-get install -y librdkafka-dev=2.8.0-1 librdkafka1=2.8.0-1 git unzip
        pecl install rdkafka-6.0.5
        docker-php-ext-enable rdkafka
        curl -fsSL https://getcomposer.org/download/2.8.12/composer.phar -o /usr/local/bin/composer
        chmod +x /usr/local/bin/composer
    '
fi

# Do not mutate/restart/delete a preexisting container. Verify it explicitly instead.
docker exec "$php_container" sh -ec "echo '$composer_sha  /usr/local/bin/composer' | sha256sum -c -"
docker exec "$php_container" php -r '
    if (PHP_VERSION !== "8.2.34" || phpversion("rdkafka") !== "6.0.5") {
        fwrite(STDERR, "Expected PHP 8.2.34 and php-rdkafka 6.0.5\n"); exit(1);
    }
'
native_version="$(docker exec "$php_container" dpkg-query -W -f='${Version}' librdkafka1)"
test "$native_version" = "2.8.0-1"
docker exec "$php_container" composer --version
printf 'Native PHP Kafka fixture ready: %s (broker namespace %s)\n' "$php_container" "$broker_container"
