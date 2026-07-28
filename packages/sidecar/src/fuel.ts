import { CappedBuffer, consecutiveDeltas, median } from "./rolling.ts";

export type SFInputs = {
  fuelMaxL: number;
  currentFuelPct: number;
  lastLapTimeS: number;
  timeRemainS: number;
  lapsRemaining: number;
  sessionLaps: number | null;
  sessionTimeSec: number | null;
  fuelHistory: CappedBuffer<number>;
  lapTimeHistory: CappedBuffer<number>;
};

export type SFResult = {
  fuelLevelL: number;
  lapsLeft: number;
  fuelPerLap: number;
  refuelL: number;
  fitsInTank: boolean;
  confidence: "high" | "low";
};

export function computeOnSFCrossing(inputs: SFInputs): SFResult {
  const fuelLevelL = (inputs.currentFuelPct / 100) * inputs.fuelMaxL;

  inputs.fuelHistory.push(fuelLevelL);
  inputs.lapTimeHistory.push(inputs.lastLapTimeS);

  const fuelDeltas = consecutiveDeltas(inputs.fuelHistory.values());
  const lapTimes = inputs.lapTimeHistory.values();

  const haveFuelData = fuelDeltas.length > 0;
  const fuelPerLap = haveFuelData ? median(fuelDeltas) : 0;

  const lapsLeft = computeLapsLeft(
    inputs.timeRemainS,
    inputs.lapsRemaining,
    inputs.sessionLaps,
    inputs.sessionTimeSec,
    lapTimes,
  );

  const fuelNeeded = lapsLeft * fuelPerLap;
  const refuelL = fuelNeeded - fuelLevelL;
  const fitsInTank = fuelLevelL + refuelL <= inputs.fuelMaxL + 1e-6;

  const confidence: "high" | "low" =
    fuelDeltas.length >= 3 && lapTimes.length >= 3 ? "high" : "low";

  return {
    fuelLevelL,
    lapsLeft,
    fuelPerLap,
    refuelL,
    fitsInTank,
    confidence,
  };
}

function computeLapsLeft(
  timeRemainS: number,
  lapsRemaining: number,
  sessionLaps: number | null,
  sessionTimeSec: number | null,
  lapTimes: readonly number[],
): number {
  const hasTimeLimit = sessionTimeSec !== null && sessionTimeSec > 0;
  const hasLapLimit = sessionLaps !== null && sessionLaps > 0;

  if (hasTimeLimit && !hasLapLimit) {
    if (lapTimes.length === 0) return 0;
    const med = median(lapTimes);
    if (med <= 0) return 0;
    return Math.max(0, Math.ceil(timeRemainS / med));
  }

  if (hasLapLimit) {
    return Math.max(0, Math.floor(lapsRemaining));
  }

  return 0;
}
