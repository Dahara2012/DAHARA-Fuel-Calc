import type { SessionInfo } from "@dahara/shared";

export type SFSnapshot = {
  fuelMaxL: number;
  currentFuelPct: number;
  lastLapTimeS: number;
  timeRemainS: number;
  lap: number;
  lapsRemaining: number;
  session: SessionInfo;
};

export interface ISDK {
  start(): Promise<boolean>;
  stop(): void;
  waitForData(timeoutMs: number): boolean;
  getCurrentData(): SFSnapshot | null;
  isConnected(): boolean;
}
