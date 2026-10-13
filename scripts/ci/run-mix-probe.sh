#!/usr/bin/env bash
set -euo pipefail
cd "$1"
shift
mix deps.get
mix run probe.exs "$@"
