#!/bin/sh
set -e

TOOLS_DIR="$(dirname "${0}")"
RUST_TARGET="$("${TOOLS_DIR}/target.sh")"

# `cross` doesn't pass thru the `fmt` command,
# but `fmt` doesn't require a build anyway.
IS_FMT_COMMAND="0"
if [ "${1}" = "fmt" ]; then
  IS_FMT_COMMAND="1"
fi

if [ -z "${CARGO}" ]; then
  if [ "${DISABLE_CROSS_RS}" = "1" ] || [ "${IS_FMT_COMMAND}" = "1" ]; then
    CARGO="cargo"
  else
    if ! command -v cross >/dev/null; then
      echo "ERROR: 'cross' binary not installed, consider running 'hack/build/install-cross-rs.sh', otherwise local builds may fail due to missing dependencies"
      exit 1
    fi
    CARGO="cross"
  fi
fi

git submodule init && git submodule update

if [ "${CARGO_BUILD_STATIC_CRT}" = "1" ]; then
  export RUSTFLAGS="-Ctarget-feature=+crt-static"
fi

echo "[${CARGO}] version: $(${CARGO} --version)" >/dev/stderr
export CARGO_BUILD_TARGET="${RUST_TARGET}"
echo "[${CARGO}] [${CARGO_BUILD_TARGET}] ${*}" >/dev/stderr
exec "${CARGO}" "${@}"
