# DAHARA Fuel Calc

A tiny, always-on-top overlay for iRacing that tells you how many liters of
fuel to add at the next pit stop to finish the race.

- Green = the calculated amount fits in the tank
- Red = you cannot make it on one tank, you need to add more or save fuel

The number updates only on start/finish crossings so it never flickers
mid-lap. It uses the median of your last 5 laps for both fuel-per-lap and
lap-time estimation (5 laps is the rolling window; you can change it in
`packages/sidecar/src/index.ts`).

## Architecture

```
┌─────────────────────┐         ┌────────────────────┐
│  Tauri (Rust)       │  emit   │  Webview (TS)      │
│  - spawns sidecar   │ ──────▶ │  - transparent     │
│  - reads stdio JSON │  fuel   │  - click-through   │
│  - manages window   │  events │  - shows number    │
└─────────┬───────────┘         └────────────────────┘
          │ spawns
          ▼
┌─────────────────────┐
│  Node sidecar       │
│  - irsdk-node       │
│  - 60 Hz tick       │
│  - S/F detector     │
│  - fuel math        │
│  - JSON-lines → out │
└─────────────────────┘
```

- `packages/sidecar/` — Node + `irsdk-node`. Pure logic in `fuel.ts`,
  `rolling.ts`, `sf-detector.ts`, `session.ts`; the `IRacingSdkAdapter`
  wraps the SDK; `index.ts` is the entrypoint that emits JSON-lines on stdout.
- `src-tauri/` — Rust host. Spawns the sidecar, reads its stdout, forwards
  events to the webview. Manages the always-on-top, transparent, click-through
  window.
- `packages/renderer/` — Vanilla TS + Vite. Listens for `fuel` events, diffs
  before redrawing, applies the green/red color class.
- `packages/shared/` — `SidecarEvent` discriminated union shared by the
  sidecar and the renderer.

## Calculation

On every S/F crossing, the sidecar:

1. Records `fuelAtSF = FuelLevelPct * DriverCarFuelMaxLtr` and the just-completed
   `LapLastLapTime` into a 5-element ring buffer.
2. Computes `fuelPerLap = median(fuelDeltas)` (newest minus older).
3. Computes `lapsLeft`:
   - **Lap-limited:** `SessionLapsRemainEx`
   - **Time-limited:** `ceil(SessionTimeRemain / median(lapTimes))`, plus one
     extra lap when the first decimal place of the division is 5 or higher.
4. `fuelNeeded = lapsLeft * fuelPerLap`
5. `refuelL = fuelNeeded - currentFuelLevelL`
6. Color: green if `currentFuelLevelL + refuelL <= fuelMaxL`, else red.

We do **not** special-case pit laps (per the design decision). The median
absorbs the in/out-lap spike after one or two clean laps.

## Develop

Requires Node 20+ on the dev host (works on Linux), and Windows + iRacing to
actually run the app.

```bash
npm install
npm run typecheck
npm test
```

In one terminal, run the renderer dev server:

```bash
npm run dev -w @dahara/renderer
```

In another, run the sidecar in dev mode (writes JSON-lines to stdout):

```bash
node --experimental-strip-types packages/sidecar/src/index.ts
```

For auto-restart on file changes, combine with Node's built-in `--watch`:

```bash
node --watch --experimental-strip-types packages/sidecar/src/index.ts
```

The Tauri host (`npm run tauri dev`) is the integration point. On Linux the
sidecar logs `iRacing SDK is Windows-only; refusing to run on linux.` and
exits with code 1 within a few hundred milliseconds; the Rust host will
surface this in the dev console.

## Build a release (Windows host only)

The sidecar is bundled into a single `.exe` via
[`bun build --compile`](https://bun.sh/docs/bundler/executables), which must be
on `PATH` on the build host. Install it once:

```bash
# Windows (PowerShell)
irm bun.sh/install.ps1 | iex
# macOS / Linux
curl -fsSL https://bun.sh/install | bash
```

Then:

```bash
# 1. install deps + typecheck + tests
npm install
npm test

# 2. produce the sidecar .exe
npm run build:sidecar:exe
# -> src-tauri/binaries/dahara-fuel-calc-sidecar-x86_64-pc-windows-msvc.exe

# 3. produce the Tauri installer
npm run tauri build
# -> src-tauri/target/release/bundle/{msi,nsis}/...
```

The Tauri config in `src-tauri/tauri.conf.json` references the sidecar under
`bundle.externalBin`, so step 2 must be run before step 3. `npm run build` will
also auto-generate `src-tauri/icons/icon.png` via `scripts/make-icon.mjs` if
it doesn't already exist.

## Why Linux dev and Windows build

`irsdk-node` uses native bindings that are Windows-only, and iRacing itself
only runs on Windows. The sidecar package provides a non-functional fallback
on Linux so that TypeScript still compiles and unit tests still run. The
final `.exe` (sidecar + Tauri host + installer) must be produced on Windows
or a Windows CI runner.

## Known limitations / v1 scope

- Works in all session types (race, practice, qualifying).
- The overlay's window position is hardcoded to `(20, 20)`. No drag-to-move
  or settings UI yet.
- No fuel safety margin; the number is the raw calculation.
- No auto-pit-click. The user reads the number and enters it themselves.
- The sidecar must be rebuilt every time the sidecar TS code changes.

## File-by-file

```
DAHARA-Fuel-Calc/
├── package.json
├── tsconfig.base.json
├── packages/
│   ├── shared/                  # protocol types
│   │   └── src/events.ts
│   ├── sidecar/                 # Node + irsdk-node
│   │   ├── src/
│   │   │   ├── index.ts         # entrypoint
│   │   │   ├── sdk.ts           # irsdk-node wrapper
│   │   │   ├── session.ts       # YAML parser (DriverCarFuelMaxLtr, session type)
│   │   │   ├── sf-detector.ts   # LapCompleted edge detector
│   │   │   ├── fuel.ts          # pure refuel calculation
│   │   │   ├── rolling.ts       # capped ring buffer + median
│   │   │   └── protocol.ts      # ISDK interface
│   │   └── test/                # node --test suites
│   └── renderer/                # webview (Vite + vanilla TS)
│       ├── index.html
│       └── src/
│           ├── main.ts
│           └── style.css
├── src-tauri/                   # Rust host
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── sidecar.rs           # spawn sidecar, stdio pump
│       └── window.rs            # transparent AOT setup
├── scripts/
│   ├── build-sidecar.mjs        # bun build --compile for Windows
│   └── make-icon.mjs            # placeholder PNG generator
└── docs/
    └── PLAN.md                  # this design doc
```

## License

TBD.
