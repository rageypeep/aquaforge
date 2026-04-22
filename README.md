# AquaForge

AquaForge is an underwater voxel sandbox written in Rust with
[Bevy 0.18](https://bevy.org). Think "Minecraft, but the whole world is
a seafloor" — you fly through a murky ocean, see a chunked blocky
terrain of sand, stone, dirt, and coral, and look up at a glinting
water surface above. You pilot a small sub through it.

This branch is the project's **base scaffold**: just enough code to
have a lit, foggy underwater scene with deterministic, chunked voxel
terrain and a piloted mini-sub.

## Controls

| Input              | Action                                          |
|--------------------|-------------------------------------------------|
| `W` / `A` / `S` / `D` | Thrust forward / strafe / reverse            |
| `Space` / `LShift` | Ascend / descend                                |
| `LCtrl`            | Hold for 2× thruster boost                      |
| Mouse              | Look around (after clicking the window)         |
| `Left Click`       | Capture the mouse / break the targeted block    |
| `Right Click`      | Place from the active hotbar slot               |
| `1`–`5`            | Select hotbar slot                              |
| `Esc`              | Release the mouse                               |

The camera is a real sub now: a compact AABB hull collides with the
terrain, an onboard O2 reserve drains while submerged, and refills
once the sub surfaces.

## Running

```bash
# Rust 1.89+ is required (see `rust-toolchain.toml`).
cargo run --release
```

On Linux, Bevy needs a few system libraries. On Debian/Ubuntu:

```bash
sudo apt install pkg-config libwayland-dev libxkbcommon-dev \
  libasound2-dev libudev-dev libx11-dev libxcursor-dev \
  libxi-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev
```

## Project layout

```
src/
├── main.rs             # App bootstrap
├── game/
│   ├── blocks.rs       # BlockType enum and visual properties
│   ├── chunk.rs        # Chunk data + greedy-ish face-culling mesher
│   └── world.rs        # World generation + spawns the chunk meshes
├── rendering/
│   └── mod.rs          # Underwater fog, ambient, camera, sea surface
├── systems/
│   ├── input.rs        # Cursor grab / release
│   └── sub.rs          # Sub controller, swept-AABB collision, O2 reserve
└── utils/
    ├── math.rs         # smoothstep / bilerp helpers
    └── noise.rs        # Tiny value-noise + fBm (no external deps)
```

## Next steps

The base intentionally leaves room for:

- Biomes (kelp forests, reefs, thermal vents)
- Underwater post-processing (caustics, god-rays, volumetric lighting)
- Drowning damage once the oxygen meter empties
- Buoyancy (gear-weighted sink / float)
