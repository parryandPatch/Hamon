# Hamon

A hardware monitor for the desktop, built as a Tauri 2 app. It samples the
machine on a background thread and pushes one snapshot per tick to the window;
the dashboard is a grid of widgets you arrange yourself.

Widgets are added, removed, reordered and configured from inside the app, so the
dashboard can be shaped around the machine it is watching — CPU-first during a
compile, storage-first during a build, quiet and small on a laptop on battery.

Runs on macOS and Linux. macOS is the development target; see
[Running it on Linux](#running-it-on-linux) for what to expect there.

## What it shows

Seventeen widget kinds:

| Group | Kinds |
| --- | --- |
| Metrics | Gauge (any metric, with an arc), History (any metric over time) |
| Hardware | CPU cores, Memory, GPU |
| Storage & network | Network, Network interfaces, Disk, Filesystems, Disk I/O history |
| System | Temperatures, Processes, Battery, System, Uptime |
| Custom | Custom text, Custom ASCII |

The `Gauge` and `History` widgets work with any metric the sampler reports: CPU
usage and frequency, CPU temperature, memory and swap usage, GPU usage,
temperature, power, memory and fan, network up/down, disk read/write and
usage, battery level and power.

A few deliberate choices worth knowing about:

- **An absent metric is never shown as zero.** A sensor the platform did not
  report renders as an em dash, or an explicit empty state, so "no data" is
  always distinguishable from "no activity".
- **Charts do not rescale to the current maximum.** An idle CPU graph scaled to
  its own peak looks like a storm; the axis grows in fixed steps instead, so a
  steady line stays steady.
- **Gaps are drawn as gaps.** A metric that stops being reported breaks the line
  rather than being interpolated across.
- **Privileges are explained, not hidden.** macOS temperatures come from SMC,
  which needs root. Hamon runs fine without it and says which sensors it could
  not read, rather than showing an empty temperatures widget.

## Requirements

- Rust 1.90 or newer (the crate uses edition 2024)
- Node 20 or newer
- Tauri's platform prerequisites — see below

## Building and running on macOS

```sh
npm install
npm run tauri dev
```

A first `cargo build` pulls in the WebKit and Objective-C bindings and takes a
while; later builds are fast.

To produce a distributable bundle:

```sh
npm run tauri build
```

This writes `Hamon.app` and a `.dmg` under
`src-tauri/target/release/bundle/`.

Templating runs through root, so to read SMC temperatures:

```sh
sudo "$(pwd)/src-tauri/target/release/hamon"
```

## Running it on Linux

Linux support is complete and ships in the same binary — the collectors read
`/proc` and `/sys`, and the frontend is identical. It has not been exercised on
a live machine as thoroughly as macOS, so treat the first run as a test.

### Prerequisites

Tauri v2 on Linux needs WebKitGTK and its dependencies. On NixOS:

```nix
pkgs.stdenv.mkDerivation {
  pname = "hamon";
  version = "0.1.0";

  src = ./.;

  nativeBuildInputs = with pkgs; [
    cargo rustc nodejs_22 pkg-config
    # For `npm run tauri build`:
    makeWrapper patchelf
  ];

  buildInputs = with pkgs; [
    webkitgtk_4_1
    gtk3
    libsoup_3
    javascriptcoregtk-4.1
    librsvg
  ];

  # Tauri must not be built inside an existing `$out`; the bundler writes into it.
  dontConfigure = true;

  installPhase = ''
    runHook preInstall
    npm ci --prefix .
    npm run tauri build --prefix .
    mkdir -p $out
    cp -r src-tauri/target/release/bundle/* $out/
    runHook postInstall
  '';
}
```

Or, interactively on a `nix-shell`:

```nix
{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [
    cargo rustc nodejs_22 pkg-config
    webkitgtk_4_1 gtk3 libsoup_3 javascriptcoregtk-4.1 librsvg
  ];
}
```

Then, inside the shell:

```sh
npm install
npm run tauri dev
```

On Debian and Ubuntu the equivalents are `libwebkit2gtk-4.1-dev`,
`libgtk-3-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`,
`librsvg2-dev`, plus `build-essential`, `curl`, `file`, `libssl-dev` and
`pkg-config`.

### What to check on Linux

- **Temperatures and fans** come from `/sys/class/hwmon`. `hwmon` is world
  readable, but some chips (`coretemp`, `k10temp`, `zenpower`) restrict their
  `temp*_input` files to root. Sensors that need it are listed with a lock icon
  rather than omitted; run `sudo hamon` to read them all.
- **GPU** metrics use NVML, `dlopen`ed at runtime. If `libnvidia-ml.so.1` is not
  on the library path the GPU widget degrades to whatever else is available
  rather than failing.
- **Disks** are read from `/proc/diskstats`, so whole-disk and partition
  entries both appear. Very large disks report in 512-byte sectors regardless of
  their real block size; rates are computed from deltas, so they stay correct.
- **Network** per-interface counters come from `/proc/net/dev`, with carrier and
  MTU from `/sys/class/net` and IPv4 addresses from `getifaddrs`.
- **Distro name** is read from `/etc/os-release`.

## Development

```sh
npm run dev        # frontend only, in a browser, against synthetic data
npm run tauri dev  # the real thing
```

`npm run dev` is not a mock UI. When the Tauri IPC is absent, `src/lib/tauri.ts`
serves the same command surface from `src/lib/mock.ts`, which generates lively
synthetic snapshots and keeps layout edits in `localStorage`. That makes the
entire frontend — edit mode, drag-to-reorder, the configuration panel, every
chart — developable and inspectable in a normal browser without a Rust rebuild.

### Checks

```sh
npm run check              # svelte-check, tsc, and both contract checks
npm run check:catalog      # widget catalog parity
npm run check:wire         # wire-format shape parity
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets
cd src-tauri && cargo fmt --check
```

The two `check:` scripts exist because parts of the contract are declared
twice, once per language, and cannot share a source. Both compare a
hand-written frontend declaration against the real Rust definition, read by
running one of the two examples in `src-tauri/examples/`:

- **`check:catalog`** — `catalog.ts` carries the labels and field schemas the UI
  renders; `layout.rs` carries the kinds and default configs the backend hands
  out. A kind in one file but not the other still builds and only shows up as a
  widget that renders nothing.
- **`check:wire`** — `types.ts` mirrors the Rust wire types by hand. Rename a
  field in `model.rs` and TypeScript still typechecks, because it has no idea
  the field moved; the widget then renders an em dash forever. This compares
  every leaf path of a real serialised `Snapshot` against the synthetic one in
  `mock.ts`, in both directions, so neither a missing field nor a field only the
  frontend believes in can pass.

## How it fits together

```
src-tauri/src/
  lib.rs        plugin setup, sampler start
  commands.rs   the IPC surface; AppState owns the sampler and the layout store
  sample.rs     the sampler thread: interval, wake channel, emit callback
  model.rs      the wire types, serialised once per tick
  layout.rs     Layout/Widget, persistence, the catalog contract
  collect/      one module per subsystem, platform-independent
  platform/     the parts that must be per-OS: macOS FFI, Linux /proc and /sys
src/lib/
  tauri.ts      the only module that imports @tauri-apps; selects real or mock
  state.svelte.ts  the Dashboard rune class: snapshot, layout, edit state, commands
  catalog.ts    widget kinds, field schemas, defaults — the frontend contract
  types.ts      hand-written mirrors of the Rust wire types
  metrics.ts    metric id -> value
  history.svelte.ts  reactive ring buffers for the charts
  widgets/      one component per widget kind
```

Three decisions shape most of the code:

**Push, not poll.** A sampler thread emits a whole `Snapshot` on
`hamon://snapshot` once per tick. There is no polling anywhere in the UI, so a
paused sampler and a stalled one are distinguishable — the UI shows a tick age.

**Layout mutations round-trip.** Every mutating command returns the new layout,
and the frontend replaces its state with it wholesale. The backend is therefore
the only place layout invariants are enforced, and the frontend cannot drift out
of sync with what was actually saved.

**Null is a value.** Every wire field is `snake_case` and `Option<T>` arrives as
`null`, never absent. `responses_use_the_same_field_naming_as_the_snapshots` in
`commands.rs` pins that convention, and `npm run check:wire` holds the whole
struct shape against the Rust definitions.

Metrics are resolved in the frontend, not the backend. A metric id like
`cpu-usage` can span several devices, and which one to report depends on the
metric: load and temperature take the worst device, power and memory are summed.
Those rules live with the rest of `metrics.ts`.

## Where your layout lives

`layout.json` in Tauri's per-platform config directory:

- macOS: `~/Library/Application Support/com.hamon.app/layout.json`
- Linux: `~/.config/com.hamon.app/layout.json`

Delete it to start over. **Edit dashboard → Reset** restores the defaults without
deleting the file.

## Layout of the file

```
Layout { version, columns, sample_interval_ms, widgets: [ { kind, config } ] }
```

`kind` is one of the catalog kinds; `config` is kind-specific and every field has
a default. Unknown kinds are preserved rather than dropped, so downgrading does
not lose your dashboard, and the widget says so instead of rendering blank.