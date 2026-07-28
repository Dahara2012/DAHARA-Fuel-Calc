export type SessionKind = "race" | "practice" | "qualifying" | "other";

export type SessionInfo = {
  kind: SessionKind;
  inRace: boolean;
  fuelMaxL: number;
  sessionLaps: number | null;
  sessionTimeSec: number | null;
};

export type FuelState = {
  type: "state";
  lap: number;
  fuelLevelL: number;
  fuelMaxL: number;
  lapTimeS: number;
  timeRemainS: number;
  lapsLeft: number;
  fuelPerLap: number;
  refuelL: number;
  fitsInTank: boolean;
  confidence: "high" | "low";
  timestamp: number;
};

export type SessionInfoEvent = {
  type: "session-info";
  session: SessionInfo;
};

export type StatusEvent = {
  type: "status";
  connected: boolean;
  inRace: boolean;
  reason?: string;
};

export type SidecarEvent = FuelState | SessionInfoEvent | StatusEvent;

export function isFuelState(e: SidecarEvent): e is FuelState {
  return e.type === "state";
}
