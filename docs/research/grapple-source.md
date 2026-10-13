# Ordinary grapple source findings

Status is static source reconstruction, with runtime parity pending. The scoped
implementation target is the ordinary fixed grapple point. Pullable objects,
shuriken blink, motorcycle mounting, enemy grapple targets and forced endpoint
climbing have separate branches. No GR2 session was launched.

All native RVAs refer to owned Steam build `16083277`, module
`Ghostrunner2-Win64-Shipping.exe`, image base `0x140000000`, SHA-256
`3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85`.
The existing persistent Ghidra project was queried without annotation changes.
Bounded functions are listed in `research/grapple-scope.tsv`; private exports,
registration/field joins, package manifests and instruction dumps are under
`.work/research/grapple-closure/`.

## Source joins

Native names come from exact name/function pointer pairs, followed through the
Unreal execution thunk. SkillGrapplingHook registration starts at RVA
`0x540b1f0`. `UseGrapplingHookR` at table `0x540b3e0` points to thunk
`0x1eee720`, whose tail jump reaches body `0x1cb3190`. `IsHookAvailableR`
points to thunk `0x1eedcf0`, which calls body `0x1ca39d0`.

GrapplingPoint registration starts at RVA `0x52f3ed0`. Its class getter
`0x1e45d80` jumps to class initialization `0x1e46700`; construction wrapper
`0x1e47070` reaches constructor `0x1c88f80`, installing vtable `0x5200328`.
`GrappleRefacto` registration at `0x52f3f90` points to thunk `0x1dc9310`.
That thunk dispatches vtable slot `+0x678`, which holds body `0x1c9fcc0`.
`UpdateGrapplingTimeline` registration at `0x52f4040` maps via thunk
`0x1e492b0` to body `0x1cb18f0`.

Owned cooked packages decoded with UAssetAPI revision
`3228c1e86261aa08131f7ec0ff1a395f5d0b2a84`, passing exact binary roundtrip
and VM instruction-size checks:

- `/Game/Blueprints/Skills/SkillGrapling`: 42 exports, 40 functions.
- `/Game/Blueprints/Skills/Grapling_Point`: 61 exports, 41 functions.
- `/Game/Blueprints/FloatCurves/GrapplingPointCurve`: one export.

The original `BP_PlayerCharacter` inspection supplies the grapple jump branch.
Package manifests retain source and output hashes. Native metadata determines
field ownership; neighboring names alone are not treated as an owner join.

## Input and target admission

`IsHookAvailableR`, native `0x1ca39d0`, requires all of:

- A selected `PointToGrapple` exists.
- Point `IsGrappling` and player `bIsGrapling` are false.
- Point `BlockGrapple` is false and `PointState` equals zero.
- Projected X/Y satisfy `0 <= x < viewport_width` and
  `0 <= y < viewport_height`.
- The point differs from `GrapplingPointUsed`, or
  `bCanGrappleToPreviousGrapplePoint` is true.

`UseGrapplingHookR` repeats these gates and then has additional native branches
for motorcycle proximity, enemy upgrade interactions and external controller
state. These branches are outside the ordinary fixed target slice.

Target finding is separate from request admission. Native `0x1c9c190` selects
the minimum `PointWeight` from registered eligible points. Point update
`0x1cb2b30` projects to screen, maintains cooldown/state and computes normalized
screen-center distance. In its visible active branch the weight is
`2 * DistanceToPlayer / MaxGrapplingDistanceInactive + screen_center_distance`.
Range uses a strict squared-distance bound. Visibility is a source collision
query in `0x1c92020`; ignore-owner and channel choices matter. Host camera-visible
anchor selection and Avian line of sight therefore need explicit adaptation
comments, rather than claiming the source target ranking is reproduced.

Native constructor defaults include speed `1300 cm/s`, release extent `200 cm`,
inactive range `3500 cm`, hide range `5000 cm`, launch power `2000 cm/s`, point
cooldown `0.5 s` and point reuse reset delay `0.3 s`. The ordinary
`Grapling_Point` class default overrides inactive range to `4000 cm` and binds
`GrapplingPointCurve`. Its construction script copies the authored legacy
parameters to native fields, including speed, release extent, launch power and
`LaunchDirectionTargetRef`. Use the actual endpoint component's position,
which is distinct from the visible actor origin.

## Trajectory and release

Point start `0x1c9fcc0` captures player position `p0` and endpoint `q` from
`LaunchDirectionTargetRef`. It stores distance `d = length(q - p0)` and
launch direction `n = normalize(q - p0)`, with native squared-length tolerance
`0.0001 cm²`. Its timeline play rate is

```text
rate = clamp(GrapplingSpeed / d, 0.6, 2.5) * player.CustomTimeDilation
```

The interpolation input is the curve value, not elapsed distance or a spring
force. Timeline setup `0x1ca25d0`, factory `0x1cf6a70`, binds the native update
and finish functions, sets length mode one and nonlooping playback. Source
curve keys have a final time of `0.5 s`:

| Time | Value | Arrive tangent | Leave tangent |
| --- | --- | --- | --- |
| 0 | 0 | 7.6666669845581055 | 0 |
| 0.05999999865889549 | 0.46000000834465027 | 2.7978944778442383 | 2.7978944778442383 |
| 0.5 | 1 | -0.03536045551300049 | -0.03536045551300049 |

Every key is cubic, user tangent, and unweighted. For an interval of width `h`,
with normalized `u`, the curve uses ordinary cubic Hermite interpolation:

```text
C = (2u³ - 3u² + 1) a.value + (u³ - 2u² + u) h a.leave
  + (-2u³ + 3u²) b.value + (u³ - u²) h b.arrive
p_requested = p0 + (q - p0) * C
```

Native `0x1cb18f0` requests a swept actor-location move to `p_requested` and
marks the source player launch/movement flag. It follows a cancellation branch
if the move returns false or the player dash flag at `+0x97b` is set. A
successful partial sweep does not automatically mean cancellation; an Avian
blocked-distance threshold is a host policy unless separately justified.

The release check reads the actual player pose after the sweep. It requires
`abs(p.x-q.x)`, `abs(p.y-q.y)` and `abs(p.z-q.z)` each to be at most
`DistanceToRelease`. This is a box, not a radial distance check. Release requests
`LaunchCharacter(n * LaunchAfterGrapplePower, true, true)` and clears the
player grapple flag. Assembly at RVA `0x1cb1af9..0x1cb1b1f` verifies both XY
and Z override booleans, omitted by the decompiler prototype. Ordinary movement
can release before the curve's last key. A Bevy implementation should retain a
single authoritative motor and feed the requested displacement through Avian.

Only the motorcycle branch refreshes `q` from the endpoint component on every
update, selected by point `IsForMotorcycle` at `+0x295`. Ordinary fixed-point
travel retains the captured endpoint and launch direction.

## Animation, cancellation and resource limits

The initial hook request starts presentation and invokes the selected point's
Blueprint through `ContinueGrappilngAfterAnimation`, native `0x1c94060`.
`SkillGrapling` ubergraph VM `5944..6094` calls the selected point's `Graple`
event, whose VM entry `1866` invokes native `GrappleRefacto`.
The hand montage's `LateThrow` notify opens a gate, sets
`bCanGrappleToPreviousGrapplePoint` true at VM `161`, and calls the continue
function at VM `172`. Playing the hook montage sets this flag false at VM
`4493`. The hand completion path also reaches VM `5839`, runs
`EndHookAnimation`, then sets it true at VM `15`. There is no evidence for
replacing this lifecycle with a fixed two-second same-point timer. A host
without the montage needs a documented timing adaptation.

The point's own cooldown timer is set to `GrappleCooldown` at native start.
Finish `0x1c9c3d0` and explicit stop `0x1caef10` schedule callback
`0x1cab1b0` after `TimeToGrappleBeAbleAgain`; its assembly clears point
`IsGrappling`. Default delay is `0.3 s`. Skill stop `0x1caedc0` dispatches to
its active point or registered points and clears player grapple state. Stop
also has animation-rate adjustments and optional destruction branches.

`BP_PlayerCharacter.CheckPlayerCanJumpWhileGrappling` allows normal jump when
not grappling or when required references are missing. During an ordinary
fixed grapple it rejects jump; its pullable-point branch clears the player and
pullable active flags and permits jump. `DoGraplingJumpCon` separately calls
`StopGrapplingRefacto`, clears player grapple state, then applies additive
vertical launch `4000 cm/s * LaunchDirection.Z` with both overrides false.
A caller must preserve that distinction rather than letting every fixed
trajectory use the pullable cancellation behavior.

Player helper `0x1c787f0` updates `bIsGrapling`; air-dash readiness at `+0x6d4`
is updated only under native upgrade predicates `0xb8` or `0x39`, including
an active-predicate test for `0x39`. It cannot justify unconditional air-dash
refill for every grapple. Native preparation also invokes stop/reset helpers;
their complete jump-count and resource consequences remain outside this
bounded closure. No blanket jump-count reset is claimed here.

## Remaining source and runtime work

The fixed-point equations and package bindings are recovered. Source selection
channels/ranking, precise montage-notify timing, arbitrary point subclasses,
endpoint climbing, motorcycle and enemy interactions, UE swept-move return
semantics under contacts, and upgrade-driven readiness need further closure or
runtime observations. Headless Bevy tests can verify the reconstructed equations
and their stated host collision policy. They do not establish GR2 parity.
