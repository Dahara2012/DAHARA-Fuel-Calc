import type { SessionInfo, SessionKind } from "@dahara/shared";

type AnySession = Record<string, unknown>;

const SESSION_TYPE_MAP: Record<string, SessionKind> = {
  Race: "race",
  Practice: "practice",
  Qualify: "qualifying",
  OfflineTesting: "practice",
};

function asNumber(v: unknown): number | null {
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "string") {
    const trimmed = v.trim();
    if (trimmed === "" || trimmed.toLowerCase() === "unlimited") return null;
    const n = Number(trimmed);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

function asString(v: unknown): string | null {
  return typeof v === "string" ? v : null;
}

export function parseSessionFromYaml(yaml: unknown): SessionInfo | null {
  if (!yaml || typeof yaml !== "object") return null;
  const root = yaml as AnySession;

  const sessionList = root["SessionInfo"];
  if (!sessionList || typeof sessionList !== "object") return null;
  const sessions = (sessionList as AnySession)["Sessions"];
  if (!Array.isArray(sessions) || sessions.length === 0) return null;

  const sessionNumRaw = root["SessionNum"];
  const sessionNum =
    typeof sessionNumRaw === "number" ? sessionNumRaw : 0;
  const session = sessions[Math.min(sessionNum, sessions.length - 1)] as
    | AnySession
    | undefined;
  if (!session) return null;

  const sessionTypeStr = asString(session["SessionType"]) ?? "";
  const kind: SessionKind = SESSION_TYPE_MAP[sessionTypeStr] ?? "other";

  const sessionLapsRaw = session["SessionLaps"];
  let sessionLaps: number | null = null;
  if (typeof sessionLapsRaw === "string") {
    sessionLaps = asNumber(sessionLapsRaw);
  } else if (typeof sessionLapsRaw === "number") {
    sessionLaps = asNumber(sessionLapsRaw);
  }

  const sessionTimeRaw = session["SessionTime"];
  let sessionTimeSec: number | null = null;
  if (typeof sessionTimeRaw === "string") {
    sessionTimeSec = asNumber(sessionTimeRaw);
  } else if (typeof sessionTimeRaw === "number") {
    sessionTimeSec = asNumber(sessionTimeRaw);
  }

  const driverInfo = root["DriverInfo"];
  let fuelMaxL = 0;
  if (driverInfo && typeof driverInfo === "object") {
    const drivers = (driverInfo as AnySession)["Drivers"];
    if (Array.isArray(drivers)) {
      const playerCarIdx = asNumber(root["PlayerCarIdx"]);
      if (playerCarIdx !== null) {
        for (const d of drivers) {
          if (d && typeof d === "object") {
            const carIdx = asNumber((d as AnySession)["CarIdx"]);
            const carFuelMaxLtr = asNumber(
              (d as AnySession)["DriverCarFuelMaxLtr"],
            );
            if (
              carIdx === playerCarIdx &&
              carFuelMaxLtr !== null &&
              carFuelMaxLtr > 0
            ) {
              fuelMaxL = carFuelMaxLtr;
              break;
            }
          }
        }
      }
      if (fuelMaxL === 0) {
        for (const d of drivers) {
          if (d && typeof d === "object") {
            const carFuelMaxLtr = asNumber(
              (d as AnySession)["DriverCarFuelMaxLtr"],
            );
            if (carFuelMaxLtr !== null && carFuelMaxLtr > 0) {
              fuelMaxL = carFuelMaxLtr;
              break;
            }
          }
        }
      }
    }
  }

  return {
    kind,
    inRace: kind === "race",
    fuelMaxL,
    sessionLaps,
    sessionTimeSec,
  };
}
