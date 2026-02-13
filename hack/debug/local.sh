#!/usr/bin/env sh
set -e

VER=$(date +%s)
USERIMG=$(whoami)
REGISTRY="localhost"

docker build . -f ./images/Containerfile.edera-check -t "${REGISTRY}/${USERIMG}-edera-check-debug:${VER}"

docker run --privileged --pid="host" \
  -e RUST_LOG="debug" \
  -e EDERA_PREFLIGHT_VERBOSE=true \
  "${REGISTRY}/${USERIMG}-edera-check-debug:${VER}" preinstall
