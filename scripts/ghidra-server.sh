#!/usr/bin/env bash
# Run the existing installed extension or bridge in the foreground.
set -Eeuo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
ghidra_home=${GHIDRA_INSTALL_DIR:-/opt/ghidra}
backend_port=${GHIDRA_BACKEND_PORT:-8089}
mcp_port=${GHIDRA_MCP_PORT:-8081}
case "${1:-help}" in
backend)
	shift
	mkdir -p "$root/research-local/ghidra" "$root/.work/ghidra/tmp"
	exec "$ghidra_home/support/launch.sh" fg jdk GhidraMCP "${GHIDRA_MAXMEM:-2G}" \
		"-Djava.awt.headless=true -Djava.io.tmpdir=$root/.work/ghidra/tmp" \
		com.xebyte.headless.GhidraMCPHeadlessServer --bind 127.0.0.1 --port "$backend_port" "$@"
	;;
bridge)
	shift
	export GHIDRA_MCP_URL="http://127.0.0.1:$backend_port"
	exec bridge-mcp-ghidra --transport streamable-http --mcp-host 127.0.0.1 --mcp-port "$mcp_port" "$@"
	;;
configure)
	# The tracked project config owns registration. CLI add writes globally.
	cd -- "$root"
	codex mcp get ghidra
	;;
*) printf 'Usage: %s backend [Ghidra args] | bridge [bridge args] | configure\n' "$0" ;;
esac
