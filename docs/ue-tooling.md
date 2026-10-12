# Unreal controller research tools

These tools come from `/mnt/gr2xhl1` at commit
`368bb5b6a69a22421b908f927b26b6783263aeab`. They inspect the owned GR2
packages, native functions, and passive movement observations. No Xash engine,
Half-Life controller, model conversion, or host integration is included.

The authored package inspector is copied unchanged. It uses UAssetAPI revision
`3228c1e86261aa08131f7ec0ff1a395f5d0b2a84`, UE4.27 decoding, an exact binary
roundtrip check, and VM instruction-size accounting. Both checks must pass before
it writes its provenance manifest. Package output does not prove native behavior.

UEViewer source revision is `a0bfb468d42be831b126632fd8a0ae6b3614f981`.
The copied Linux build uses installed libpng16 and SDL2. Its SHA-256 is
`6633bede12e7274e7ea1bf65f359b8ebccaa72380b0cdd1316d538203145002c`.
Dependencies, binaries, the private pak key, and saved packages stay under
ignored `.work/`. The launchers redirect tool state there. Existing REA setup
is separate and remains available.

## Package inspection

Run from this repository:

```sh
scripts/package-inspector.sh \
  .work/extracted/16083277/controller/BP_SlideLaunchPlayerDatabase.uasset \
  .work/research/16083277/slide-inspection
```

The output directory must be new. The wrapper builds the inspector with system
.NET 10, placing build products, NuGet cache, and CLI state in `.work/`.
It writes `asset.json`, `bytecode-index.json`, and a SHA-256-qualified manifest.
The companion `.uexp` must remain beside the `.uasset`.

Verification in this workspace on 2026-10-12 built with no warnings or errors.
`BP_SlideLaunchPlayerDatabase` decoded one export and passed the exact binary
roundtrip. Private output is `.work/research/16083277/slide-inspection/`.
`BP_PlayerCharacter` decoded 440 exports and indexed 226 functions, passing
the same binary roundtrip and VM-size checks. Its output is
`.work/research/16083277/player-inspection/`. UEViewer also listed the saved
slide package successfully. ShellCheck and Pyright passed for these scripts.
The subsequent native pass reopened the original persistent project. The local
health probe initialized MCP, enumerated 222 tools and confirmed the selected
`Ghostrunner2-Win64-Shipping.exe` program. Project-local `.codex/config.toml`
registers this bridge; no global MCP configuration was changed.

To save another owned package, set `GR2_PAK` to the installed
`Ghostrunner2-WindowsNoEditor.pak`:

```sh
scripts/ueviewer.sh -save -game=ue4.27 \
  "-path=$(dirname -- "$GR2_PAK")" \
  "-aes=@$PWD/.work/research/16083277/package-key/verified-key.txt" \
  /Game/Player/BP_PlayerCharacter
```

Umodel saves under `.work/tool-state/ueviewer/home/UmodelSaved/`. Its `-out`
option controls exports, not saved packages. Both launchers supply a default
`-out` under `.work/extracted/ueviewer/`; an explicit later `-out` overrides it.
`scripts/package-extract.sh -help`
exposes the pinned standalone package extractor. `UEVIEWER_BIN` and
`PACKAGE_EXTRACT_BIN` override the respective tool paths. Keep keys private;
use key-file arguments rather than exposing a key in shell history.

## Native analysis

The copied Ghidra scripts use the installed `/opt/ghidra` and
`bridge-mcp-ghidra`, with the existing GhidraMCP 6.0.0 extension.
The persistent project in the prior workspace retains recovered names and types.
Reuse that project with one owner rather than importing the entire binary again.
Ghidra rejects hidden canonical project ancestors, so databases belong in
ignored `research-local/ghidra/`, not `.work/` or a symlink into it.

Health checks initialize MCP and verify loaded program identity:

```sh
python3 scripts/ghidra-health.py \
  --expected-program Ghostrunner2-Win64-Shipping.exe \
  --url http://127.0.0.1:8081/mcp
```

An existing backend/bridge can be reused. To run the installed frontend manually,
use `scripts/ghidra-server.sh backend --help` for installed project-open options
and `scripts/ghidra-server.sh bridge` for the bridge. Ports default to
8089 and 8081 and accept `GHIDRA_BACKEND_PORT`/`GHIDRA_MCP_PORT` overrides.
The wrapper does not install extensions or change global MCP configuration.
The `configure` command only reads an already-registered `ghidra` entry.

An authored TSV scope has columns `id`, `rva`, `reason`. Export only the bounded
controller dependency list:

```sh
python3 scripts/export-scope.py research/controller-scope.tsv \
  .work/decompiled/16083277/controller-new \
  --build 16083277 \
  --sha256 3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85
```

The scope must exist within this repository. The exporter verifies both Ghidra's
import hash and the source executable hash, preserves completed output, and
records unresolved calls. It does not claim dependency closure automatically.

## Passive movement recorder

`config/ue4ss/GR2MovementRecorder/` contains the original authored UE4SS 2.4
recorder. It observes Unreal fields without driving player input. The installer
requires an existing working flat UE4SS installation and verifies the researched
shipping binary before changing the recorder. Installation is an explicit action,
not part of package research or the Bevy Xvfb test.

```sh
python3 scripts/install-movement-recorder.py --help
python3 scripts/archive-movement-traces.py --help
python3 scripts/stamina_trace.py --help
```

Default active storage is `~/.local/state/gr2_bevy/traces/movement/`; archives
are `.work/traces/movement/`. The installer rewrites Wine's `Z:` output path,
backs up load order and settings, and refuses an active game or edited existing
recorder. `--update` verifies installed hashes before replacing an existing mod.
The copied override INI is a researched reference, not automatically installed.

Read `movement_trace.py` before interpreting traces. Samples carry possession
and upgrade revisions; missing upgrade observations mean unknown loadout.
`stamina_trace.py` reports sampled dash timestamp transitions and resource
changes. Sampling cannot prove physics-tick parity, final dash cost, or behavior
between polls. No GR2 launch script is copied or executed here.
