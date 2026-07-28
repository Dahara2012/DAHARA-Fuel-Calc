import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isFuelState, type FuelState, type SidecarEvent } from "@dahara/shared";

type Mood = "ok" | "bad" | "idle";

const root = document.getElementById("root") as HTMLElement;
const valueEl = document.getElementById("value") as HTMLElement;
const unitEl = document.getElementById("unit") as HTMLElement;

let lastMood: Mood = "idle";
let lastText = "";

function setMood(mood: Mood): void {
  if (mood === lastMood) return;
  root.classList.remove("ok", "bad", "idle");
  root.classList.add(mood);
  lastMood = mood;
}

function setValue(text: string): void {
  if (text === lastText) return;
  valueEl.textContent = text;
  lastText = text;
}

function formatLiters(n: number): string {
  if (!Number.isFinite(n)) return "—";
  const rounded = Math.round(n * 10) / 10;
  return rounded.toFixed(1);
}

function render(state: FuelState): void {
  // While we don't have enough samples, the raw refuel number is
  // meaningless (often negative — see fuel.ts::computeOnSFCrossing).
  // Show 0.0 in gray until the math is reliable.
  if (state.confidence === "low") {
    setValue("0.0");
    setMood("idle");
    return;
  }

  const refuel = Math.max(0, state.refuelL);
  setValue(formatLiters(refuel));
  if (refuel <= 0) {
    setMood("ok");
  } else if (state.fitsInTank) {
    setMood("ok");
  } else {
    setMood("bad");
  }
}

function renderIdle(): void {
  setValue("—");
  setMood("idle");
}

async function main(): Promise<void> {
  unitEl.textContent = "L";

  let unlisten: UnlistenFn | null = null;
  try {
    unlisten = await listen<SidecarEvent>("fuel", (event) => {
      // When the user is not in a race session (garage, practice, pits),
      // reset the display to "—". This is safe because status events are
      // throttled to every ~0.5 s, and renderIdle() is idempotent.
      if (
        event.payload.type === "status" &&
        event.payload.inRace === false
      ) {
        renderIdle();
        return;
      }

      // Only FuelState events ("state" type) drive a fuel display update.
      if (isFuelState(event.payload)) {
        render(event.payload);
      }
    });
  } catch (err) {
    console.error("[renderer] failed to subscribe to fuel events:", err);
  }

  window.addEventListener("beforeunload", () => {
    unlisten?.();
  });

  renderIdle();
}

main();

