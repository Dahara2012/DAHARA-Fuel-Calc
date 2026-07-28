import { test } from "node:test";
import assert from "node:assert/strict";
import { SidecarRunner } from "../src/index.ts";
import type { ISDK, SFSnapshot } from "../src/protocol.ts";
import type { FuelState, SidecarEvent } from "@dahara/shared";

class MockSdk implements ISDK {
  private snapshots: SFSnapshot[];
  private cursor = 0;
  public started = false;

  constructor(snapshots: SFSnapshot[]) {
    this.snapshots = snapshots;
  }

  async start(): Promise<boolean> {
    this.started = true;
    return true;
  }
  stop(): void {
    this.started = false;
  }
  waitForData(_timeoutMs: number): boolean {
    return this.cursor < this.snapshots.length;
  }
  isConnected(): boolean {
    return this.started;
  }
  getCurrentData(): SFSnapshot | null {
    return this.snapshots[this.cursor] ?? null;
  }
  advance(): void {
    this.cursor++;
  }
}

function makeSnap(over: Partial<SFSnapshot> = {}): SFSnapshot {
  return {
    fuelMaxL: 100,
    currentFuelPct: 50,
    lastLapTimeS: 90,
    timeRemainS: 1800,
    lap: 1,
    lapsRemaining: 20,
    session: {
      kind: "race",
      inRace: true,
      fuelMaxL: 100,
      sessionLaps: null,
      sessionTimeSec: 1800,
    },
    ...over,
  };
}

function makeRunner(snapshots: SFSnapshot[]) {
  const sdk = new MockSdk(snapshots);
  const events: SidecarEvent[] = [];
  const runner = new SidecarRunner({
    sdkFactory: () => sdk,
    emit: (e) => events.push(e),
    now: () => 0,
  });
  return { runner, sdk, events };
}

test("buffers reset on session-info change", () => {
  // Race A: 5 stable laps, fuelPerLap=2.
  // The first lap is a warmup (no state event); laps 2..5 each fire.
  const raceA = [];
  for (let i = 1; i <= 5; i++) {
    raceA.push(makeSnap({ lap: i, currentFuelPct: 60 - (i - 1) * 2 }));
  }
  // Then a new race session (fuelMaxL changes -> session-info key changes).
  // 2 laps so we see the warmup + 1 fire.
  const raceB = [
    makeSnap({
      fuelMaxL: 80,
      currentFuelPct: 95,
      lap: 1,
      session: {
        kind: "race",
        inRace: true,
        fuelMaxL: 80,
        sessionLaps: null,
        sessionTimeSec: 1800,
      },
    }),
    makeSnap({
      fuelMaxL: 80,
      currentFuelPct: 95,
      lap: 2,
      session: {
        kind: "race",
        inRace: true,
        fuelMaxL: 80,
        sessionLaps: null,
        sessionTimeSec: 1800,
      },
    }),
  ];

  const { runner, sdk, events } = makeRunner([...raceA, ...raceB]);
  while (sdk.waitForData(0)) {
    runner.tickOnce();
    sdk.advance();
  }

  const stateEvents = events.filter(
    (e): e is FuelState => e.type === "state",
  );

  // 5 race A snapshots -> 4 state events (laps 2..5).
  // 2 race B snapshots -> 1 state event (lap 2 after reset).
  assert.equal(stateEvents.length, 5);
  // Last race A event: full buffer, fuelPerLap=2, confidence=high.
  assert.equal(stateEvents[3]!.fuelPerLap, 2);
  assert.equal(stateEvents[3]!.confidence, "high");

  // First (only) race B state event: buffers were just reset.
  const firstB = stateEvents[4]!;
  assert.equal(firstB.lap, 2);
  assert.equal(firstB.fuelPerLap, 0, "buffers should have been reset");
  assert.equal(firstB.confidence, "low");
});
