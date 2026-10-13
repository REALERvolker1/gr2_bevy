# Movement execution and engine boundaries

Static findings below belong to GR2 Steam build **16083277**, shipping executable
SHA-256 `3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85`,
preferred base `0x140000000`. Addresses are RVAs; native vectors use centimeters
and seconds. No game launch, runtime observation, or parity claim accompanied this
pass. The original native binary and qualified cooked player package are the
authority. Xash movement was not inspected.

The authored bounded recipe is `research/movement-execution-scope.tsv`. Private
native exports, assembly excerpts, and hashes are in
`.work/research/movement-execution/manifest.json`. The cooked package provenance
is `.work/research/16083277/player-inspection/manifest.json`.

## Pending launch and accumulated force order

PlayerMovementComponent constructor `0x1c4f120` installs vtable `0x51f9218`.
The concrete calls and fields are:

| Operation | Concrete RVA / vtable slot | Native effect |
| --- | --- | --- |
| Character launch | `0x37f9e70` | Combine requested vector with current character velocity according to independent XY/Z override flags, then queue movement launch |
| Movement launch queue | `0x38363d0`, `+0x658` | Replace pending launch at movement `+0x3c0..0x3c8` under active/valid gates |
| Impulse queue | `0x38292a0`, `+0x818` | Add to movement `+0x274..0x27c`; velocity-change flag bypasses mass division |
| Accumulated impulse/force consumer | GR2 override `0x1c512c0`, `+0x728` | `velocity += impulse + force * dt`, then clear both accumulators |
| Pending launch consumer | `0x3835430`, `+0x660` | For a nonzero pending vector and valid movement, replace all velocity channels, set movement mode3, clear pending launch, set movement `+0x1f1` bit8 |
| Accumulator clear | `0x382e570`, `+0x730` | Clear impulse `+0x274`, force `+0x280`, and pending launch `+0x3c0` |

Ordinary movement entry `0x3838620` calls these in a definite order:

1. Base/movement preparation and root-motion preparation.
2. Accumulated impulse/force consumer `+0x728`.
3. Character-state update `+0x738` and a conditional navigation update.
4. Pending launch consumer `+0x660`.
5. Accumulator clear `+0x730`.
6. Root-motion velocity handling, character callback `+0x850`, then physics
   dispatch `+0x640`.

The consumer calls are visible at native `0x383889c` and `0x38388d5`;
the private decompilation retains their surrounding control flow. Consequently,
a nonzero pending launch overwrites the velocity that just received the queued
impulse. Without a pending launch, the added impulse survives this phase. Launch
XY/Z preservation happens when `LaunchCharacter` queues its vector, before this
consumer phase; it is not a merge with subsequently accumulated impulses.

Multiple impulse requests accumulate; multiple launch requests replace the
pending vector. A zero pending vector does not trigger the launch consumer and
does not replace current velocity. The later clear operation removes it anyway.
The ordinary entry also clears accumulated operations on its movement-disabled
exit branch. Its validity/world early exit does not establish universal clearing.

The simulated movement entry `0x3843920` updates state, consumes pending launch
when movement mode is nonzero, and clears accumulated operations. It does not
call the ordinary accumulated-force consumer. This is a distinct source path,
not evidence to apply the ordinary order to network proxies.

This closes the local pending-operation order. Blueprint event/timeline timing
relative to entry into `0x3838620`, actor/component tick prerequisites, networking,
and root-motion override behavior remain outside this bounded conclusion. A
Bevy fixed schedule ordering actions before this phase remains a host mapping.

## Plane constraint consumption

Direction projection `0x38584d0`, movement vtable `+0x500`, reads normal
`n = movement[+0xd0..0xd8]`. With movement `+0xe8` bit `0x10` enabled, it computes
`d' = d - n * dot(d, n)`; otherwise it returns the input direction unchanged.

Movement helper `0x386c090`, vtable `+0x498`, copies the requested displacement,
calls the direction projection, and passes the result to UpdatedComponent's
move slot `+0x490`. Thus this boundary constrains displacement. It does not
directly overwrite the stored velocity. The source may subsequently reconstruct
velocity from achieved movement; projecting all retained velocity immediately
when setting a plane normal is an additional host policy.

Location projection `0x3858590`, slot `+0x508`, also reads plane origin
`o = movement[+0xdc..0xe4]` and computes `p' = p - n * dot(p - o, n)`.
Normal projection `0x3858660`, slot `+0x510`, projects and safely normalizes the
result, using squared-length tolerance `1e-8`. These helpers assume the stored
normal has the required representation; this pass does not close its setter or
all plane-enable writers.

## Walking acceleration and braking

GR2 walking override `0x1c71190` delegates to ordinary walking `0x383b8e0`
when its rotated-gravity gate at movement `+0xafc` is false. Ordinary walking
clears acceleration Z and invokes movement `+0x6c0` with timestep,
GroundFriction `+0x16c`, fluid=false, and braking-deceleration getter `+0x6f8`.
That slot resolves to GR2 velocity calculation `0x1c54cf0`, which delegates to
ordinary `0x382baa0` under the same gate.

For the ordinary acceleration-present, within-speed branch, `0x382baa0` first
turns the current velocity toward normalized acceleration while preserving its
current speed: `v -= (v - normalize(a) * length(v)) * min(dt * friction, 1)`.
It then adds `a * dt` and applies the source's effective speed limit. The effective
limit includes analog input, minimum analog speed, and requested movement;
this is not simply an unconditional clamp to one constant.

No acceleration or an above-limit velocity takes the source braking path,
slot `+0x8b8 -> 0x382af20`. With separate braking friction enabled it selects
movement `+0x1ac`; otherwise it uses the supplied friction. Braking multiplies
nonnegative friction by the nonnegative factor at `+0x1a8`. It advances
`v += (-friction * v - normalize(original_v) * deceleration) * substep`.
The original direction remains fixed throughout the braking substeps. The
substep limit clamps movement `+0x1b0` to `[0.013333334, 0.05]`; when friction
is nonzero, the last split uses half the remaining time to avoid a small tail.
It stops when velocity reverses against the original vector, or squared speed
falls below `0.0001`, or below `100` when constant deceleration is active.

Base constructor `0x3828540` supplies GroundFriction8, MaxAcceleration2048,
BrakingFrictionFactor2, separate braking friction0, and walking braking
deceleration2048. These are constructor defaults, not proof of effective campaign
values. Qualified player movement export **309**, named
`CharMoveComp`, overrides MaxAcceleration to50000, MaxWalkSpeed1000,
MaxStepHeight60, WalkableFloorAngle45, WalkableFloorZ0.7071067690849304, and
FallingLateralFriction0.10000000149011612. Subsequent gameplay writers and loadout
changes still matter. Full walking floor, step, moving-base, collision, and
requested-movement closure is not claimed here.

## Gravity defaults and effective gravity

Movement gravity getter `0x3834010` returns generic gravity getter `0x3860720`
times GravityScale at movement `+0x150`. The generic getter first resolves a
PhysicsVolume and dispatches its `+0x660` gravity getter. If no volume exists,
it obtains the PhysicsSettings class through `0x3f1e710` and reads its CDO `+0x38`.
The construction wrapper `0x3f1eb40` jumps to `0x3bbb3b0`, which calls core
constructor `0x2fb06b0`. That constructor writes `+0x38 = 0xc4750000`, the `f32`
value **-980cm/s²**. This establishes the native settings fallback.

Movement's base constructor initializes GravityScale1. The same cooked player
movement export309 sets **GravityScale2.5**. Later jump, dash, wall and gravity
writers can replace that scale. Therefore neither constructor1 nor cooked2.5
alone proves the effective scale during every controller action. Native fallback
-980 likewise does not prove a world's configured gravity, PhysicsVolume gravity,
or the settings CDO after config loading. Those remain explicit dependencies.

## Validation and tool limits

The MCP health probe confirmed the original persistent GR2 program; no cold
import, new game launch, or Ghidra mutation was required. Some existing functions
have one-byte listing bodies even though direct decompilation reconstructs their
machine-code flow. Negative cross-reference/instruction-search results therefore
were not used as evidence of absence.

To locate the phase entry, a bounded original-PE byte scan inspected virtual-call
operands `+0x660` and `+0x728` only within RVAs `0x3810000..0x3860000`, then
followed PE chained unwind records to entry `0x3838620`. Ghidra's recovered body
confirmed both calls and their order. Tiny unlisted leaf `0x382e570` was verified
with bounded `objdump` assembly. The manifest qualifies private exports by hash.
Static results can specify focused Rust fixtures; synthetic fixtures do not
establish source runtime parity.

## Bounded horizontal walking implementation

`src/locomotion.rs` reconstructs only ordinary horizontal velocity calculation
with no requested/path-following movement, no fluid, no forced maximum
acceleration, and no root motion. The caller provides clamped horizontal analog
input. Host Y remains the caller's responsibility; this function does not replace
Unreal floor integration or native vertical handling.

The native overspeed predicate is movement slot `+0x438 -> 0x3869fe0`.
Bounded original-executable assembly compares
`speed_squared > max(0, maximum_speed)^2 * 1.01`, strictly. The multiplier's
`f32` bits at readonly RVA `0x489d1c0` decode to `1.0099999904632568`.
Constructor `0x3828540` writes braking substep time `+0x1b0 = 0x3cf83e10`,
which decodes to `0.03030303120613098` seconds, before the braking helper clamps
that value to its permitted range.

The ordinary `0x382baa0` branch restores a speed crossed by braking only if the
velocity was originally above the overspeed threshold, its squared speed after
braking is strictly below the effective maximum squared, and
`dot(original_velocity, acceleration) > 0`. It restores normalized original
velocity times the effective maximum. Opposing or perpendicular input does not
restore it. This restoration precedes input acceleration.

Immediately before input acceleration, the function tests overspeed again. If
still over the threshold, the acceleration cap is its current speed; otherwise
the cap is the effective maximum. It adds input acceleration times timestep,
then clamps the result to that selected cap. Thus supported incoming overspeed
can decay through braking without being immediately snapped to the ordinary
walk maximum. With no input acceleration, it retains the braking result without
this final acceleration/clamp path.

The Rust helpers convert native squared-speed tolerances from centimeters
squared to meters squared: normalization `1e-8 -> 1e-12`, braking-zero
`0.0001 -> 1e-8`, and active constant-braking stop `100 -> 0.01`. These last
thresholds correspond to0.0001m/s and0.1m/s. The portable square-root
normalization implements the recovered equation; matching the original SSE
reciprocal-square-root and Newton refinement bits remains an explicit numerical
adaptation. Focused tests cover the restore guard, cap ordering, native1.01
tolerance, friction-turn order, analog limits, braking split, separate friction,
stop thresholds and prevention of velocity reversal.
