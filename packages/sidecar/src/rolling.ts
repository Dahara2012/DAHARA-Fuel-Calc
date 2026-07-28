export class CappedBuffer<T> {
  private buf: T[] = [];
  private readonly cap: number;

  constructor(cap: number) {
    if (cap <= 0) throw new Error("CappedBuffer cap must be > 0");
    this.cap = cap;
  }

  push(value: T): void {
    this.buf.push(value);
    if (this.buf.length > this.cap) {
      this.buf.shift();
    }
  }

  values(): readonly T[] {
    return this.buf;
  }

  size(): number {
    return this.buf.length;
  }

  clear(): void {
    this.buf = [];
  }
}

export function median(values: readonly number[]): number {
  if (values.length === 0) {
    throw new Error("median() requires at least one value");
  }
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  if (sorted.length % 2 === 1) {
    return sorted[mid]!;
  }
  return (sorted[mid - 1]! + sorted[mid]!) / 2;
}

export function consecutiveDeltas(
  oldestFirst: readonly number[],
): number[] {
  const out: number[] = [];
  for (let i = 0; i < oldestFirst.length - 1; i++) {
    out.push(oldestFirst[i]! - oldestFirst[i + 1]!);
  }
  return out;
}
