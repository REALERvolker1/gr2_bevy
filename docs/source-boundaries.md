# Source and host boundaries

The authority is the owned GR2 Steam build 16083277, executable SHA-256
`3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85`.
Original native bodies and cooked Blueprint bytecode are the movement source.
No Xash movement implementation was translated. See
[the native/Blueprint specification](research/controller-source.md) for equations,
RVA/property/VM locators and the new bounded native export manifest.

## Implemented static reconstruction

| Rust module | Recovered behavior | Authority |
| --- | --- | --- |
| resources | Negative clamp -150, reciprocal HUD fraction, starting-sign regeneration, audio cooldown, strictly positive dash admission, ordered resolved cost terms | SetStamina 0x1cac1c0, Tick 0x1cafff0, dash 0x1cb0cf0 |
| slide | Exact ordered cooked flat/slope bands, strict endpoints, input threshold, height-difference downhill discriminator | 0x1c68f60/0x1c69330, original BP_SlideLaunchPlayerDatabase |
| launch | Independent XY/Z override/add flags and air-release vector equation | concrete LaunchCharacter 0x37f9e70, air end 0x1c5b750 |
| curves | Original dash, forward-speed and other-speed unweighted cubic keys | player Blueprint curve exports287/297/301 |
| locomotion / movement | Ordinary walking acceleration, directional friction, braking substeps, pending impulse/force before launch | 0x382baa0,0x382af20,0x3838620 |
| speed | Four captured-speed target profiles, authored dilation flags | player VM16119..17542 and curve275/297/298/301 |
| grapple / mantle | Fixed-point grapple curve, committed-pose release box and launch; native two-phase mantle geometry, moving-origin term and finish velocity | research/grapple-source.md, research/traversal-source.md |
| controller | Ground range classification/defaults, dash play rate/curve, end velocity-change impulse, first-jump11m/s with XY preservation, air hold periodic expenditure and release impulse/minimum, ground coyote default.3s | 0x1c5a410,0x1c6c180,0x1c60dc0; qualified player Blueprint |

Campaign resource values use the cooked stamina overrides40/.2 rather than
native constructor defaults50/.25. Native defaults supply maximum100 and
regeneration35/20. The fixture has an empty effective loadout. DashModifiers
accepts resolved coefficients; it does not invent effective membership from UI
socket placement. Energy conversion and upgrade consumers are not connected.

## Deliberate host policies

These are integration choices with explicit comments, not established UE parity:

- Simulation runs at120Hz. The audio clock advances independently in fixed,
  unpaused seconds. World/player factors enter resource arithmetic explicitly.
- Avian's fixed physics step refreshes collider trees, then motor sensing, action
  decisions, motor movement, and native writeback run in that order. Static
  source closes impulse/force-before-launch consumption, but does not establish
  this exact UE action/resource/physics phase order.
- Motor Position is authoritative and capsule movement has one integration owner.
  Positional dash displacement is independent of physical momentum. Avian sweeps
  the displacement; source Timeline_4 teleports with sweep=false. The native
  dash endpoint's full ten-iteration collision/step/class policy remains open.
- Gravity uses world980cm/s² times recovered native scale3. World gravity itself
  is configurable through PhysicsVolume/config and remains an effective-value
  dependency. Ordinary walking uses recovered friction/braking and cooked
  acceleration. Four speed timelines drive the target with separate forward,
  backward and side entry latches. The fixture supplies raw keyboard axes;
  world-only callers use an adapted axis projection. Timeline advancement during
  non-locomotion actions, landing guards,
  slide-end CurrentSpeed writes and native upgrade writer activation remain open.
  Falling steering is an explicit host acceleration policy.
- Jack uses the cooked main capsule radius50 cm/half-height96 cm and camera
  relative location(-10,0,70) cm. Skin, floor slope threshold and crouch clearance
  remain host parameters. Resizing preserves feet. Standing requires clearance.
- First jump and.3s coyote admission implement the ordinary unupgraded branch.
  Motorcycle, collision-clearance and alternate jump-type branches remain
  source dependencies. Fixed post-query updates reset the edge timer/jump count.
- Slide launch replaces horizontal velocity and keeps vertical velocity per the
  now-resolved LaunchCharacter flags. The host uses flat data. Slope transforms,
  steering, sentinel classification and repeated update semantics remain open.
- Air release uses the recovered impulse after periodic.01s sensory debit and
  applies the minimum remainder while airborne. Sensory world slowdown and its
  player compensation, effect timing, camera material and timer-manager clock
  remain unresolved; the fixture uses factor one. Landing readiness reset is a
  host mapping to the recovered ResetDash operation.
- Active block toggles the recovered regeneration gate; combat/parry are outside
  this controller fixture. Regeneration runs after actions. Pure tests retain
  both competing source action/tick schedules.
- Respawn increments lifecycle generation and clears action/resource deadlines.
  Full original death/possession/save-load behavior remains open.

## Validation limits and remaining work

Headless tests exercise actual Avian floors, walls, corners, slopes, ceilings,
ledges and blocked standing. Pure fixtures check source arithmetic, strict band
holes, Hermite curves, launch flags and lifecycle. The Xvfb run renders the
fixture, records committed per-step poses/resources/results, asserts action
coverage, and saves a real window screenshot.

None of these establishes a repeated GR2 source runtime baseline or controller
parity. Interactive GR2 testing remains unavailable under the prior project gate.
Full wall attach/edge/uplift, special Blueprint climbing, grapple target classes
and montage timing, slope/wavedash collision, complete upgrades, sensory
compensation/presentation and UE action-to-physics scheduling remain required.
Research findings remain static until an appropriate source oracle confirms
their behavior.

## Wallrun integration

The tagged test walls use source speed/angle/view guards, source wall-normal
rotation, captured-speed clamp, the two-second cubic slowdown and additive
wall-jump launch. See [traversal research](research/traversal-source.md).
`WallRunnable` is an authored host surface marker replacing the game's component,
class and `BlockWallrun` tag classifier. Side rays replace native overlap/probe
selection. Collision projections, horizontal constraint timing, attach/detach
cooldown clock and wall-jump counter handling are host policies. Source uplift,
wall-edge launch/coyote, arbitrary moving walls and the complete reset caller
closure remain open. This bounded integration does not establish wallrun parity.

## Grapple and mantle integration

E selects a visible authored grapple anchor. Screen ranking and Avian line of
sight are host selection policies. The ordinary source curve moves toward its
captured endpoint; the motor sweeps that displacement. Release checks the actual
committed capsule pose against the source box, then queues the full override
launch for the next movement phase. Point cooldown/reset delays use the fixed
audio clock. Montage, nonordinary point classes and cancellation timing remain
source dependencies. See [grapple findings](research/grapple-source.md).

Ordinary blocking geometry uses the recovered ordered capsule/box/sphere ledge
queries, class/tag filters, facing guard and retry geometry. Query success is
separate from motion initialization. Automatic admission includes ascent with
positive forward input and no held jump. The native two-phase component uses
unswept absolute poses, with source reset, camera thresholds and nonretriggering
permission delay. The motor suppresses its own physical integration on pose
request ticks; native gravity/physics ordering remains unresolved. Avian contacts,
source actor/root hierarchy and world-clock scheduling are host mappings.
See [ledge mounting](research/ledge-mounting.md) for qualified findings and tests.
Special Blueprint wall climbing remains separate and unimplemented.
