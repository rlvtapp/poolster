# Native PHP and Symfony Kafka tests

These tests compile the emitted PHP package, install its real Composer dependencies, send messages to a local Kafka broker, and instantiate the generated Symfony DI container. They are ignored by ordinary Cargo test runs because they require Docker, native dependencies and a broker.

The checked-in setup uses PHP **8.2.34** from the immutable official image `php:8.2-cli@sha256:185eb902246c5757dbbb7e439bf92dfea32f627a401b1e2785bee0770fe0c41b`, php-rdkafka **6.0.5**, Debian librdkafka **2.8.0-1**, and Composer **2.8.12** (SHA-256 verified). The generated package pins Opis JSON Schema **2.6.0**. The pinned official image includes Linux ARM64 and AMD64 variants; Docker selects the host architecture by default. Set `POOLSTER_KAFKA_PHP_PLATFORM` only when intentionally testing another architecture. The setup pins direct runtime dependencies; Debian's Git/unzip utility dependencies remain selected by its package repository.

From the repository root, start the pinned broker fixture:

```sh
docker compose -f crates/plugins/typescript/tests/fixtures/kafka-broker.compose.yaml up -d
export POOLSTER_KAFKA_BROKER_CONTAINER="$(docker compose -f crates/plugins/typescript/tests/fixtures/kafka-broker.compose.yaml ps -q kafka)"
```

For an existing task broker named `poolster-alpha2-kafka`, omit that export. Its advertised address must remain `127.0.0.1:29092`. Then prepare the native runner and execute both packages' focused checks:

```sh
bash crates/plugins/php/tests/fixtures/kafka/setup.sh
cargo test -p poolster-plugin-php -p poolster-plugin-symfony \
  --lib events --offline --locked -- --include-ignored
```

`setup.sh` creates `poolster-events-php`, sharing the broker's network namespace and mounting host `/private/tmp` at the identical container path by default. For hosted Linux CI, set `POOLSTER_KAFKA_FIXTURE_ROOT` to a writable fixture directory and `POOLSTER_PHP_EVENTS_OUTPUT` to its `poolster-php-events-generated` child before running the test. It installs the pinned native extension and Composer with the commands shown in the script. `POOLSTER_KAFKA_PHP_CONTAINER` overrides the PHP container name; `POOLSTER_KAFKA_BROKER_CONTAINER` overrides the broker container name. The tests honor both variables. Setup verifies an existing PHP container rather than mutating it. It never removes containers or stops an existing broker.

Cargo must already have the project's Rust dependencies cached for `--offline`; Composer needs network access for its first install. The tests write `/private/tmp/poolster-php-events-generated` and `/private/tmp/poolster-symfony-events-generated`; Composer lockfiles and downloaded dependencies are retained there for repeat runs. The PHP test explicitly provisions unique single-partition topics for valid traffic and malformed payload/key/header/duplicate-JSON cases.

Coverage includes typed payload/key/header roundtrip, delivery metadata, manual acknowledgements, double-ack rejection, failed-handler replay, malformed-wire poisoning and uncommitted replay, cooperative cancellation, fixed-group compatibility, native producer/consumer client IDs, and generated Symfony DI registration. php-rdkafka exposes headers as a map, so duplicate **Kafka header names** cannot be detected; duplicate **JSON object members** are rejected and tested.
