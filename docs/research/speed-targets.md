# Ordinary speed target timelines

Status is cooked Blueprint and bounded native source reconstruction. Runtime
parity is pending. All locators refer to owned GR2 Steam build `16083277`,
shipping module SHA-256
`3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85`, image
base `0x140000000`. The decoded player package passed exact binary roundtrip
and VM-size checks. Private inputs are
`.work/research/16083277/player-inspection/` and
`.work/research/controller-closure/all-vm.txt`; additional field joins and bounded
native instruction evidence are under `.work/research/speed-targets/`.

## Axis gates and entry captures

Forward classification uses the raw `MoveForward` axis, not velocity or camera
angle. Its accepted branch requires axis `>= 0.5`, VM `1649`. A DoOnce gate
controls its `ForwardAcceleration` call at `2676`. The gate resets through
`ResetForwardAccelerationFlag` -> VM `25906` -> `2470` when forward movement
is unavailable or the axis reset path runs.

Negative forward axis selects the backward acceleration DoOnce at VM
`2516..2649`. Side movement selects its own DoOnce only when
`0 <= MoveForwardInput < 0.2` and `MoveRightInput != 0`, VM
`16541..16804`. These are separate backward and side latch groups, even though
both call `Left/Right/BackwardAcceleration`. A coarse Forward/Other host enum
alone does not reproduce backward-to-side restart ordering.

Zero axis or the positive gap below the forward threshold resets relevant
DoOnce gates without itself stopping a speed timeline. A continuing held
forward input therefore does not invoke `PlayFromStart` every frame, and
releasing the input does not imply resetting its elapsed time or MaxWalkSpeed.
Native `MoveForward`/`MoveRight` admission, sliding, cinematic/skill movement
and axis dispatch order remain the caller's responsibility.

`ForwardAcceleration`, VM `16119`, first stops Other acceleration and
deceleration. Unless `JustUsedSlide` is true, VM `16351..16509` captures

```text
CurrentSpeed = clamp(0.8 * length(full actor velocity), 0, 1200 cm/s)
```

The vector includes vertical velocity. `JustUsedSlide` preserves the current
field instead. The resulting `CurrentSpeed >= 1000 cm/s` check at VM `16161`
starts `Timeline_5` at `16209`; otherwise VM `16252` starts
`ForwardAccelerationTimeline`. Thus the distinct high-speed curve can be
selected by ordinary captured incoming speed as well as the slide-preserved
field. Do not silently replace this branch with the ordinary forward curve.

`Left/Right/BackwardAcceleration` -> VM `17542` first calls
`StopForwardAcceleration`, then evaluates full incoming velocity at VM
`16971`. Below `800 cm/s`, VM `17207` starts Other acceleration. At or above
that threshold it captures the full speed into `CurrentSpeed`, VM `16911`, and
starts Other deceleration at `16938`. The Hel-mode-and-falling branch pops
execution flow without starting either Other timeline. Forward has stopped;
a previously playing Other timeline can continue with its existing clock.
Preserve this no-new-timeline result instead of inventing a speed target.

## MaxWalkSpeed equations and clock selection

Speeds below are in source centimetres per second. Native getter defaults are
Forward min/max `700/1000`, Other min/max `500/700`; modifiers can change the
getter results. These defaults become `7/10` and `5/7` metres per second.

| Profile | Source target equation | Timeline metadata |
| --- | --- | --- |
| Ordinary Forward | `lerp(clamp(CurrentSpeed, ForwardMin, 3000), ForwardMax, alpha)` | Template export411, curve297, 1.5 seconds, ignores time dilation |
| High incoming Forward | `lerp(clamp(CurrentSpeed, 0, 1200), ForwardMax, alpha)` | Template export435, curve275, 1.5 seconds, respects dilation |
| Other acceleration | `lerp(OtherMin, OtherMax, alpha)` | Template export417, curve301, 1 second, respects dilation |
| Other deceleration | `lerp(clamp(CurrentSpeed, 0, 1200*(1+SpeedModifier)), OtherMax, alpha)` | Template export418, curve298, 2 seconds, respects dilation |

Writer VM ranges are `35811..35973`, `35209..35339`, `38520..38631`, and
`40347..40593` respectively. Ordinary Forward suppresses its MaxWalkSpeed write
while `LandingForwardAccelerationTimeline.IsPlaying` or `IsOnWall`, VM
`35719..35811`. Its clock still belongs to the timeline. The high incoming
Forward writer has its own callback and does not share these guards.

The already recovered Forward and Other acceleration curves are in
`src/curves.rs`. High incoming Forward curve275 is unweighted cubic:

| Time | Value | Arrive tangent | Leave tangent |
| --- | --- | --- | --- |
| 0 | 0 | 0 | 0 |
| 0.6307741403579712 | 0.09813332557678223 | 0.35970187187194824 | 0.35970187187194824 |
| 1.149470329284668 | 0.4134666323661804 | 1.0375516414642334 | 1.0375516414642334 |
| 1.5 | 1 | 2.146879196166992 | 2446.29150390625 |

Other deceleration curve298 has linear keys `(0,0)` and `(1,1)`. The authored
timeline lasts two seconds; its curve reaches the final target after one
second and retains it for the remaining interval. Preserve the difference
between timeline length and the final keyed time.

## CurrentSpeed writers and stop behavior

`CurrentSpeed` is a native PlayerCharacter property at offset `+0x704`, proven
by generated field record RVA `0x53c95f0` and its exact name pointer. It is not
a Blueprint-declared private cache. Native constructor `0x1c4c470` initializes
it to `170 cm/s`; ordinary Forward immediately clamps it against ForwardMin.
The five direct writers in the decoded player bytecode are:

| VM offset | Writer |
| --- | --- |
| 16509 | Forward entry captures clamped `0.8 * full speed` |
| 16911 | Other deceleration entry captures full speed |
| 17491 | StopForwardAcceleration resets to ForwardMin when modifier is not positive |
| 23067 | Slide-end context writes `999 cm/s`, marks JustUsedSlide, and schedules a 0.2 second delay |
| 24153 | StopAccelerationTimelines resets to ForwardMin and writes MaxWalkSpeed directly |

These captures are event-bound. Forward and Other deceleration updates read
`CurrentSpeed`; they do not rewrite it from observed velocity each tick.
StopForwardAcceleration, VM `17240..17518`, resets it only when
`GetSpeedModifiersMultipler` is not positive, then stops ordinary Forward and
high incoming Forward timelines. With a positive modifier it stops the
same timelines while retaining the field. Stop does not directly assign a
new MaxWalkSpeed. Other stop callbacks simply stop their respective timelines.
All four Finished callbacks pop execution flow without restarting a clock.

Bounded disassembly also found native CurrentSpeed writes. Leaf function
`0x1c54b00` applies an additive/subtractive speed factor to CurrentSpeed and
MaxWalkSpeed. Native `0x1c5707d` writes incoming setter data. Routine
`0x1c86080` multiplies CurrentSpeed and min/max fields by `1 +` the current
speed modifier. A setter/reset route at `0x1c6da73` rewrites the same field
value. These are additional context mutators, not evidence of an ordinary
per-tick copy from actor velocity. Their callers and activation conditions are
not closed by this slice. A host should expose the captured field for explicit
modifier/slide context changes rather than claim it is globally immutable.

The supplied `0x1c84240` export uses the PlayerMovement-style component layout;
its TickComponent arithmetic does not establish a PlayerCharacter `+0x704`
writer. It was not treated as the player field's owner merely because it ticks.

## Host implementation boundary

`src/speed.rs` contains pure target equations, captured state and all four
curves. `classify_input` reports Forward, Other or NoNewTimeline. Callers own
raw-axis DoOnce latches, movement admission, wall/landing write suppression,
native modifier consequences and schedule order. `advance` accepts separate
unscaled and scaled deltas so the authored dilation flags remain explicit.
Unchanged means preserve the existing motor speed target, not choose zero or
reset to the default run speed. This arithmetic does not provide UE frame-order
or source-feel parity by itself.
