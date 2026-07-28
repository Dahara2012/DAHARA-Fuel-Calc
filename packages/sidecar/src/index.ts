import { CappedBuffer } from "./rolling.ts";
import { SFDetector } from "./sf-detector.ts";
import { computeOnSFCrossing } from "./fuel.ts";
export { IRacingSdkAdapter } from "./sdk.ts";
import type { ISDK, SFSnapshot } from "./protocol.ts";
import type {
  FuelState,
  SessionInfoEvent,
  SidecarEvent,
  StatusEvent,
} from "@dahara/shared";

const TICK_MS = 1000 / 60;
const STATE_PUBLISH_TICK = 30;

export type SidecarRunnerArgs = {
  sdkFactory: () => ISDK;
  emit: (e: SidecarEvent) => void;
  now: () => number;
};

export class SidecarRunner {
  private readonly sdk: ISDK;
  private readonly emit: (e: SidecarEvent) => void;
  private readonly now: () => number;

  private fuelHistory = new CappedBuffer<number>(5);
  private lapTimeHistory = new CappedBuffer<number>(5);
  private detector = new SFDetector();

  private lastSessionInfoKey = "";
  // Start at 1 so the very first tick does not pass the
  // `tickCount % STATE_PUBLISH_TICK === 0` check — the first status
  // event fires after the throttle window has elapsed.
  private tickCount = 1;
  private running = false;

  constructor(args: SidecarRunnerArgs) {
    this.sdk = args.sdkFactory();
    this.emit = args.emit;
    this.now = args.now;
  }

  async run(): Promise<void> {
    const started = await this.sdk.start();
    if (!started) {
      process.stderr.write(
        "[sidecar] SDK unavailable; exiting (this is expected on non-Windows dev hosts)\n",
      );
      return;
    }
    this.running = true;
    while (this.running) {
      this.tick();
      await sleep(TICK_MS);
    }
  }

  stop(): void {
    this.running = false;
    this.sdk.stop();
  }

  isRunning(): boolean {
    return this.running;
  }

  resetBuffers(): void {
    this.fuelHistory.clear();
    this.lapTimeHistory.clear();
    this.detector.reset();
  }

  tickOnce(): void {
    this.tick();
  }

  private tick(): void {
    if (!this.sdk.waitForData(0)) {
      this.status(false, false, "no data");
      return;
    }

    const snap = this.sdk.getCurrentData();
    if (!snap) {
      this.status(false, false, "no session data");
      return;
    }

    this.publishSessionInfoIfChanged(snap);

    if (!snap.session.inRace) {
      this.status(true, false, "non-race session");
      return;
    }

    this.status(true, true);

    if (this.detector.onLap(snap.lap)) {
      this.handleSFCrossing(snap);
    }
  }

  private handleSFCrossing(snap: SFSnapshot): void {
    if (snap.fuelMaxL <= 0) {
      return;
    }

    const result = computeOnSFCrossing({
      fuelMaxL: snap.fuelMaxL,
      currentFuelPct: snap.currentFuelPct,
      lastLapTimeS: snap.lastLapTimeS,
      timeRemainS: snap.timeRemainS,
      lapsRemaining: snap.lapsRemaining,
      sessionLaps: snap.session.sessionLaps,
      sessionTimeSec: snap.session.sessionTimeSec,
      fuelHistory: this.fuelHistory,
      lapTimeHistory: this.lapTimeHistory,
    });

    const event: FuelState = {
      type: "state",
      lap: snap.lap,
      fuelLevelL: result.fuelLevelL,
      fuelMaxL: snap.fuelMaxL,
      lapTimeS: snap.lastLapTimeS,
      timeRemainS: snap.timeRemainS,
      lapsLeft: result.lapsLeft,
      fuelPerLap: result.fuelPerLap,
      refuelL: result.refuelL,
      fitsInTank: result.fitsInTank,
      confidence: result.confidence,
      timestamp: this.now(),
    };
    this.emit(event);
  }

  private publishSessionInfoIfChanged(snap: SFSnapshot): void {
    const key = `${snap.session.kind}|${snap.fuelMaxL}|${snap.session.sessionLaps ?? ""}|${snap.session.sessionTimeSec ?? ""}`;
    if (key === this.lastSessionInfoKey) return;
    this.resetBuffers();
    this.lastSessionInfoKey = key;
    const event: SessionInfoEvent = {
      type: "session-info",
      session: snap.session,
    };
    this.emit(event);
  }

  private status(connected: boolean, inRace: boolean, reason?: string): void {
    if (this.tickCount++ % STATE_PUBLISH_TICK !== 0) {
      return;
    }
    const event: StatusEvent = {
      type: "status",
      connected,
      inRace,
      reason,
    };
    this.emit(event);
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((res) => setTimeout(res, ms));
}
