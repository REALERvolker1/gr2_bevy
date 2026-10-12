#!/usr/bin/env bash
# Umodel's -save uses HOME/UmodelSaved rather than -out.
set -Eeuo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
state="$root/.work/tool-state/ueviewer"
export HOME="$state/home"
export XDG_CONFIG_HOME="$state/config"
export XDG_CACHE_HOME="$state/cache"
export TMPDIR="$state/tmp"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$TMPDIR"
tool="${UEVIEWER_BIN:-$root/.work/deps/UEViewer/umodel}"
if [[ ! -x "$tool" ]]; then
	printf 'Missing UEViewer binary: %s\n' "$tool" >&2
	exit 2
fi
# Both tools accept -out; a later caller option overrides this private default.
exec "$tool" "-out=$root/.work/extracted/ueviewer" "$@"
