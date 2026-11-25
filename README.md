# ztrx

**ztrx** runs the Xbox Series build of the game natively on Windows PCs with
NVIDIA graphics cards, from game files you supply yourself.

**This repository contains no game files or assets.** You need your own copy
of the game. Not affiliated with or endorsed by its developer or publisher.

## Status (2026-11-25)

Tested on Windows 11 with an **NVIDIA GeForce RTX 3050** (8 GB):

| | |
|---|---|
| Boot, first-boot screens, intro, opening credits | works |
| Prologue | playable from the cell to the boat chase: cell, cafeteria, robbery, police chase (wanted level, minimap), boat, binoculars |
| Stability | 1 h 30 min+ in one session without a crash |
| Frame rate | 1-3 fps on the old compatibility path; the default `performance` profile targets 30 fps with a 720p internal mode and asynchronous GPU work (hardware/scene dependent; not yet measured on every machine). The first run is slower while shaders translate; they are cached after that. |
| Known problems | anything in shadow is black and bright scenes overexpose (the game's ray-traced lighting does not run yet); some blocky artifacts; boats sit low in the water |

## Requirements

- Windows 10/11 x64, a CPU with AVX2
- A Direct3D 12 GPU with shader model 6.6 and mesh shaders (NVIDIA RTX 20 series or newer)
- Rust (the version in `rust-toolchain.toml`)
- Several GB of free Windows commit memory (raise the paging file if `crashes.log` reports error 1455)

## Build

```powershell
cargo build --release -p victory
```

## Play

```powershell
powershell -ExecutionPolicy Bypass -File scripts\play.ps1 -Game "<your game folder>\<the game's exe>"
```

`scripts\play.ps1` keeps the shader cache and saves in folders you choose
(`-Cache`, `-Data`) and presses no buttons for you. Controller: XInput pads
(a DualShock/DualSense through DS4Windows works). With no pad connected the
keyboard acts as one while the game window has focus.

## Performance profiles (30 fps target)

The launcher and `victory run` now start in the `performance` profile. It is
a bundle of the back end's measured fast paths: CPU/GPU overlap, a copy-back
relay thread, parallel memory refreshes and comparisons, batched GPU
bookkeeping, GPU-local buffer copies, and a 1280x720 internal display mode
instead of 4K. The title still owns its frame pacing; 30 fps needs a 33.3 ms
frame and is a target, not a guarantee on every GPU or scene. See
[docs/performance-30fps.md](docs/performance-30fps.md) for the bottleneck
analysis and validation procedure.

```powershell
# Default: 720p + fast scheduling.
.\scripts\play.ps1 -Game "<your game folder>\<the game's exe>"

# Better image, or the old synchronous 4K path for diagnosis.
.\scripts\play.ps1 -Game "..." -Performance balanced
.\scripts\play.ps1 -Game "..." -Performance compatibility

# Try another internal resolution without changing the profile's switches.
.\scripts\play.ps1 -Game "..." -Resolution 960x540
```

Profiles can also be selected with `VICTORY_PERFORMANCE`, `--performance`, or
`[performance].profile` in `victory.toml`. Every individual `VICTORY_*`
switch you set explicitly still wins; for example,
`VICTORY_DISPLAY_MODE=1920x1080` keeps the fast scheduling but raises the
resolution. `VICTORY_PERFORMANCE=compatibility` restores the old defaults
for every switch you have not set explicitly.
The first run remains slow while shaders translate; use the cached-shader run
to judge the steady-state frame rate.

## Debugging switches

| Switch | Effect |
|---|---|
| `VICTORY_CAPTURE=<dir>` + `VICTORY_CAPTURE_FRAMES=<list>` | save those frames as PNG (diagnostics build) |
| `VICTORY_KEYS=30:Enter,35:X` | press keys at those seconds (diagnostics build) |
| `VICTORY_D3D12_DRED=1` | on a GPU failure, report the commands in flight |
| `VICTORY_PIPELINE_WAIT_MS=24` | shorter per-frame wait for pipelines still being built |

Diagnostics build: `cargo build --profile diag -p victory --features diagnostics`

## License

GPL-3.0-or-later (see `LICENSE`). Based on VIctory (Marvisak) and victory-fork (fxceless).
