# AquaForge

AquaForge is an underwater voxel sandbox written in Rust with
[Bevy 0.18](https://bevy.org). Think "Minecraft, but the whole world is
a seafloor" — you fly through a murky ocean, see a chunked blocky
terrain of sand, stone, dirt, and coral, and look up at a glinting
water surface above.

This branch is the project's **base scaffold**: just enough code to
have a lit, foggy underwater scene with deterministic, chunked voxel
terrain and a free-flying camera.

## Controls

| Input              | Action                                  |
|--------------------|-----------------------------------------|
| `W` / `A` / `S` / `D` | Move forward/left/back/right         |
| `Space` / `LShift` | Move up / down                          |
| `LCtrl`            | Hold to move 3× faster                  |
| Mouse              | Look around (after clicking the window) |
| `Left Click`       | Capture the mouse                       |
| `Esc`              | Release the mouse                       |

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
│   └── input.rs        # Fly-cam: WASD, mouse look, cursor grab/release
└── utils/
    ├── math.rs         # smoothstep / bilerp helpers
    └── noise.rs        # Tiny value-noise + fBm (no external deps)
```

## Next steps

The base intentionally leaves room for:

- Block placement & destruction (ray-cast from the camera)
- Proper swim physics (buoyancy, drag, air budget)
- Biomes (kelp forests, reefs, thermal vents)
- Infinite chunk streaming around the player
- Underwater post-processing (caustics, god-rays, volumetric lighting)
