#!/usr/bin/env bash
# Drive the rendered application and keep diagnostic output private.
set -Eeuo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd -- "$root"
command -v xvfb-run >/dev/null
command -v rg >/dev/null
cargo build --locked --bin gr2-controller -j 2
mkdir -p .work/validation .work/xvfb
export TMPDIR="$root/.work/xvfb"
# Vulkan selects the available hardware or software adapter; the log records which.
export WGPU_BACKEND=vulkan
export RUST_LOG=info
if [[ -e .work/validation/xvfb.png ]]; then
    mv -- .work/validation/xvfb.png .work/validation/xvfb.previous.png
fi
xvfb-run -a -s '-screen 0 1280x720x24' \
    timeout 120 "${CARGO_TARGET_DIR:-target}/debug/gr2-controller" --smoke \
    >.work/validation/xvfb.log 2>&1
test -s .work/validation/xvfb.png
test -s .work/validation/controller.csv
if rg -n 'ERROR|VALIDATION' .work/validation/xvfb.log; then
    printf 'Renderer validation failed; inspect .work/validation/xvfb.log\n' >&2
    exit 1
fi
printf 'Xvfb proof: %s/.work/validation/{xvfb.png,xvfb.log,controller.csv}\n' "$root"
