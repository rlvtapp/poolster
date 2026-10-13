#!/usr/bin/env bash
set -euo pipefail
cd "$1"
shift
if [[ -n "${POOLSTER_RUBY_BUNDLE_PATH:-}" ]]; then
  bundle config set --local path "$POOLSTER_RUBY_BUNDLE_PATH"
fi
bundle install --jobs 4 --retry 3
bundle exec ruby probe.rb "$@"
