#!/usr/bin/env bash
set -euo pipefail

taplo fmt --check
rumdl check .
cargo sort-derives --check
typos
actionlint
git ls-files -z '*.sh' | xargs -0 -r shellcheck
