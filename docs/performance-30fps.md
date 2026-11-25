# Plan: making 30 fps possible in the world

## Goal and budget

The compatibility path reports 1-3 fps in the world: 333-1000 ms per frame.
A 30 fps frame has **33.3 ms** for all of the following:

1. the title records its lists on its CPU threads;
2. the layer refreshes host copies of title memory, detiles textures and
   prepares host command lists;
3. the host GPU runs translated shaders and the layer's own bookkeeping;
4. shader-written buffers are compared and copied back into title memory;
5. the frame is copied to the swap chain and presented.

The repository's own measurements explain why the old path cannot approach
that budget. In run 86, a 446 ms frame was 134 ms of preparation, 194 ms
waiting for the GPU and 78 ms of copy-back, with those three phases largely
serial. Other comments record thousands of small kernel calls and compute
passes per frame, 150-360 texture refreshes that can block on remapping, and
4K shading on a desktop GPU. A single optimization cannot close a 10-30x
gap; the work has to be overlapped, parallelised, batched and reduced.

## Plan

### 1. Remove the CPU/GPU relay race

Turn on the already-implemented asynchronous submission line:

- `VICTORY_LATE_WAIT=3`: stage 2's exact refresh/merge rules and stage 3's
  present sent ahead of the next recorded frame;
- `VICTORY_RELAY=2`: a worker lands completed flights in order, and a
  submission waits only for the flights whose declared writes it reads.

This removes the repeated pattern of `prepare -> wait for GPU -> copy back`
on one thread. Correctness relies on the existing declared write ranges,
bottom-of-pipe writes, fence ordering and per-flight copy-back accounting.

### 2. Parallelise host memory work

- `VICTORY_WIDE_BACK=1`: snapshots and comparisons across ranges on a small
  persistent worker pool;
- `VICTORY_WIDE_REFRESH=1`: independent host-copy refreshes in parallel;
- `VICTORY_WIDE_MERGE=1`: shader-written pieces merged back on several
  threads;
- `VICTORY_TEXTURE_NO_WAIT=1`: avoid a whole frame's texture refreshes
  queueing behind the runtime's remapping lock. A texture may arrive one
  frame later; it is never shown partly read.

These move work off the submitting thread without changing the bytes that
are compared or copied back.

### 3. Batch small GPU and host steps

- `VICTORY_CARD_REHASH_RUNS=1`: one hash pass over adjacent 64 KiB pieces
  instead of one pass per changed range;
- `VICTORY_JOIN_COPIES=1`: adjacent title-buffer copies become one kernel
  call;
- `VICTORY_INDIRECT_WINDOW=1`: clamp a window of consecutive indirect mesh
  records in one compute pass rather than alternating compute/draw for every
  record;
- `VICTORY_PIPELINE_WAIT_MS=24`: cap per-frame waits for background pipeline
  creation. A missing pipeline drops its draw until ready instead of blocking
  five title threads for half a second each.

### 4. Reduce traffic and pixel count

- `VICTORY_CARD_BUFFERS_MIN_MB=1`, `VICTORY_CARD_BUFFERS_MB=1024`: put more
  frequently-read title buffers in GPU-local memory;
- `VICTORY_CARD_WINDOWS_MB=512`: do the same for vertex/index/constant
  windows. `victory-gpu` checks the adapter's budget before each placement,
  so this is a ceiling, not an unconditional allocation;
- `VICTORY_DISPLAY_MODE=1280x720`: 720p is one ninth of the 4K pixel count
  the layer previously reported. `balanced` uses 1080p (one quarter).

The internal display mode is the largest available quality/performance
lever: it changes what the title itself allocates and shades. Dynamic
in-run resolution scaling is deliberately out of scope here; changing the
reported mode after the title has created its render targets would require
engine-specific cooperation.

### 5. Keep an escape hatch

The implementation lives in `victory-core::performance`. It applies a
profile's defaults only to environment variables the user has not already
set, and canonicalises `VICTORY_PERFORMANCE` for relaunched child processes.

| Profile | Resolution | Scheduling | Use |
|---|---:|---|---|
| `performance` (default) | 1280x720 | all fast switches | 30 fps target |
| `balanced` | 1920x1080 | all fast switches, smaller GPU-window budget | better image |
| `compatibility` | 4K | old synchronous path | diagnosis / A-B comparison |

`victory.toml` can set `[performance].profile` and
`[performance].display_mode`; `--performance <name>` and
`scripts\play.ps1 -Performance <name>` override it. An explicit
`VICTORY_RELAY`, `VICTORY_WIDE_BACK`, `VICTORY_DISPLAY_MODE`, etc. always
wins. For example, `VICTORY_PERFORMANCE=performance` with
`VICTORY_DISPLAY_MODE=960x540` keeps the fast scheduler and lowers the pixel
count further.

## What was deliberately not enabled

- `VICTORY_INPLACE=2` is incompatible with late-wait's write tracking; stage
  1 also locks title pages and is still experimental.
- `VICTORY_BVH_BUILDER=1` would add ray-tracing builder work that is not yet
  end-to-end stable and was measured at roughly 40 ms of GPU time in one
  shader change.
- `VICTORY_MESH_TEST`, `VICTORY_SCISSOR_TEST` and `VICTORY_WAVE_WIDE` skip
  or alter rendering, or have caused device hangs. They are diagnostics, not
  a playable performance mode.
- `VICTORY_GPU_TIMING` disables late waiting to time individual lists; the
  normal launcher clears it.

## Validation

The 30 fps claim requires a Windows run with the user's game files and a
cached shader library; it cannot be measured from this repository or a Linux
sandbox. Use the window caption / `fps, ms/frame` log after shaders have
settled, and compare profiles in the same scene:

1. `scripts\play.ps1 -Game ... -Performance compatibility` for the baseline;
2. `scripts\play.ps1 -Game ... -Performance balanced`;
3. `scripts\play.ps1 -Game ...` for the 30 fps target;
4. if the GPU is still the bottleneck, add `-Resolution 960x540`;
5. if the CPU is the bottleneck, use a diagnostics build with
   `VICTORY_GPU_TIMING=1` to see the heaviest lists and commands.

A profile is successful when the steady-state line is at or below 33.3
ms/frame with no growth in dropped draws, refused descriptor tables, device
removals, or black/missing geometry. On an 8 GB card, also watch the memory
line: lower `VICTORY_CARD_BUFFERS_MB` or `VICTORY_CARD_WINDOWS_MB` if the
adapter's budget forces texture uploads out.

Repository checks:

```powershell
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

## Remaining work toward a guaranteed 30 fps

The profile removes avoidable serialisation and gives low-end GPUs a much
smaller frame, but translated shaders still do more work than native console
shaders and the layer still mirrors title memory. The next substantial gains
would be a stable in-place title-memory heap compatible with asynchronous
submissions, specialized shader lowering for the world's hottest programs,
adaptive resolution inside the title's own upscaler, and completing the
native BVH/lighting path. Each needs a measured Windows A-B run before it
can replace the conservative memory model.
