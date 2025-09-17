#!/bin/sh
set -e

# NOTE that Github CI currently looks for this rev in this script as well
CROSS_RS_REV="e281947ca900da425e4ecea7483cfde646c8a1ea"

cargo install cross --git "https://github.com/cross-rs/cross.git" --rev "${CROSS_RS_REV}" "${@}"
