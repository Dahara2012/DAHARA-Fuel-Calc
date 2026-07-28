import { test } from "node:test";
import assert from "node:assert/strict";
import { parseSessionFromYaml } from "../src/session.ts";

test("parses race session with fuel max", () => {
  const yaml = {
    SessionNum: 0,
    PlayerCarIdx: 0,
    SessionInfo: {
      Sessions: [
        {
          SessionType: "Race",
          SessionLaps: "25",
          SessionTime: "unlimited",
        },
      ],
    },
    DriverInfo: {
      Drivers: [{ CarIdx: 0, DriverCarFuelMaxLtr: 120 }],
    },
  };
  const s = parseSessionFromYaml(yaml);
  assert.ok(s);
  assert.equal(s!.kind, "race");
  assert.equal(s!.inRace, true);
  assert.equal(s!.fuelMaxL, 120);
  assert.equal(s!.sessionLaps, 25);
  assert.equal(s!.sessionTimeSec, null);
});

test("parses time-limited race", () => {
  const yaml = {
    SessionNum: 0,
    PlayerCarIdx: 0,
    SessionInfo: {
      Sessions: [
        {
          SessionType: "Race",
          SessionLaps: "unlimited",
          SessionTime: "3600",
        },
      ],
    },
    DriverInfo: {
      Drivers: [{ CarIdx: 0, DriverCarFuelMaxLtr: 60 }],
    },
  };
  const s = parseSessionFromYaml(yaml);
  assert.ok(s);
  assert.equal(s!.sessionTimeSec, 3600);
  assert.equal(s!.sessionLaps, null);
});

test("parses practice session, inRace false", () => {
  const yaml = {
    SessionNum: 0,
    PlayerCarIdx: 0,
    SessionInfo: {
      Sessions: [
        { SessionType: "Practice", SessionLaps: "0", SessionTime: "0" },
      ],
    },
    DriverInfo: { Drivers: [{ CarIdx: 0 }] },
  };
  const s = parseSessionFromYaml(yaml);
  assert.ok(s);
  assert.equal(s!.kind, "practice");
  assert.equal(s!.inRace, false);
});

test("handles unknown session type", () => {
  const yaml = {
    SessionNum: 0,
    PlayerCarIdx: 0,
    SessionInfo: {
      Sessions: [{ SessionType: "FooBar", SessionLaps: "0", SessionTime: "0" }],
    },
    DriverInfo: { Drivers: [{ CarIdx: 0 }] },
  };
  const s = parseSessionFromYaml(yaml);
  assert.equal(s!.kind, "other");
});

test("returns null for invalid input", () => {
  assert.equal(parseSessionFromYaml(null), null);
  assert.equal(parseSessionFromYaml({}), null);
  assert.equal(parseSessionFromYaml({ SessionInfo: {} }), null);
  assert.equal(parseSessionFromYaml({ SessionInfo: { Sessions: [] } }), null);
});

test("reads fuel max from the player's row, not the first driver", () => {
  const yaml = {
    SessionNum: 0,
    PlayerCarIdx: 2,
    SessionInfo: {
      Sessions: [
        { SessionType: "Race", SessionLaps: "10", SessionTime: "0" },
      ],
    },
    DriverInfo: {
      Drivers: [
        { CarIdx: 0, DriverCarFuelMaxLtr: 100 },
        { CarIdx: 1, DriverCarFuelMaxLtr: 80 },
        { CarIdx: 2, DriverCarFuelMaxLtr: 88 },
      ],
    },
  };
  const s = parseSessionFromYaml(yaml);
  assert.equal(s!.fuelMaxL, 88);
});

test("falls back to first driver with fuel max if PlayerCarIdx row missing it", () => {
  const yaml = {
    SessionNum: 0,
    PlayerCarIdx: 5,
    SessionInfo: {
      Sessions: [
        { SessionType: "Race", SessionLaps: "10", SessionTime: "0" },
      ],
    },
    DriverInfo: {
      Drivers: [
        { CarIdx: 0, DriverCarFuelMaxLtr: 70 },
        { CarIdx: 5 },
      ],
    },
  };
  const s = parseSessionFromYaml(yaml);
  assert.equal(s!.fuelMaxL, 70);
});
