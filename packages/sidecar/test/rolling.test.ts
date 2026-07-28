import { test } from "node:test";
import assert from "node:assert/strict";
import { CappedBuffer, consecutiveDeltas, median } from "../src/rolling.ts";

test("median of odd-length list", () => {
  assert.equal(median([3, 1, 2]), 2);
});

test("median of even-length list", () => {
  assert.equal(median([4, 1, 3, 2]), 2.5);
});

test("median ignores input order", () => {
  assert.equal(median([10, 90, 50, 30, 70]), 50);
});

test("median throws on empty", () => {
  assert.throws(() => median([]));
});

test("CappedBuffer caps at the configured size", () => {
  const buf = new CappedBuffer<number>(3);
  buf.push(1);
  buf.push(2);
  buf.push(3);
  buf.push(4);
  assert.deepEqual([...buf.values()], [2, 3, 4]);
  assert.equal(buf.size(), 3);
});

test("CappedBuffer clear empties", () => {
  const buf = new CappedBuffer<number>(3);
  buf.push(1);
  buf.push(2);
  buf.clear();
  assert.equal(buf.size(), 0);
  assert.deepEqual([...buf.values()], []);
});

test("CappedBuffer rejects zero cap", () => {
  assert.throws(() => new CappedBuffer<number>(0));
});

test("consecutiveDeltas oldest-to-newest", () => {
  assert.deepEqual(consecutiveDeltas([5, 3, 1]), [2, 2]);
});

test("consecutiveDeltas empty", () => {
  assert.deepEqual(consecutiveDeltas([]), []);
});

test("consecutiveDeltas single value", () => {
  assert.deepEqual(consecutiveDeltas([5]), []);
});
