#!/usr/bin/env bash
set -Eeuo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
export UEVIEWER_BIN="${PACKAGE_EXTRACT_BIN:-$root/.work/tools/ueviewer/extract}"
exec "$root/scripts/ueviewer.sh" "$@"
