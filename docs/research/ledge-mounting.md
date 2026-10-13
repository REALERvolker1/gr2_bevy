# Native ledge mounting

Static reconstruction from owned GR2 Steam build 16083277, executable SHA-256
`3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85`, image base
`0x140000000`. All native addresses below are RVAs. Blueprint offsets are VM
coordinates, export indices are one-based. No original-game runtime comparison
was performed. The Xash prototype is not the implementation authority.

The native `UClimbComponent` trajectory is separate from the 400 cm trajectory
in `BP_WallClimbing`. Existing component/class joins and configuration are in
[traversal-source.md](traversal-source.md). The additional bounded recipe is
`research/ledge-mount-scope.tsv`; original native exports and source identity
receipts remain private in `.work/decompiled/16083277/ledge-mount-01/` and
`ledge-mount-02/`. Bounded original-binary Delay disassembly and its hash manifest
are in `.work/research/ledge-coordinator-audit/`. Package manifests are in
`.work/research/16083277/player-inspection/manifest.json` and
`.work/research/traversal-closure/wall-blueprint/manifest.json`. Both passed exact
binary roundtrip and VM-size checks. Player uasset/uexp SHA-256 values are
`8c2fdb49e128b93175fc64990083c5c529a903ea90322f9069e805349454eb65` /
`d54e5545d7b3ac01cd81446831d6c1cf5c652e90ae1f90069b9c694d381f7cea`;
WallClimbing values are
`507dfececb8912c728e46dae4b032933eb11065250fab6a5453e73d27a50e69e` /
`830f1ce58e74e8e567e96b53b6b97d2fe3388aea9da4f16e3251ef9bcf41be96`.

## Capsule and camera

Player package `CapsuleComponent` resolves to export263 `CollisionCylinder`,
class import-144 `CapsuleComponent`, template import-119 under
`Default__PlayerCharacter` import-863. It explicitly sets radius50 cm and
half-height96 cm. Export262 `InAirCapsule` has half-height60 cm and is a separate
component. The Jack fixture uses the main capsule, with no inferred engine
constructor dimensions.

`FollowCameraRef` is a `UCameraComponent`, joined through generated field
RVA0x53cb1b8 and type getter0x3e33f20. ConstructionScript VM0 assigns it from
`FollowCamera`. SCS node export336 names this component, attaches it to the native
`CollisionCylinder` and references template export261. The template location is
UE(-10,0,70) cm, with pawn control rotation enabled. The Bevy actor-local offset
is `(0,0.70,0.10)` metres. Source classification consumes the camera's current
world position. Dynamic camera offset, crouch and head-bob writers are not closed.

`AdjustToCapsule` body0x1a1c660, called at player VM27414, overwrites
`MaxWallForward = 1.5 * scaled_radius` and
`AdditionalHeightOffset = 1.5 * scaled_half_height + 20 cm` once during setup.
Scaled radius uses min(scaleX,scaleY); half-height uses scaleZ. The fixture uses
unit scale and supplies already scaled dimensions. Runtime resize callers remain
open; this port does not invent a per-attempt adjustment.

## Ordered queries

TryClimb0x1a49ac0 calls helpers0x1a25440/0x1a259f0. World sweep0x3824b60 closes
the shape ABI. Quaternion RVA0x6395e90 is identity; up RVA0x6395dc0 is UE Z-up.
The following geometry is converted to Bevy Y-up metres. Let `r` be scaled
radius, `h` scaled half-height, `M` MaxClimbHeight3, `d` normalized planar global
player0 actor forward, `P` component-owner position and `C` cached character
position. These positions need not be the same in source.

| Order | Shape | Start | End | Result |
| --- | --- | --- | --- | --- |
| 1 | Capsule r,h | C | P + up*(2h+M) | Any block rejects |
| 2 | Box half-extents (r, BottomLedgeMaxDist+0.5, r) | P + up*(M/2-1) | start + d*MaxWallForward | First wall block required |
| 3 | Sphere r | wall ImpactPoint + up*M | wall ImpactPoint | Top block required |
| 4 | Capsule r,h | top ImpactPoint + up*(h+0.5) | start + d*(r+LedgeDepthCheckOffset) | If clear, accept clearance |
| 5 | Capsule r,h | previous start + up*0.5 - d*r | start + d*(2r+LedgeDepthCheckOffset) | Run only if order4 blocks; block rejects |

The overhead trace always uses channel0. Other probes use the component's cooked
`ECC_WorldStatic` channel0. Ignored actor helpers0x37e7670/0x37e76d0 exclude the
owner. There is no added top-slope threshold, height band, or requirement that
wall and top hits share an actor. Initial overlaps and contact generation still
depend on the collision backend.

The first wall hit rejects actors derived from EnemyBaseCharacter getter0x1e0d7d0,
BaseProjectile0x1dc88b0, ShurikenProjectile0x1ee90f0 or SurgeWaveElement0x1ef8040.
It does not search behind a rejected actor. Ordinary facing uses wall `Normal`
at HitResult+0x24, rather than `ImpactNormal` at+0x30:

```text
deviation = 180 - acos(clamp(dot(normalized_planar_camera_forward, wall.Normal), -1, 1)) * 57.295776
reject when deviation > 20 degrees
```

Forced calls bypass that facing guard. Top coordinates use `ImpactPoint` at
HitResult+0x18, rather than `Location` at+0x0c. A selected top actor derived from
StaticMeshActorOML getter0x1ef2c20 rejects if its unnamed flag at+0x230 is false.
Actor or component `BlockClimb` tags suppress initialization but retain a true
TryClimb query result. The tag is ASCII at RVA0x5173c28.

## Query success and component initialization

A blocking query result and active mounting are separate outcomes. Native origin
helper0x1a287e0 prefers a valid hit actor's root; it only falls back to the hit
component when the actor is invalid. A valid actor without a root, or a world-zero
root origin, prevents initialization. Tags can also prevent it. Neither case
converts the successful query to false.

Initialization stores the source climb point before the zero-origin check, then
captures S, T, D, F, d and previous origin as documented in traversal-source.md.
It sets component active and writes movement velocity zero once. It does not
clear the old interpolation alphas on entry. Ordinary permission guards prevent
normal reentry. Source has no positive epsilon for D or F; the Rust pure boundary
rejects invalid/nonpositive geometry explicitly as a host adaptation.

Update wrapper0x1a442c0 reaches hot body0x1a4c5d0. It consumes world delta and actor
custom dilation, and advances exactly one phase per tick. Lift reaches its target
before the forward phase starts on a later tick. Forward uses the current actor
height and the increment in surface origin since the previous forward tick.
Native float addition order is retained:

```text
B = (saved_start.x, current_actor.y, saved_start.z)
target = (current_origin - previous_origin) + F*d + B
position = (target - B)*forward_alpha + B
```

Subtracting origins before adding B matters for distant static objects; adding
and subtracting a large origin around B can lose its height in f32. The final
velocity on the tick after forward reaches1 is current actor forward*1000 cm/s.
The source position helper0x36fe260 receives sweep=false, no hit output and
teleport=None. Hot-body instructions at0x1a4c8ce..0x1a4c8d9 qualify those flags.
The motor therefore accepts an unswept absolute position request.

Finish0x1a310c0 clears active, both alphas, saved start/lift/forward vectors and
HitResult. It retains D/F, previous origin and surface references. It does not
restore the player's climb permission or clear the assigned finish velocity.

## Player admission, events and permission

Player TryClimbing0x1c828b0 is the coordinator. Ordinary calls require valid input
and raw positive forward axis, the global-controller predicate0x1c2dbf0,
`bCanClimbOnJumped`, `bCanUseClimbing`, and neither sliding nor custom/native
crouching. Forced calls bypass the forward-axis and component-facing guards,
not the other permissions. Player VM2749..3439 calls it when `bCanMoveInAir` and
movement `IsFalling` hold. This includes ascent and requires no held jump.
Wallrun direction probing0x1c75680 is another ordinary caller. The component's
cooked `bShouldAutoActivateOnFalling` property has no independently proved
consumer here.

On a true component result, even without initialization, the player invokes
ResetJump and stops wallrun, then disables `bCanUseClimbing`. ResetJump event
wrapper0x1eba290 resolves through registration0x53d4040 to exec0x1e0ff60 and
body0x1c76e20. It clears bJumped, JumpCounter and FallingSpeed. It does not refill
air readiness, zero movement velocity or change movement mode.

High presentation is selected when stored climb-point height minus current
FollowCamera world height is strictly above NormalLedgeHeight100 cm, or forced.
Otherwise Low is selected strictly above LowLedgeHeight0. Equality100 chooses
Low; equality0 chooses neither. Both high and low player events VM15677/15789
join VM15713 and BP_WallClimbing.ClimbEdgeCon VM2831. This sets ContinueWithJump,
clears ShouldWallJumpOnRelease, allows rotation and stops the special wall-climb
timeline. It does not start that timeline's 400 cm motion.

VM2983 computes `1/CustomTimeDilation`; VM3047 calls ordinary `Delay` with stable
Self callback target, UUID -532948934 and linkage83. Native name registration
RVA0x5ad1b50 reaches exec0x3ecfef0, then body0x3a10f30. The body looks up target+UUID
at0x3a10f90 and returns immediately if an action already exists. It only stores
new duration at0x3a10fcb for an absent action. A second mount therefore preserves
the pending deadline. The same package uses distinct `RetriggerableDelay` at
VM303. Completion VM83 clears bIsClimbingWall and, after PlayerRef validation,
VM155 jumps to VM49 to restore bCanUseClimbing.

Neither-presentation leaves that timer unscheduled. PlayerLanding0x1c76c20 also
restores climb permissions. Cooked player CDO initially overrides bCanUseClimbing
to false, despite native constructor true. The fixture maps landing to a newly
grounded motor transition; other player initialization/jump writers remain open.

## Host boundaries and validation

Avian supplies the probes and one motor owns integration. Fixture colliders all
block the configured channel, metadata represents source classes/tags, and one
entity Position represents the hit actor/root. This is not the full UE class,
collision response, attachment or component hierarchy. Avian point1/normal1 map
to UE ImpactPoint/Normal; PhysX contact equivalence is unverified. Global player0,
component owner and cached character are aliased to the fixture's single player.
The empty fixture assumes the global controller/action and bCanMoveInAir gates
allow movement. Optional native camera-reset callbacks, montage playback and
the shared Blueprint presentation flags are not simulated beyond the recovered
High/Low classification and permission delay. The host resets its jump count;
source bJumped/FallingSpeed have no separate host fields.
Portable vector normalization retains the source dimensionless squared threshold
1e-8 but uses Rust sqrt rather than native reciprocal-square-root refinement.

The host suppresses its physical integration on an absolute pose-request tick.
No gravity-scale, movement-disable or tick-disable write was recovered in native
entry/update/finish. Inherited movement TickComponent0x3848360 and callback
0x1c789d0 do not close all delegated physics/animation ordering. Holding native
mount poses and preventing a second host integration are integration policies,
not proof that Unreal freezes gravity during mounting. Clocks map world delta
to fixed dt*global dilation and actor factor to custom dilation.

Focused tests check ordered probe geometry, first-hit rejection, tag/query-result
separation, strict camera thresholds, phase arithmetic and float order. Actual
Avian fixtures check ascent admission without held jump, velocity/reset effects,
long overhead rejection, world-clock delay, duplicate pending delay and unswept
pose ownership. These validate the stated reconstruction and host mappings.
The rendered `--ledge-smoke` course tests one complete mount under Xvfb.
Original runtime parity, moving actor/root hierarchy, camera effects, special
Blueprint climbing and full action-to-physics scheduling remain unverified.
