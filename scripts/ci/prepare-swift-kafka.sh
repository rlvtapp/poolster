#!/usr/bin/env bash
set -euo pipefail
source_dir="$RUNNER_TEMP/librdkafka-source"
prefix="$RUNNER_TEMP/librdkafka-2.12.0"
sudo apt-get update
sudo apt-get install --yes build-essential pkg-config zlib1g-dev
# Verify the release commit rather than trusting a movable version tag.
git clone --depth 1 --branch v2.12.0 https://github.com/confluentinc/librdkafka.git "$source_dir"
test "$(git -C "$source_dir" rev-parse HEAD)" = 2f2208847f7f91eaf54bcbd2cedf5dbb2dd3a3fe
cd "$source_dir"
./configure --prefix="$prefix" --disable-ssl --disable-sasl --disable-curl
make -j2
make install
printf 'POOLSTER_SWIFT_KAFKA_PREFIX=%s\n' "$prefix" >> "$GITHUB_ENV"
printf 'PKG_CONFIG_PATH=%s/lib/pkgconfig\n' "$prefix" >> "$GITHUB_ENV"
