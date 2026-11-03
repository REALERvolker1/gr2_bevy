# Vulkan presentation validation

This investigation runs the Bevy fixture, not the original game. Headless GR2
testing under Xvfb/gamescope is handled by the separate
[gr2_headless](https://github.com/REALERvolker1/gr2_headless) project.

The Xvfb run preserved in `.work/render-diagnosis/baseline.log` produced a real
PBR screenshot and completed the scripted controller actions, but its renderer
validation was not clean. The adapter was the host's Vulkan GPU. The pinned
crates are Bevy0.19.1 and
wgpu/wgpu-hal29.0.4. This is a host-renderer investigation, independent of GR2
controller source parity.

The log contains two startup `VUID-VkPresentInfoKHR-pImageIndices-01430` reports:
swapchain images were presented in `VK_IMAGE_LAYOUT_UNDEFINED`. Three later
`VUID-vkAcquireNextImageKHR-semaphore-01286` reports show reuse of acquire
semaphores that remained signaled. The first two semaphore reports immediately
follow startup; the third follows screenshot saving. A saved image does not
make these errors acceptable or establish a clean Vulkan run.

## Pinned-source failure path

The local Cargo registry source establishes a concrete missing-pipeline path:

1. `bevy_core_pipeline-0.19.1/src/upscaling/node.rs:63` obtains the output color
   attachment before checking whether the upscaling pipeline is ready.
2. `bevy_render-0.19.1/src/texture/texture_attachment.rs:157` marks that attachment
   used when its attachment descriptor is requested. `needs_present()` reads
   the same flag; it does not confirm a submitted render pass.
3. `upscaling/node.rs:76` returns when the asynchronous pipeline is unavailable.
   Its fallback clear pass is compiled only on macOS/iOS. On Linux this path
   can mark the output used without beginning any pass against it.
4. `bevy_render-0.19.1/src/renderer/mod.rs:98` presents windows when their camera
   output reports `needs_present`, or when an initial present is required.
   Meanwhile `bevy_core_pipeline-0.19.1/src/schedule.rs:252` excludes camera
   windows from the no-camera clear pass, even if their final pass was skipped.
5. `wgpu-hal-29.0.4/src/vulkan/swapchain/native.rs:542` obtains the present-wait
   semaphore list, then resets the acquire semaphore's bookkeeping. Presenting
   an image with no GPU work referencing it provides neither the required image
   layout transition nor the normal submission that consumes its acquire
   semaphore. This explains the observed sequence; attribution to this precise
   runtime branch still requires an intervention run.

Screenshot handling contains another missing-pipeline path:
`bevy_render-0.19.1/src/view/window/screenshot.rs:607` only creates the
screenshot-to-screen render pass if its specialized pipeline is available.
The screenshot captures into a separate texture, so a successful readback alone
does not prove that the associated window image was rendered and presented
correctly. The later semaphore report is consistent with this mechanism, but
the log alone does not establish its precise first cause.

## Intervention results

Synchronous pipeline compilation alone reduced the baseline reports but left
one startup layout error and one startup semaphore error. Its log, screenshot
and controller trace are preserved under `.work/render-diagnosis/synchronous-*`.
Deferring `needs_initial_present` also left these errors; its artifacts are
preserved under `.work/render-diagnosis/deferred-*`. Neither was accepted as a
clean renderer result.

The application now keeps synchronous pipeline compilation and submits a clear
pass for an acquired image whose `needs_initial_present` flag is true. The clear
runs after `RenderSystems::Prepare` and before `RenderSystems::Render`, when the
surface texture exists. It follows the pinned engine's existing uncovered-window
clear pattern, but also covers the initial camera window. Normal camera and
screenshot rendering retain their original paths. Vulkan validation stays enabled;
no driver, presentation mode or dependency version changed.

The fresh Xvfb run on 2026-10-13 UTC used the same Vulkan adapter and driver.
It completed 2,132 fixed controller steps, satisfied the ground dash, slide,
jump, air press/release and wall-stop assertions, and saved an
inspected 1280x720 course screenshot. The complete log contains no `ERROR` or
`VALIDATION` entries. Artifacts are `.work/validation/xvfb.log`, `xvfb.png` and
`controller.csv`. `scripts/test-xvfb.sh` now fails when the render log contains
an error, even if a screenshot was saved.

This intervention validates the application workaround on this pinned Linux/X11
configuration. It does not establish the exact first failing runtime branch,
behavior on other render backends, or GR2 controller parity.

## Ledge port regression checks

Fresh runs on 2026-10-13 UTC used the same Vulkan adapter and driver.
Both logs contain no `ERROR` or `VALIDATION` entries, and both inspected screenshots
are 1280x720. The application now uses the cooked main capsule dimensions and
camera offset. The end wall is authored taller to keep the wall-stop course
separate from automatically admitted ledges.

| Course | Fixed steps | Observed action results | Final position in metres |
| --- | ---: | --- | --- |
| `--ledge-smoke` | 2230 | Jump1, mount1, mount finish1 | (3.000040,0.962000,-7.766550) |
| `--smoke` | 2150 | Ground dash2, slide1, jump1, air press1, air release1 | (-0.000297,0.962000,-24.247999) |

The ledge course starts mounting at step 27, zeros velocity on entry, releases
forward input during the trajectory and finishes at step 45 with planar velocity
(0,-10) m/s. The host applies gravity on that finish tick, producing vertical
velocity -0.245 m/s. This records the host schedule; it does not establish the
original Unreal physics order. See [ledge source boundaries](ledge-mounting.md).

The focused test run passed 34 tests across controller, grapple_motor, ledge,
ledge_motor, mantle and motor. It includes real Avian fixtures for automatic
ascent mounting, BlockClimb query success without initialization, long overhead
rejection, world-clock permission delay and preservation of a pending Delay
after a second mount. Pure fixtures check source phase boundaries and floating
point addition order. The complete unrelated suite was not rerun for this slice.

`scripts/test-xvfb.sh` accepts either course and archives screenshot, log and CSV
under `.work/validation/ledge/` or `.work/validation/controller/`. The command
logs and focused test output remain private in `.work/validation/`. Original GR2
was not launched; interactive source testing remains gated unavailable.
