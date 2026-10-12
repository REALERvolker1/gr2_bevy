# Controller port contract

Reconstruct Jack's controller from the owned GR2 build's native code and cooked
Blueprints. Implement the recovered behavior in Rust, Bevy and Avian3d. The Xash
prototype is prior research, not movement authority. Keep controller arithmetic,
collision integration and presentation separate.

Before changing mechanics, read `docs/source-boundaries.md` and the relevant
`docs/research/` finding. Before Unreal/native research, read
`docs/ue-tooling.md`. Every equation needs a build-qualified source locator or
an explicit host-adaptation comment and unresolved source dependency. Static
findings, runtime observations, host adaptations and source parity are distinct.
Synthetic tests validate their stated specification, not source parity.

Human interactive GR2 testing remains unavailable until explicitly resumed by
the user. Continue static analysis and Bevy/Xvfb tests without launching GR2 or
requesting new human captures.

## Private data

Owned assets, native exports, decoded bytecode, raw traces, dumps, tool binaries
and generated builds stay in ignored `.work/` or existing owned installations.
Ghidra projects stay in ignored `research-local/` or the original project's
persistent database. Track authored summaries, provenance, source and tooling.
Keep one writer per Git checkout and Ghidra project. Durable coordination
belongs in normal storage rather than tmpfs.

## Atomic Git policy

Each commit is one coherent, independently understandable change. Stage
explicit reviewed paths. Use imperative subjects and explain non-obvious reasons
in the body. Keep generated files and proprietary inputs out of history.
Before committing run `git diff --check`, inspect `git diff --cached`, and run
the smallest meaningful check. Preserve unrelated edits and published history.

## Rust

Prefer existing engine facilities and small pure helpers. Typesafety is critical.
Bevy owns lifecycle/schedules, Avian owns queries, and one motor owns capsule
integration. Use authoritative physics poses for queries. Configure varying
values explicitly. Comments explain source limits and adaptation reasons.
Add no dependencies beyond those requested or pre-approved by the user.
