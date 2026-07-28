import { test } from "node:test";
import assert from "node:assert/strict";
import { CappedBuffer } from "../src/rolling.ts";
import { computeOnSFCrossing, type SFInputs } from "../src/fuel.ts";

const baseInputs: {
  fuelMaxL: number;
  currentFuelPct: number;
  lastLapTimeS: number;
  timeRemainS: number;
  lapsRemaining: number;
  sessionLaps: number | null;
  sessionTimeSec: number | null;
} = {
  fuelMaxL: 100,
  currentFuelPct: 50,
  lastLapTimeS: 90,
  timeRemainS: 1800,
  lapsRemaining: 25,
  sessionLaps: null,
  sessionTimeSec: 1800,
};

type Overrides = {
  [K in keyof SFInputs]?: SFInputs[K] | null;
};

function freshInputs(over: Overrides = {}): SFInputs {
  const merged = {
    fuelMaxL: baseInputs.fuelMaxL,
    currentFuelPct: baseInputs.currentFuelPct,
    lastLapTimeS: baseInputs.lastLapTimeS,
    timeRemainS: baseInputs.timeRemainS,
    lapsRemaining: baseInputs.lapsRemaining,
    sessionLaps: baseInputs.sessionLaps,
    sessionTimeSec: baseInputs.sessionTimeSec,
  };
  const o = over as Record<string, unknown>;
  const result: SFInputs = {
    fuelMaxL: (o.fuelMaxL as number | undefined) ?? merged.fuelMaxL,
    currentFuelPct:
      (o.currentFuelPct as number | undefined) ?? merged.currentFuelPct,
    lastLapTimeS: (o.lastLapTimeS as number | undefined) ?? merged.lastLapTimeS,
    timeRemainS: (o.timeRemainS as number | undefined) ?? merged.timeRemainS,
    lapsRemaining:
      (o.lapsRemaining as number | undefined) ?? merged.lapsRemaining,
    sessionLaps:
      (o.sessionLaps as number | null | undefined) ?? merged.sessionLaps,
    sessionTimeSec:
      (o.sessionTimeSec as number | null | undefined) ?? merged.sessionTimeSec,
    fuelHistory: new CappedBuffer<number>(5),
    lapTimeHistory: new CappedBuffer<number>(5),
  };
  return result;
}

test("first S/F: confidence low, fuel per lap 0, refuel may be wonky", () => {
  const r = computeOnSFCrossing(freshInputs());
  assert.equal(r.confidence, "low");
  assert.equal(r.fuelPerLap, 0);
  assert.equal(r.lapsLeft, 20);
  assert.equal(r.fuelLevelL, 50);
});

test("5 stable laps: time-limited lapsLeft = ceil(rem/medLap)", () => {
  const ins = freshInputs();
  for (let i = 0; i < 5; i++) {
    computeOnSFCrossing({
      ...ins,
      currentFuelPct: 60 - i * 2,
      lastLapTimeS: 90,
      timeRemainS: 1800 - i * 90,
    });
  }
  const final = computeOnSFCrossing({
    ...ins,
    currentFuelPct: 50,
    lastLapTimeS: 90,
    timeRemainS: 1350,
  });
  assert.equal(final.fuelPerLap, 2);
  assert.equal(final.lapsLeft, Math.ceil(1350 / 90));
  assert.equal(final.confidence, "high");
});

test("lap-limited: lapsLeft from lapsRemaining telemetry", () => {
  const ins = freshInputs({
    sessionLaps: 25,
    sessionTimeSec: null,
    timeRemainS: 0,
  });
  for (let i = 0; i < 5; i++) {
    computeOnSFCrossing({
      ...ins,
      currentFuelPct: 50 - i * 2,
      lastLapTimeS: 80,
    });
  }
  const r = computeOnSFCrossing({
    ...ins,
    currentFuelPct: 40,
    lastLapTimeS: 80,
    lapsRemaining: 22,
  });
  assert.equal(r.lapsLeft, 22);
  assert.equal(r.fuelPerLap, 2);
  assert.ok(r.refuelL > 0);
});

test("refuel negative when enough fuel", () => {
  const ins = freshInputs({
    sessionLaps: 3,
    sessionTimeSec: null,
    timeRemainS: 0,
    currentFuelPct: 90,
  });
  for (let i = 0; i < 5; i++) {
    computeOnSFCrossing({
      ...ins,
      currentFuelPct: 90 - i * 2,
      lastLapTimeS: 80,
    });
  }
  const r = computeOnSFCrossing({
    ...ins,
    currentFuelPct: 80,
    lastLapTimeS: 80,
    lapsRemaining: 3,
  });
  assert.equal(r.fuelPerLap, 2);
  assert.equal(r.lapsLeft, 3);
  assert.equal(r.refuelL, 6 - 80);
  assert.equal(r.fitsInTank, true);
});

test("pit-lap spike: trust the median, expect a skewed result", () => {
  const ins = freshInputs({
    sessionLaps: 10,
    sessionTimeSec: null,
    timeRemainS: 0,
  });
  const fuel = [50, 48, 46, 44, 42, 40, 60, 58, 56, 54, 52];
  const lapTimes = [90, 90, 90, 90, 90, 90, 90, 90, 90, 90, 90];
  for (let i = 0; i < fuel.length; i++) {
    computeOnSFCrossing({
      ...ins,
      currentFuelPct: fuel[i]!,
      lastLapTimeS: lapTimes[i]!,
    });
  }
  const r = computeOnSFCrossing({
    ...ins,
    currentFuelPct: 50,
    lastLapTimeS: 90,
  });
  // CappedBuffer is FIFO (shift on overflow). Pushing 11 values
  // [50,48,46,44,42,40,60,58,56,54,52] (fuelPct, fuelMaxL=100) yields the
  // buffer of the 5 most-recent absolute fuel levels: [60,58,56,54,52].
  // computeOnSFCrossing then pushes the final 50, making the buffer
  // [58,56,54,52,50]. Deltas (older - newer): 58-56, 56-54, 54-52, 52-50
  // = [2,2,2,2]. Median = 2.
  const expectedMedian = 2;
  assert.equal(r.fuelPerLap, expectedMedian);
});

test("time-limited with no lap history returns 0 laps", () => {
  const ins = freshInputs({
    sessionLaps: null,
    sessionTimeSec: 1800,
    timeRemainS: 1800,
    lastLapTimeS: 0,
  });
  const r = computeOnSFCrossing(ins);
  assert.equal(r.lapsLeft, 0);
});
