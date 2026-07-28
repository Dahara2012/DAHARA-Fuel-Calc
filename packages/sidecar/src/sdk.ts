import type { ISDK, SFSnapshot } from "./protocol.ts";
import { parseSessionFromYaml } from "./session.ts";

type IRacingSdkInstance = {
  startSDK(): boolean;
  stopSDK(): void;
  waitForData(timeout?: number): boolean;
  getSessionData(): unknown;
  getTelemetryVariable<T = number>(name: string):
    | { value: T[] }
    | null;
};

type IRacingSdkModule = {
  IRacingSDK: new (config?: Record<string, unknown>) => IRacingSdkInstance;
};

let irsdkModulePromise: Promise<IRacingSdkModule | null> | null = null;

function tryLoadIRacingSdk(): Promise<IRacingSdkModule | null> {
  if (irsdkModulePromise) return irsdkModulePromise;
  irsdkModulePromise = (async () => {
    try {
      const mod = (await import("irsdk-node")) as unknown as IRacingSdkModule;
      return mod;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      process.stderr.write(`[sidecar] irsdk-node unavailable: ${msg}\n`);
      return null;
    }
  })();
  return irsdkModulePromise;
}

function readNum(
  value: readonly number[] | undefined,
  fallback: number,
): number {
  if (!value || value.length === 0) return fallback;
  const v = value[0];
  return typeof v === "number" && Number.isFinite(v) ? v : fallback;
}

export class IRacingSdkAdapter implements ISDK {
  private sdk: IRacingSdkInstance | null = null;
  private started = false;

  async start(): Promise<boolean> {
    if (this.started) return true;
    const mod = await tryLoadIRacingSdk();
    if (!mod) return false;
    this.sdk = new mod.IRacingSDK({ autoEnableTelemetry: true });
    this.started = true;
    return true;
  }

  stop(): void {
    if (this.sdk && this.started) {
      try {
        this.sdk.stopSDK();
      } catch {
        // ignore
      }
    }
    this.sdk = null;
    this.started = false;
  }

  waitForData(timeoutMs: number): boolean {
    if (!this.sdk) return false;
    try {
      return this.sdk.waitForData(timeoutMs);
    } catch {
      return false;
    }
  }

  isConnected(): boolean {
    return this.sdk !== null && this.started;
  }

  getCurrentData(): SFSnapshot | null {
    if (!this.sdk) return null;
    let sessionYaml: unknown;
    try {
      sessionYaml = this.sdk.getSessionData();
    } catch {
      return null;
    }
    const session = parseSessionFromYaml(sessionYaml);
    if (!session) return null;

    const getNum = (name: string, fallback = 0): number => {
      try {
        const v = this.sdk!.getTelemetryVariable<number>(name);
        return readNum(v?.value, fallback);
      } catch {
        return fallback;
      }
    };

    const fuelMaxL = session.fuelMaxL;

    return {
      fuelMaxL,
      currentFuelPct: getNum("FuelLevelPct", 0),
      lastLapTimeS: getNum("LapLastLapTime", 0),
      timeRemainS: getNum("SessionTimeRemain", -1),
      lap: getNum("LapCompleted", 0),
      lapsRemaining: getNum("SessionLapsRemainEx", -1),
      session,
    };
  }
}
