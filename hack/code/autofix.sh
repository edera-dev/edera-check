#!/bin/sh
set -e

REAL_SCRIPT="$(realpath "${0}")"
cd "$(dirname "${REAL_SCRIPT}")/../.."

cargo clippy --all --fix --allow-dirty --allow-staged
cargo fmt --all
