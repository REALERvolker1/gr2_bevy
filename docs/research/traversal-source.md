# Traversal source findings

These are static findings for owned GR2 Steam build 16083277, executable SHA-256
`3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85`, image base
`0x140000000`. Addresses below are RVAs. Source units are centimeters and seconds.
No source runtime testing or parity comparison accompanied this research.

The bounded native recipe is `research/traversal-scope.tsv`; private native
exports, hashes and package manifests are in `.work/research/traversal-closure/`.
Original PlayerCharacter bytecode is qualified by the package manifest cited in
[controller-source.md](controller-source.md). New owned package decodes for
`/Game/Player/BP_WallClimbing`, `WallrunSlowdown`, `WallRunUplift` and
`ClimbAnimationCurve` pass exact binary roundtrip and VM-size checks. Export
indices and VM offsets below are one-based package indices and in-memory VM
coordinates, not serialized file offsets. No Xash movement code was used.

## Wallrun admission and probes

Player Blueprint VM2749..3501 requires bCanMoveInAir. Positive forward input
continues existing wallrun only if CanKeepWallrunning passes; otherwise it calls
CanStartWallrunning, then native direction probes. Nonpositive forward input
stops an existing run. If all four bCanFall directions are clear, it calls
StopWallrunWithLaunch. These conditions do not imply arbitrary wall attachment.

CanKeepWallrunning dispatch `0x1ebb900` calls `0x1c56ae0`, returning
`planar_speed > ForwardMaxSpeed * 0.1`. Default threshold is100 cm/s, strict.
CanStartWallrunning dispatch `0x1ebb960` calls `0x1c56b90`. It requires wallrun
readiness, camera, not sliding, not climbing and the movement falling predicate.
It rejects `planar_speed < ForwardMaxSpeed * WallrunMaxSpeedFractionCondition`,
whose native fraction is0.1. Equality survives this start speed gate.

Start uses a candidate overlap within1.5 times scaled capsule radius, filters
component collision/class states, and requires candidate surface Z strictly
within capsule height around the actor. It chooses a candidate closest surface
point before the angle test. Source helper `0x1b75c60` normalizes both vectors
and returns `acos(clamp(dot,-1,1))*57.295776`. The vectors are camera forward
with Z cleared and actor-position-to-candidate-point including Z. Cooked player
CDO accepts `10 < angle < 140` degrees. These are not movement-to-wall-normal
angles. The complete collision object/channel/class interpretation remains open.

RaycastInFourDirectionWallRun dispatch `0x1ec0580` calls `0x1c75680`. It tries
left, forward-left, right, forward-right and backward direction probes, with
additional fall/ledge checks. Probe helper `0x1c75720` uses100 cm cardinal and
70 cm normalized diagonal traces. Actor and component `BlockWallrun` tag checks
reject the selected surface. The tag literal is at RVA `0x51fc718`, joined through
the constructor to BlockWallrunTag at player+0x5e0. Selected special actor classes
have extra flag/component checks. A simple host surface tag and side rays are
explicit adaptations; they do not recover this complete query classifier.

## Wall direction and velocity

Probe helper `0x1c75720` calls `0x1c77fd0` with true for left and false for right.
The hit normal rotates around world Z using helper `0x1a403d0`. Disassembly at
`0x1c78027..0x1c78042` establishes angle `(side XOR1)*180-90`, giving -90 degrees
for left and +90 for right. This tangent is WallForward at+0x618; hit normal is
WallNormal at+0x60c. Reflection records independently join those offsets.
It detaches when tangent dot actor forward is less than0.1.

The first selected wall captures incoming full speed, sets movement MaxWalkSpeed
to cooked MaxWallrunSpeed1350, and captures a speed for the slowdown curve. If
post-event current speed is above1350, captured speed is1350. Otherwise it clamps
pre-event incoming speed between native WallrunSpeed1000 and MaxWallrunSpeed1350.
The emitted event is StopWalkSpeedBPTimelines, not a jump-count reset. Its native
wrapper `0x1eba940` resolves through FName initializer `0x6f1f30` and string
`0x53bf8c0`. Other event timing remains outside this bounded closure.

CheckWhichSideIsWallBP dispatch `0x1ebc5a0` calls `0x1c584b0`. While already
bOnWall it restores air-dash readiness and writes
`velocity = normalize(WallForward) * movement.MaxWalkSpeed`, including Z.
This is source velocity authority, not a guessed projection onto a wall.

ExecuteWallRun dispatch `0x1ebd190` calls `0x1c60f00`. Its ordinary branch
requires normalized planar velocity dot normalized planar camera forward >0,
normalized full velocity dot full camera forward >=0.3, and actor forward dot
WallNormal >=-0.97. It records camera direction, starts native timelines and
sets bOnWall. Additional action/mode gates remain source dependencies.

WallrunSlowdownUpdate dispatch `0x1ec2920` calls `0x1c87440`. While on a wall:

```text
max_walk_speed = captured_speed + (WallrunSpeed - captured_speed) * alpha
velocity = normalize(current_full_velocity) * max_walk_speed
```

The original `/Game/Blueprints/FloatCurves/WallrunSlowdown` curve has unweighted
cubic keys time 0/value 0/tangents 0 and time 2/value 1/tangents 0. Thus its segment
is `alpha=3u²-2u³` for `u=clamp(time/2,0,1)`. Wall-selection helper starts the
timeline at player+0x6a8 after capturing speed. Complete timeline clock/rate
configuration remains open. A persistent13.5 m/s entry speed is not implied;
ordinary incoming speed <=10 m/s produces captured10 m/s at the first curve update.

## Plane constraint, uplift and detachment

WallrunUpdatePlaneConstraint dispatch `0x1ec29a0` calls `0x1c875b0`. When on
wall, not jumping, another source gate is active and planar speed exceeds native
WallrunVelocityPlaneConstraint110 cm/s, it sends normal `(0,0,1)` through movement
slot+0x4e0. Concrete target `0x3873f40` normalizes the request and writes movement
plane normal at+0xd0..0xd8, resetting axis byte+0xeb. Detachment sends zero.
The engine consumer, enable bit, collision scheduling and gravity projection
remain open. No gravityscale=0 write was recovered in these wallrun paths.

UpliftPlayerWallrun dispatch `0x1ec2730` calls `0x1c866c0`. If its ceiling query
allows, it interpolates actor Z toward stored PlayerZAxisValue+uplift, preserving
XY. Native WallrunUpliftPlayer `0x1c87770` chooses uplift300 cm if planar
displacement from the saved jump start is less than25 cm, otherwise150 cm.
The original WallRunUplift curve is linear0 to1 over10 seconds. Runtime play rate
and phase remain open; this curve duration must not be guessed into a host.

StopWallrunBP/DropFromWall reach `0x1c7e1a0`. It clears plane normal and bOnWall,
stops several timelines, stores actor departure position, moves PreviousWallPending
into PreviousWall and starts cooldown/rotation callbacks. SameWallRunningCooldown
native2 seconds is overridden by cooked CDO to0.30000001192092896 seconds. Its
callback `0x1c76f10` clears PreviousWall and sets bCanWallrun true. Another
1 second callback `0x1c7e5c0` clears departure state+0x5c9. Wall-tangent cache
reset `0x1c77700` zeros+0x600..608 after native+0x5dc delay1.2 seconds.

StopWallrunWithLaunch dispatch `0x1ec1ad0` calls `0x1c7e5f0`. It requests
LaunchCharacter `(0,0,800)` with false/true, detaches, sets
bIsDuringWallrunCoyoteTime at+0x66c and schedules its clearing callback
`0x1c55bc0` after WallrunCoyoteTime+0x668, default 0.30000001192092896 seconds.
This wall-edge launch/coyote path is distinct from ordinary wall departure.

## Running wall jump

DoWallJump dispatch `0x1ebcdd0` tail-calls `0x1c5eda0`. Windows XMM arguments
establish the inputs to helper `0x1c67690`:

```text
F = WallJumpForwardStrength * WallJumpForwardMultipler
N = WallJumpNormalStrength
Z = WallJumpHeight * WallJumpVelocityMultipler
launch.xy = normalize(WallForward).xy * F + normalize(WallNormal).xy * N
launch.z = Z
LaunchCharacter(launch, false, true)
```

Cooked forward/normal strengths are250/850 cm/s. Native height1100 and all three
multipliers1 are not overridden in the player CDO. The launch adds XY to incoming
velocity and replaces Z, as established by concrete LaunchCharacter in the
controller specification. GravityScale becomes InitialGravity3 times
WallJumpGravityMultipler1, with an alternate source mode using1.5. The helper
also detaches and schedules a0.3 second source callback.

Counter handling remains unresolved. Wall selection does not directly reset
JumpCounter+0x920. It clears bWasJumpingPlayer+0x91d. Native PlayerJump has a
count gate before the wall branch, while an independently reflected DoWallJump
entry exists. ResetJumpCounter dispatch `0x1ec0910` tail-calls `0x1c76d90` and
zeros+0x920, but its attachment caller has not been recovered. Host wall-jump
counter handling must remain an explicit adaptation.

## Climbing wall Blueprint

The original `/Game/Player/BP_WallClimbing` is a Blueprint actor used by the
player's WallClimbingActor child. It is not a native WallClimbingComponent class.
Its decode has26 exports and14 functions with exact roundtrip/VM-size checks.

ClimbWallCon export 2 enters Ubergraph export 1 VM4390, then traces80 cm from camera
along camera forward using object types0 and9. After a hit it requires CanClimb
and held jump input, sets climbing/rotation flags, records actor location and
hit normal, sets bCanWallrun false, and starts Timeline_0 at play rate
1.2999999523162842. Update VM3239..3388 requests actor position:

```text
position = lerp(saved_climb_start, saved_climb_start + (0,0,400), alpha)
K2_SetActorLocation(position, sweep=false, teleport=false)
```

Timeline_0 template export 24 length 1 second references position curve export 20.
Its unweighted cubic keys are:

| Time | Value | Arrive tangent | Leave tangent |
| ---: | ---: | ---: | ---: |
| 0 | 0 | 4.230441570281982 | 2.0409226417541504 |
| 0.7351821660995483 | 0.875657856464386 | 0.6271374821662903 | 0.6271406412124634 |
| 1 | 1 | 0.21017993986606598 | 0.21017855405807495 |

Rotation curve export 19 is linear0 to1 in0.25 seconds. The update interpolates saved
control rotation toward pitch from the hit normal and yaw+180. A60 cm upward
ceiling trace stops climbing, clears CanClimb and writes movement velocity zero.

WallJumpCon export 6 enters VM1818. It saves actor pose and controller rotation,
starts a position-hold timeline, stops wall sticking, restores rotation control,
and requests LaunchCharacter `(0,0,400)` with true/true at VM2184. It clears
climbing/release flags and waits0.2 seconds. This is a climb-wall release jump,
not the running-wall jump equation above.

ClimbEdgeCon export 4 enters VM2831. It marks ContinueWithJump true, stops the
climb-wall timeline and schedules an inverse-CustomTimeDilation delay to clear
climbing. Some of its position/animation effects belong to native player paths.
Do not label the 400 cm climb-wall trajectory as recovered ledge mantle motion.

## Native ledge mounting

Player ClimbComponent is a native UClimbComponent, independently joined by its
class name at RVA 0x5279b08, class getter 0x1dee4c0 and native TryClimb registration
pair 0x527d170. The exec thunk 0x1df3600 calls 0x1a49ac0. Its constructor wrapper
0x1defb30 tail-calls 0x1a18aa0 and installs vtable 0x51737a8. The update at vtable
slot+0x350 is 0x1a442c0. This native component is separate from BP_WallClimbing.

Original PlayerCharacter package export 267 contains the following component
configuration. Omitted properties retain corroborated native constructor values.

| Field | Component offset | Configured value | Origin |
| --- | ---: | ---: | --- |
| MaxClimbHeight | 0xb0 | 300 cm | Cooked |
| MaxWallForward | 0xb4 | 50 cm | Cooked |
| MaxEmptyTopForward | 0xb8 | 300 cm | Native |
| AdditionalHeightOffset | 0xbc | 0 cm | Cooked |
| ClimbSpeed | 0xc0 | 1500 cm/s | Cooked |
| MoveForwardOffset | 0xc4 | 0 cm | Cooked |
| MoveForwardSpeed | 0xc8 | 2500 cm/s | Cooked |
| BottomLedgeMaxDist | 0xcc | 80 cm | Cooked |
| LedgeDepthCheckOffset | 0xd0 | 0 cm | Cooked |
| CollisionChannel | 0xd4 | ECC_WorldStatic | Cooked |
| bShouldAutoActivateOnFalling | 0xd5 | true | Cooked |
| NormalLedgeHeight | 0xd8 | 100 cm | Native |
| LowLedgeHeight | 0xdc | 0 cm | Native |
| AdditionalFinishValocityMultiplier | 0xec | 1000 | Native |
| AdditionalClimbHeightForMovingObjects | 0xf0 | 150 cm | Native |
| AdditionalClimbDistanceForMovingObjects | 0xf4 | 300 cm | Native |

The finish value's source spelling is Valocity. Its consumer multiplies actor
forward directly to produce velocity, so it is a speed scalar in this boundary.
Player Blueprint VM27414 calls AdjustToCapsule with its CapsuleComponent. Exec
0x1df2210 reaches body 0x1a1c660. Its original instructions overwrite MaxWallForward
with 1.5 times scaled capsule radius, and AdditionalHeightOffset with 1.5 times
scaled capsule half-height plus 20 cm. Thus cooked 50/0 values are configuration
defaults, replaced during setup. Float constants 1.5/20 are at RVAs 0x48615a8 and
0x4854c80. Other callers and runtime capsule-resize coupling remain open.

### Detection and initialization

TryClimbing dispatch 0x1ec1f10 calls player body 0x1c828b0. Ordinary calls require
positive forward input, input/action gate and player state guards. It calls
UClimbComponent.TryClimb through player+0x7b0. On a true return it stops wallrun,
chooses high/low climb events using component+0xd8/+0xdc height thresholds relative
to camera, and updates climb permission state. This is an action coordinator,
not the actual native mounting trajectory.

Component TryClimb 0x1a49ac0 first requires owner/world. It checks overhead capsule
clearance before forward ledge queries. Helpers 0x1a25440 and 0x1a259f0 perform the
wall/top and full capsule clearance operations. The ordinary facing guard in
0x1a259f0 uses camera forward with Z cleared and wall-hit impact normal:

```text
deviation = 180 - acos(clamp(dot(normalized_camera_XY, impact_normal), -1, 1))
                   * 57.295776
reject if deviation > 20 degrees
```

Forced calls bypass this facing guard. Several special actor classes reject the
query. Actor/component tags and additional full-capsule sweeps reject blocked
upper space. The exact shape/channel argument ABI and hit-coordinate conventions
must be retained as open dependencies; an authored host ledge detector is a
query adaptation even when it supplies source-compatible geometry.

TryClimb's existing decompiler incorrectly declares void. Instruction 0x1a49afd
initializes SIL false;0x1a4a0b8..0x1a4a0bf loads the selected hit's blocking bit;
0x1a4a54b clears it on a special rejection;0x1a4a5a0 returns SIL in AL. The reflected
exec thunk also writes this Boolean output. A true return is not identical to
bIsClimbing at component+0xd6, which has further target/tag guards.

After an accepted query, initialization records hit actor/component and their
origin using 0x1a287e0. It requires a nonzero origin vector; invalid references
also produce zero. It then captures these values:

```text
S = initial actor position                       stored at component+0x188
H = selected hit ImpactPoint                    component+0x118
T = (S.x, S.y, H.z + AdditionalHeightOffset)      stored at component+0x194
D = length(T - S)                               stored at component+0x1ac
F = length(T - H) + MoveForwardOffset - scaled_capsule_radius
                                                 stored at component+0x1b8
d = normalized planar actor forward         stored at component+0x1a0
O = hit actor/component origin                   stored at component+0x1d0
```

It sets bIsClimbing true and movement velocity zero. Parent hit-coordinate and
collision-resolution policy still need closure before interpreting T.z as a
standalone world-surface height. Do not replace F with an invented fixed distance.

### Two movement phases

Native update 0x1a442c0 uses world delta at+0x5b0 and PlayerCharacter custom time
dilation at+0x98. It runs only while component bIsClimbing and required owner/world
references are valid. The two alphas are at+0x1b0 and+0x1b4. The recovered phase
arithmetic is:

```text
if lift_alpha < 1:
    lift_alpha = clamp(lift_alpha + delta * ClimbSpeed * player_factor / D, 0, 1)
    position = lerp(S, T, lift_alpha)
else if forward_alpha < 1:
    forward_alpha = clamp(forward_alpha + delta * MoveForwardSpeed * player_factor / F, 0, 1)
    B = (S.x, S.y, current_actor_position.z)
    delta_origin = current_surface_origin - O
    position = lerp(B, B + d * F + delta_origin, forward_alpha)
    O = current_surface_origin
else:
    movement_velocity = current_owner_actor_forward * AdditionalFinishValocityMultiplier
    finish()
```

The update writes position through native actor-position helper 0x36fe260 with
zero flag arguments. Phase 2 begins on the tick after lift reaches1. Finish occurs
on the tick after forward reaches1. Do not consume a step's unused lift time
inside the forward phase. At factor1 the continuous arithmetic durations are
D/1500 and F/2500, with those additional update-boundary delays. The source does
not clamp D/Fto a positive epsilon. Host geometry validation must be identified
as an adaptation when preventing degenerate or negative distances.

For a static surface delta_originis zero. For moving surfaces source uses the
increment since the previously stored origin, then updates that origin every
forward tick. This is not a cumulative displacement from mount start. The update
returns without pose commit when origin lookup produces zero.

Finish 0x1a310c0 clears bIsClimbing, both alphas, captured trajectory and hit state.
Its ordinary completion keeps the1000 cm/sactor-forward velocity assigned above.
Explicit cancellation/forced finish can call it separately. The full lifecycle
and invocation closure remain open.

### Cosmetic climb curve and remaining work

MoveCharacterOnClimbUpdate dispatch 0x1ebf390 calls player body 0x1c6b150. It
interpolates PlayerLocationOnClimbStart+0x804 toPlayerLocationOnClimbStartWithOffset
+0x810 using supplied alpha. Original ClimbAnimationCurve is linear0 to1 over
0.05000000074505806 seconds. Its endpoint writers, runtime play rate and montage
coupling remain open. This curve is distinct from the UClimbComponent two-phase
update; layering both unconditionally would invent a second movement owner.

Required follow-up includes exact capsule/ledge query ABI, remaining capsule-resize callers,
special actor classes and moving-platform corrections, engine movement/query
phase, wall-jump counter callers, and source runtime comparison. Existing passing
host fixtures validate their stated source equations and integration adaptations;
they do not establish traversal parity.
