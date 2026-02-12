#!/usr/bin/env sh
set -e

VER=$(date +%s)
USERIMG=$(whoami)
REGISTRY="localhost"

docker build . -f ./images/Dockerfile.preflight -t "${REGISTRY}/${USERIMG}-preflight-debug:${VER}"

docker run --privileged --pid="host" \
  -e RUST_LOG="debug" \
  -e EDERA_PREFLIGHT_VERBOSE=true \
  -e EDERA_PREFLIGHT_TARGET_DIR='/host' \
  -e EDERA_PREFLIGHT_SKIP_GROUPS='ScriptedChecks' \
  -e EDERA_PREFLIGHT_SCRIPTS_DIR=/scripts \
  "${REGISTRY}/${USERIMG}-preflight-debug:${VER}"
