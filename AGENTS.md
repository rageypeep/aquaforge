# AGENTS.md — AI assistant context for AquaForge

AquaForge is an underwater voxel sandbox in Rust + Bevy 0.18. Think
"Minecraft, but the whole world is a seafloor." This file is written for
AI coding assistants (Devin, Claude, Cursor, Copilot-Workspace, etc.) —
keep it current whenever the repo's layout, commands, or conventions
change.

The human-facing feature list and prioritised backlog lives at
[`docs/index.html`](docs/index.html) — always read that before starting
a feature task.

## Stack

| | |
|---|---|
| Language | Rust (2024 edition) |
| Toolchain | **Rust 1.89** — pinned in `rust-toolchain.toml` (Bevy 0.18 MSRV) |
| Engine | Bevy 0.18.1 |
| Graphics | wgpu, PBR `StandardMaterial`, custom `ExtendedMaterial` for water |
| Windowing | winit |
| CI | GitHub Actions — `cargo check`, `cargo build`, `cargo test` on `ubuntu-latest` |
| Target | Native desktop (Linux / macOS / Windows). No WASM yet. |

## Commands

```bash
# First-time Linux system deps (Bevy needs these):
sudo apt install pkg-config libwayland-dev libxkbcommon-dev \
  libasound2-dev libudev-dev libx11-dev libxcursor-dev \
  libxi-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev

# Build & run (pinned Rust will be fetched by rustup on first invocation):
cargo run --release

# Verify exactly what CI runs:
cargo check
cargo build
cargo test

# Run a single module's tests:
cargo test chunk::tests

# NOTE: aquaforge is a *binary* crate, not a lib. `cargo test --lib` errors.
```

Clippy is **not** enforced in CI — `cargo clippy --all-targets -- -D warnings`
may report pre-existing findings (`too_many_arguments`, `type_complexity`,
etc.) on the chunk mesher and systems. Don't chase those as part of a
feature PR; if you want to address them, do it in a dedicated lint-only
PR so the diff stays reviewable.

## Repo layout

```
aquaforge/
├── assets/
│   └── shaders/
│       ├── water.wgsl       # Animated sea-surface vertex shader (PR #11)
│       ├── caustics.wgsl    # Chunk-material fragment: animated caustics (PR #17)
│       └── god_rays.wgsl    # Fullscreen post-process: radial light shafts (PR #17)
├── docs/                    # Human-facing progress page (GitHub Pages target)
│   ├── index.html           # Backlog + shipped features — UPDATE when adding PRs
│   └── screenshots/
├── src/
│   ├── main.rs              # App bootstrap, plugin wiring
│   ├── game/
│   │   ├── mod.rs           # GamePlugin — composes world + edit plugins
│   │   ├── blocks.rs        # BlockType enum, opacity, per-type vertex colour
│   │   ├── chunk.rs         # 16³ Chunk, seabed generator, greedy AO mesher
│   │   ├── chunk_map.rs     # HashMap<IVec3, Entity> + world-block helpers
│   │   ├── edit.rs          # Voxel DDA raycast, break/place, target highlight
│   │   └── world.rs         # 6×2×6 chunk grid spawn, water-level constant
│   ├── rendering/
│   │   ├── mod.rs           # AtmospherePlugin: ambient, fog, HDR, sea plane
│   │   ├── atlas.rs         # Procedural block texture atlas (BlockAtlasPlugin)
│   │   ├── headlights.rs    # Toggleable sub-style spot-light headlights
│   │   ├── lighting.rs      # PBR rig: cascaded shadows, tonemapping
│   │   ├── water.rs         # ExtendedMaterial + MaterialPlugin for water
│   │   ├── shaders.rs       # (stub, reserved for future custom materials)
│   │   └── ui.rs            # Oxygen HUD meter (HudPlugin)
│   ├── systems/
│   │   ├── mod.rs           # ControlsPlugin (composes InputPlugin + SubPlugin)
│   │   ├── input.rs         # Cursor grab / release
│   │   └── sub.rs           # Sub controller, swept-AABB collision, O2 reserve
│   └── utils/
│       ├── mod.rs           # Re-exports math + noise
│       ├── math.rs          # smoothstep, bilerp
│       └── noise.rs         # Dependency-free value-noise + fBm
├── AGENTS.md                # (this file)
├── Cargo.toml
├── README.md
├── rust-toolchain.toml      # Pins Rust 1.89
└── .github/workflows/rust.yml
```

## Architecture

- **Plugins are the seams.** `main.rs` installs three top-level plugins —
  `AtmospherePlugin`, `GamePlugin`, `ControlsPlugin` — and each of those
  composes smaller plugins. New features should be added as a plugin
  wired into the nearest aggregate, not poked directly into `main.rs`.
- **Chunks are authoritative voxel data.** A `Chunk` is a 16³ grid of
  `BlockType` stored as a linear `Vec<BlockType>`. `Chunk::build_mesh`
  emits one `Mesh` per chunk using a **greedy coplanar-face mesher**
  with per-vertex ambient occlusion baked into vertex colours.
- **`ChunkMap` is the only way to translate world coords.** Use
  `world_block_to_chunk(world_block) -> (chunk_pos, local)` and
  `ChunkMap::get(chunk_pos)` to find the `Entity` holding a chunk's
  components. Anything that edits voxels (`edit.rs`, future inventory /
  save-load / networking) goes through this.
- **Rendering is PBR + fog.** `AtmospherePlugin` configures the main
  `Camera3d` with HDR, natural `Bloom`, exponential `DistanceFog`,
  tonemapping, and cascaded shadows. The sea surface is a separate
  tessellated `Plane3d` driven by `WaterMaterialPlugin` (an
  `ExtendedMaterial<StandardMaterial, WaterMaterialExt>` with a custom
  vertex shader in `assets/shaders/water.wgsl`).
- **Chunk material is an `ExtendedMaterial` too.** Since PR #17, chunks
  render with `ChunkMaterial = ExtendedMaterial<StandardMaterial,
  CausticsMaterialExt>`. The extension's fragment shader
  (`assets/shaders/caustics.wgsl`) adds a procedural sunlight-caustics
  term to the PBR emissive so shadowed crevices still pick up the
  projected streaks.
- **God-rays are a Core3d post-process.** `GodRaysPlugin` (PR #17)
  registers a `ViewNode` between `Node3d::Tonemapping` and
  `Node3d::EndMainPassPostProcessing`; `GodRaysSettings` on the camera
  carries the sun's screen-UV position, updated from the `SunLight`
  marker each frame.
- **Input uses `CursorGrabMode::Locked`.** `systems/input.rs` is the
  single source of truth for the locked state: left-click grabs, Esc
  releases. The sub controller (`systems/sub.rs`) reads `WASD`,
  `Space`/`LShift`, `LCtrl` boost, and yaw/pitch from mouse motion
  only while the cursor is locked, and `edit.rs` hooks into the same
  grabbed state for break/place and digit-key slot selection.
- **Sub physics live in pure functions.** `systems/sub.rs` isolates
  swept-AABB resolution (`resolve_collisions`), wish-vector composition
  (`wish_direction`), and O2 bookkeeping (`step_oxygen`) from Bevy
  so each has focused unit tests. The plugin-level systems are thin
  glue on top.

## Conventions

- **No bespoke voxel crate.** Value-noise, fBm, and the chunk mesher are
  all in-tree (`utils/noise.rs`, `game/chunk.rs`). Don't pull in a
  dependency for what's already ~50 lines of hand-rolled math.
- **Imports at the top of every file.** Never `use` inside a function.
- **Comments describe intent, not the diff.** If a comment only makes
  sense to someone reading the PR — e.g. "fixed bug X", "previously
  this did Y" — delete it. Put that context in the PR description.
  Reserve comments for non-obvious invariants (coordinate conventions,
  GPU layout alignment, face-winding direction).
- **Keep PRs focused.** One feature per branch, even when several are
  queued up. Formatting-only churn goes in its own PR.
- **Cross-chunk face-culling is a known pre-existing limitation.**
  Edits at a chunk boundary may leave a neighbour's mesh stale until
  that neighbour is remeshed. Don't silently "fix" this as a side
  effect of another feature — it warrants its own PR.
- **Match the repo's existing `rustfmt` output** (default settings).
  Pre-commit hooks are not configured, so run `cargo fmt` before
  pushing.

## Branching & PRs

- Default branch: `master`.
- Work branches use the `devin/<timestamp>-<slug>` convention.
- Always base feature branches off `master`, not off an open PR branch.
- PRs auto-get a Devin session link appended to the description — don't
  add one manually. Preview / Devin Review comments appear inline; read
  them before marking the PR ready.
- CI must be green before merging: `cargo check`, `cargo build`,
  `cargo test` + GitGuardian + Devin Review.

## Testing notes

- **`cargo test` runs unit tests as part of the bin crate.** Example:
  `src/game/chunk.rs` has a `#[cfg(test)] mod tests` block exercising
  the greedy mesher, AO baking, and face-visibility predicates. Tests
  live alongside the code they cover — when a new module lands with
  non-trivial logic (e.g. the `Inventory` resource coming in PR #7),
  add its own `#[cfg(test)] mod tests` in the same file.
- **GUI end-to-end testing on Linux VMs:** boot the release binary
  under lavapipe with
  ```bash
  DISPLAY=:0 LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND=vulkan \
    VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
    ./target/release/aquaforge
  ```
  then maximise with
  `wmctrl -r AquaForge -b add,maximized_vert,maximized_horz`.
- **X11 cursor-grab trap (important):** the game uses
  `CursorGrabMode::Locked`, which calls `XGrabPointer` on X11. That
  **drops** `XTestFakeButtonEvent` injections (i.e.
  `xdotool click 1 / 3` don't reach Bevy as mouse-button events once
  the cursor is grabbed — camera rotation still works because raw
  `DeviceEvent` motion deltas pass through). Keyboard injection via
  `xdotool key` **does** work. For click-triggered logic, prefer unit
  tests against the underlying resource API that mirror the call
  sequence in `src/game/edit.rs` — see the `Inventory` tests in PR #7
  for the pattern.
- **Raycast silent no-op:** `edit.rs` early-returns when
  `target.face_normal == IVec3::ZERO` (ray miss). If you're scripting
  a placement test, pitch the camera down first with
  `xdotool mousemove_relative -- 0 400` so the crosshair is on a real
  block face before right-clicking.

## When adding a feature

1. Read `docs/index.html` → "Recommended next features" to confirm
   priority and anchor module.
2. Branch off `master` as `devin/<timestamp>-<slug>`.
3. Add the feature as a new plugin (or extension of an existing one).
   Update the relevant `mod.rs` to wire it in.
4. Add unit tests for any pure logic (inventory, noise, math, mesher
   predicates). GUI-observable behaviour stays manual.
5. Update `docs/index.html`:
   - Add a `<li><span class="pill done">PR #N</span>…</li>` entry to
     the "Timeline" section.
   - If the feature shipped a backlog item from "Recommended next
     features", flip its heading to include
     `<span class="pill done">Shipped in #N</span>` and rewrite the
     body in past tense.
   - Bump the file-count / LOC in the hero meta if the order-of-magnitude
     changed.
6. Update `README.md` **Controls** if you changed user input.
7. Open a PR with the standard template (summary, review checklist,
   notes). CI green → ready for review.

## Quick reference: existing PRs

| PR | Status | What it added |
|---|---|---|
| #3 | Merged | Bevy 0.18 voxel scaffold, face-culling mesher, fly-cam, fog |
| #4 | Merged | `docs/index.html` progress page + screenshots |
| #6 | Merged | Voxel DDA raycast, break / place, digit-key block select |
| #8 | Merged | PBR lighting rig, cascaded shadows, tonemapping |
| #9 | Merged | Per-vertex ambient occlusion baked into the mesher |
| #10 | Merged | Greedy AO-aware mesher (merges coplanar same-type quads) |
| #11 | Merged | Animated water via `ExtendedMaterial` + custom vertex shader |
| #7 | Merged | Inventory-backed hotbar HUD with per-slot counts |
| #12 | Merged | Streaming chunk loader (Chebyshev radius, per-frame caps, hysteresis) |
| #13 | Merged | Procedural block texture atlas sampled by the chunk material |
| #15 | Merged | Toggleable sub-style headlights (`L` to toggle) |
| #16 | Merged | Sub controller: swept-AABB collision + onboard O2 reserve + HUD |
| #17 | Open | Caustics fragment shader + fullscreen god-rays post-process |
