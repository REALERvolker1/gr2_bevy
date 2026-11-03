# Controller source specification

This is static analysis of GR2 Steam build 16083277, shipping executable SHA-256
`3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85`, preferred
base `0x140000000`. RVAs below belong to that binary. No GR2 launch or source
runtime comparison accompanied this work. Values use UE centimeters, seconds
and dimensionless factors. The Rust host converts lengths and velocities by 0.01.

The investigation follows original native exports and the original cooked
`/Game/Player/BP_PlayerCharacter` bytecode. Xash movement code is not a source.
New private native exports and hashes are in
`.work/research/controller-closure/manifest.json`. Prior package qualification is
the originating workspace's private
`.work/research/16083277/player-blueprint/indexed-player-qualified/manifest.json`.
Its asset and bytecode index account for all declared VM bytes and exact package
roundtrip. The authored native export recipe is `research/controller-scope.tsv`. Package export indices below are one-based. VM offsets belong to
`ExecuteUbergraph_BP_PlayerCharacter`, export 3, unless stated otherwise.

## Launch and impulse execution

PlayerCharacter constructor RVA `0x1c4c470`, reached through class construction
wrapper `0x1eb8b20`, installs vtable RVA `0x51facb8`. Slot `+0x7d8` points to
`0x37f9e70`. This concrete LaunchCharacter implementation adds existing XY when
its first Boolean is false, and adds existing Z when its second Boolean is
false. True replaces the corresponding channel. It queues the combined vector
through movement slot `+0x658` and then sends the launch event.

PlayerMovementComponent constructor `0x1c4f120` installs vtable `0x51f9218`.
Its slot `+0x658` points to `0x38363d0`, which writes a pending launch vector at
movement `+0x3c0..+0x3c8` under active/valid gates. Slot `+0x818` points to
`0x38292a0`, which adds an impulse to movement `+0x274..+0x27c`. A true Boolean
bypasses division by mass. Launch and impulse are separate pending operations;
this pass does not establish their application phase, cancellation or order.

## Ground dash

DashContinue VM `17674..18323` chooses the air path only when falling, not on a
wall, and either not sliding or sliding with `NotOnGroundTimer > 0.2`. A slide
that just left an edge therefore stays on the ground path for this discriminator.
This is not proof of a jump grace window.

DashOnFloorStart VM `19141` obtains stamina, resolves BounceBack override and
calls TryUseStaminaForDash. False exits before endpoint/movement. VM `19377`
gets DashEndPoint, stores LocationToDash and actor start position, then branches
to VM `4820`. That branch sets bIsDashing and starts Timeline_4 at a computed rate.

DashEndPoint dispatch `0x1ebc980` calls body `0x1c5a410`. The ordinary branch
uses normalized camera-planar forward/right weighted by movement input. With
both inputs zero it falls back to camera forward projected into the plane.
Direction angle uses `0x1f36bf0`, an atan2 approximation, with forward input
first and right input second. Its angle is `atan2(forward, right) * 57.295776`.
Forward range applies when `abs(angle - 90) < 15`; equality selects side range.
At zero input angle is zero, so the fallback direction still gets side range.

Native defaults are DashForwardRange 800 at player `+0x9b0`, DashSideRange 550
at `+0x9b4`, and forward threshold 15 at `+0x9b8`. The cooked player CDO export
228 does not override them. Several chip and ability paths add range before
endpoint collision work. Backward-dash special upgrade and collision-class
paths remain unresolved. The endpoint routine runs bounded capsule queries,
projects remaining displacement across shallow surfaces, handles steeper hits
and selected actor classes, and caps its search at ten iterations. A generic
Avian sweep is a host collision adaptation until these branches are reproduced.

CalculateDashPlayRate export 212 computes:

```text
rate = CurrentDashRange / max(length(start - endpoint), 1) * direction_multiplier
```

CDO export 228 sets forward multiplier to `1.7999999523162842` and side to
`1.399999976158142`. Timeline_4 template export 434 has length
`0.3499999940395355` and refers to curve export 287. Its unweighted cubic keys
are time/value/arrive tangent/leave tangent:

| Time | Value | Arrive | Leave |
| ---: | ---: | ---: | ---: |
| 0 | 0 | 4.160395622253418 | 4.160396099090576 |
| 0.3499999940395355 | 1 | 1.0844796895980835 | 1.0844796895980835 |

Use segment Hermite interpolation, multiplying each endpoint tangent by segment
length. Update handler export 156 VM 31105 jumps to VM 15438. Unless snapping,
it interpolates actor position between stored start and endpoint using this
curve, then calls K2_SetActorLocation with sweep false and teleport true.
It also calls TryKnockDash. The related TryPlayerDashOnGround export 198 has
step/low-passage gates before teleporting, but this Timeline_4 update does not
call that helper directly. Do not combine these two distinct paths silently.

Finished handler export 157 VM 31240 jumps to VM 8244. It passes endpoint minus
start into OnDashOnGroundEnd. Dispatch `0x1ebf7f0` calls `0x1c6c180`, which
queues `normalize(full_3D_endpoint_delta) * DashOnEndForce` through movement
slot `+0x818` with true. Native DashOnEndForce is 1200 at player `+0x9ac`.
This adds a velocity-change impulse; it does not overwrite current velocity.
The valid movement-mode predicate and impulse application phase remain open.

## First jump

PlayerJump dispatch `0x1ec0460` reaches `0x1c72bc0`. The entry blocks while
bIsDashingInAir, enforces jump count against bCanDoubleJump, handles wall,
clearance and state gates, increments count and selects a jump type.
The cooked CDO sets bCanDoubleJump false. Its On_Jump export 51 enters VM31908,
which immediately pops execution flow and supplies no velocity equation.

Type 5 calls helper `0x1c60dc0`; DoFirstJump dispatch `0x1ebcd90` tail-calls
`0x1c5e160`, containing the same first-jump calculation. Native defaults are
FirstJumpVelocity 1100 at `+0x910`, JumpVelocityMultipler 1 at `+0x8d4`,
JumpForwardMultipler 1 at `+0x8d8`, and JumpGravityMultipler 1 at `+0x8dc`.
Ordinary launch is:

```text
J = FirstJumpVelocity * JumpVelocityMultipler
n = normalize(incoming full 3D velocity)
launch = (FirstJumpVelocity*n.x*(JumpForwardMultipler-1),
          FirstJumpVelocity*n.y*(JumpForwardMultipler-1), J)
LaunchCharacter(launch, false, true)
```

Default XY contribution is zero, so incoming planar velocity survives and Z
becomes 1100. The special `+0x978` branch preserves incoming XY in its request
and calls LaunchCharacter with true/true. Windows registers at
`0x1c5e332..0x1c5e34d` establish both flags despite the incomplete decompiler
prototype. The branch's reflected Boolean identity remains unresolved.
Ordinary jump changes movement GravityScale at `+0x150` to InitialGravity
player `+0x558` times JumpGravityMultipler. InitialGravity default is 3; an
alternate mode uses 1.5. World gravity and full landing resets remain open.

## Air dash release

Air press native PlayerDash `0x1c722f0` requires input/readiness, not already
air-dashing, sets bIsDashingInAir, records full incoming speed at `+0x990`, and
schedules `0x1c726f0` after DashSlowmoDelay times the world-associated factor.
DashSlowmoDelay native default is 0.1 at `+0x9c0`. The callback applies sensory
movement fields and schedules late/effect and end callbacks. Native
DashSlomoEffectDelay is 0.2 at `+0x9a8`; native duration is 2, cooked CDO sets
DashSlomoDuration 5. Timer-manager clock and factor semantics remain open. Time-dilation getter
`0x1cfd060` reads an optional service float at service `+0x30` then `+4`, or
WorldSettings `+0x2e8`, with fallback 1. Its callers should retain the service
path rather than replacing it with the fallback constant.

DashInTheAirStart VM6826 requires ready and not riding a drone. It starts a
repeating 0.01-second stamina timer. VM38711..38896 adds sensory getter cost
multiplied by 0.01 then divided by global time dilation to accumulated use.
Each callback then jumps to VM31245..31686, which recomputes and debits that
single increment, rather than debiting the accumulated total. It ends the dash
when resources are exhausted. Energy-converter upgrade changes that exit.

DashInTheAirEndRefactor dispatch `0x1ebca40` tail-calls `0x1c5b750`. This end
body restores movement parameters, emits Blueprint OnDashEnd, clears current
movement velocity at `+0xc4..+0xcc`, then constructs a camera-directed impulse.
With no special-ability modifier:

```text
s = clamp(PlayerVelocityBeforeDash * 0.00083333335, 1, 1.5)
d = FollowCameraRef world forward
impulse.xy = DashInAirForwardForce * d.xy * s
impulse.z = DashInAirUpwardForce * d.z * DashInAirGravityMultiplier
```

The CDO forward force is 2200, native upward force 1300, gravity multiplier 1.
A special ability multiplies both forces by one plus its configured bonus.
The impulse uses movement slot +0x818 with true. Guard states at `+0xe74` and
riding-drone skip it. End clears bIsAirDashReady and bIsDashingInAir. InputDashReleased dispatch
`0x1ebe9e0` reaches `0x1c64ef0`, which invokes this end body when air-dashing.
ResetDash dispatch `0x1ec0890` reaches `0x1c768d0`, which restores air readiness,
clears air-dashing and player `+0x978`, zeroes CurrentDashCooldown and restores
movement settings. First jump also restores air readiness. All landing/reset
callers have not been recovered.
This is a release/end impulse, not an immediate press impulse.

Blueprint end VM11087 clears the sensory timer. If still falling and accumulated
sensory use is below AirDashStaminaCost, VM11353..11443 debits their difference,
then clears the accumulator. This end minimum-cost path does not call the
ground dash admission method. Preserve those different transaction boundaries.

## Speed target curves

Native defaults are forward minimum/maximum 700/1000 at `+0x6f4/+0x6f8`,
other minimum/maximum 500/700 at `+0x6fc/+0x700`, and crouch speed 500 at `+0x850`.
The player CDO does not override them. These are target speeds; acceleration,
friction and floor integration remain separate engine movement policies.

Forward update VM35839..35973 sets MaxWalkSpeed to interpolation between
`clamp(CurrentSpeed, GetForwardSpeedMin(), 3000)` and GetForwardSpeedMax().
It skips while the landing acceleration timeline plays or while on a wall.
Template export411 runs 1.5 seconds and ignores time dilation. Curve export297:

| Time | Value | Arrive | Leave |
| ---: | ---: | ---: | ---: |
| 0 | 0 | 4252.83203125 | 3.974170207977295 |
| 0.2937280237674713 | 0.5997332334518433 | 1.1194560527801514 | 1.1194560527801514 |
| 0.8043191432952881 | 0.900399923324585 | 0.21796390414237976 | 0.21796561777591705 |
| 1.5 | 1 | 0.13844440877437592 | 1.5846220254898071 |

Other update VM38520..38631 interpolates GetOtherSpeedMin to GetOtherSpeedMax.
Template417 runs one second, affected by dilation; curve301:

| Time | Value | Arrive | Leave |
| ---: | ---: | ---: | ---: |
| 0 | 0 | 4.988814830780029 | 4.988807678222656 |
| 0.15142104029655457 | 0.5627804398536682 | 2.3176674842834473 | 2.3176674842834473 |
| 0.3806501030921936 | 0.8822203874588013 | 0.5152373313903809 | 0.5152373313903809 |
| 1 | 1 | 0 | 0 |

Both use unweighted cubic preceding segments. The final curve301 key is linear;
its outgoing mode has no effect before the last key. CurrentSpeed writer timing,
getter upgrade modifiers and timeline input switching remain unresolved.

## Slide and remaining boundaries

Original flat/slope helpers `0x1c68f60/0x1c69330` select strict speed bands from
SlideLaunchData. They dispatch LaunchCharacter with true/false, which the concrete
implementation above now resolves as replacing XY and preserving existing Z.
The arrays and four scalar fields come from the qualified original database
package, not Xash tuning. The slope direction transform, special hit-class
sentinel, complete input/start admission and repeated-update semantics remain
open. Native probe `0x1c5d540/0x1c5d6c0` measures height difference between
points 200 cm apart traced downward 500 cm. Its -100..-5 branch is not degrees.

Remaining required controller work includes engine impulse/launch application
order, floor/corner/step collision, source gravity, slopes, slide steering,
wallrun/wall jump, climb, mount, grapple, upgrades, sensory camera/time factors,
full lifecycle resets and source-to-host runtime comparison. Passing synthetic
host fixtures cannot promote these static findings to runtime parity.
