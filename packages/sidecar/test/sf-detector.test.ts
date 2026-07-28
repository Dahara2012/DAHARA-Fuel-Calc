import { test } from "node:test";
import assert from "node:assert/strict";
import { SFDetector } from "../src/sf-detector.ts";

test("first valid lap does not fire", () => {
  const d = new SFDetector();
  assert.equal(d.onLap(1), false);
});

test("increasing lap fires exactly once", () => {
  const d = new SFDetector();
  d.onLap(1);
  assert.equal(d.onLap(2), true);
  assert.equal(d.onLap(2), false);
});

test("same lap does not fire", () => {
  const d = new SFDetector();
  d.onLap(5);
  assert.equal(d.onLap(5), false);
  assert.equal(d.onLap(5), false);
});

test("zero or negative lap is ignored", () => {
  const d = new SFDetector();
  assert.equal(d.onLap(0), false);
  assert.equal(d.onLap(-1), false);
  d.onLap(3);
  assert.equal(d.onLap(0), false);
  assert.equal(d.onLap(4), true);
});

test("reset clears state", () => {
  const d = new SFDetector();
  d.onLap(1);
  d.reset();
  assert.equal(d.onLap(1), false);
});
