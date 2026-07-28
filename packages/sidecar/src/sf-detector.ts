export class SFDetector {
  private lastLap: number = -1;

  onLap(lap: number): boolean {
    if (lap <= 0) return false;
    if (this.lastLap === -1) {
      this.lastLap = lap;
      return false;
    }
    if (lap > this.lastLap) {
      this.lastLap = lap;
      return true;
    }
    return false;
  }

  reset(): void {
    this.lastLap = -1;
  }
}
