# Ghostrunner 2 controller in Bevy

A Rust reconstruction of Jack's controller from the owned GR2 native executable
and cooked player Blueprint. Uses Bevy 0.19.1 and Avian3d 0.7.0 with a locked
Cargo dependency graph. This is an implemented research port with explicit
remaining source and parity gaps, not a complete Ghostrunner 2 runtime.

> Unofficial fan research. This project is not affiliated with, endorsed by or
> sponsored by 505 Games or One More Level. Ghostrunner 2 and related marks
> belong to their respective owners. No game assets, binaries, keys or traces
> are included.

```sh
cargo run --locked
```

WASD moves, Space jumps, Shift dashes, Ctrl crouches/slides, right mouse blocks
regeneration, E grapples to a visible anchor, R resets. Click captures the mouse;
Esc releases it. Air dash holds until Shift release, then applies the recovered
camera-directed impulse.

The fixture includes recovered ground dash range/curve/end impulse, first jump,
edge grace, air-release/resource arithmetic, walking/braking and four speed
timelines, tagged-wall running/jumping, fixed-point grappling, native mantle,
strict slide tables and an Avian capsule motor. Ledge mounting activates while
airborne with positive forward input, including ascent, without holding jump.
Its ordered queries, permission guards and trajectory come from the native
component and cooked coordinator. See [ledge findings](docs/research/ledge-mounting.md).
It contains authored test geometry and no copied game assets.
Source equations and collision/clock adaptations are distinguished in
[the implementation boundary](docs/source-boundaries.md). The detailed
[native/Blueprint findings](docs/research/controller-source.md) record source
locators and remaining dependencies. Full traversal work and source-runtime
parity remain incomplete.

```sh
cargo test --locked --tests -j 2
scripts/test-xvfb.sh
scripts/test-xvfb.sh --ledge-smoke
```

The graphical test runs the actual application under Xvfb. Its screenshot,
render log and per-step committed-pose/resource CSV are archived separately in
`.work/validation/controller/` and `.work/validation/ledge/`. The default course
asserts ground dash, slide, jump, air press/release and wall stopping. The ledge
course asserts one complete native mount and its planar finish velocity. Headless tests
independently exercise real Avian collisions and source arithmetic, including
ascent admission, blocked ledges and the nonretriggering permission delay.
Vulkan validation remains enabled; the Xvfb check rejects renderer errors. See the
[renderer investigation](docs/research/render-validation.md). The Xvfb script
only drives the Bevy fixture. Headless testing of the original game
(Xvfb/gamescope) is out of scope here and lives in the separate
[gr2_headless](https://github.com/REALERvolker1/gr2_headless) project.

[UE tooling](docs/ue-tooling.md) is copied locally: pinned package inspector,
UEViewer/package extraction wrappers, bounded Ghidra exporter and passive UE4SS
movement recorder. Dependencies, owned inputs, raw bytecode/native exports and
traces stay ignored. Atomic commits and source-evidence rules are in AGENTS.md.

## Research tooling

The findings were produced with the
[Reverse Engineer Anything (REA)](https://github.com/morluto/rea) skill,
Ghidra and UE4SS. The skill text is vendored under
`.agents/skills/reverse-engineer-anything/` for provenance; it is not needed to
build, run or test the controller. REA is MIT-licensed; see
[third-party notices](THIRD_PARTY_NOTICES.md).

`tools/rea` and `tools/codex-workspace` launch the local research environment.
`.codex/` and `config/local.toml` are gitignored and machine-specific. The
recorder and research scripts read local paths from that config; copy the
example before using them:

```sh
cp config/local.toml.example config/local.toml
$EDITOR config/local.toml
```

Engine references: [Avian0.7.0 manifest](https://github.com/avianphysics/avian/blob/v0.7.0/crates/avian3d/Cargo.toml)
and [its kinematic controller example](https://github.com/avianphysics/avian/blob/v0.7.0/crates/avian3d/examples/kinematic_character_3d/plugin.rs).
