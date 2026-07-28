import { invoke, Channel } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { PhysicalPosition } from "@tauri-apps/api/dpi";

type FuelState = {
  type: "state";
  lap: number;
  fuelLevelL: number;
  fuelMaxL: number;
  lapTimeS: number;
  timeRemainS: number;
  lapsLeft: number;
  fuelPerLap: number;
  refuelL: number;
  fitsInTank: boolean;
  confidence: "high" | "low";
  timestamp: number;
};

type StatusEvent = {
  type: "status";
  connected: boolean;
};

type TelemetryEvent = FuelState | StatusEvent;

function isFuelState(e: TelemetryEvent): e is FuelState {
  return e.type === "state";
}

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

  let unlistenMove: UnlistenFn | null = null;

  try {
    unlistenMove = await listen<boolean>("move-mode", (event) => {
      console.log("[renderer] move-mode received:", event.payload);
      if (event.payload) {
        root.classList.add("move-mode");
      } else {
        root.classList.remove("move-mode");
      }
    });
  } catch (err) {
    console.error("[renderer] failed to subscribe to move-mode events:", err);
  }

  const channel = new Channel<TelemetryEvent>();
  channel.onmessage = (event) => {
    console.log("[telemetry] channel event:", JSON.stringify(event));
    if (isFuelState(event)) {
      render(event);
    }
  };

  invoke("start_telemetry", { channel }).catch((err) => {
    console.error("[renderer] failed to start telemetry:", err);
  });

  renderIdle();

  const win = getCurrentWebviewWindow();
  let isDragging = false;
  let startMouse = { x: 0, y: 0 };
  let startWinPos = { x: 0, y: 0 };
  let lastSetPos = { x: 0, y: 0 };

  root.addEventListener("mousedown", (e) => {
    if (!root.classList.contains("move-mode")) return;
    startMouse = { x: e.screenX, y: e.screenY };
    win.outerPosition().then((pos) => {
      startWinPos = lastSetPos = { x: pos.x, y: pos.y };
      isDragging = true;
    });
  });

  document.addEventListener("mousemove", (e) => {
    if (!isDragging) return;
    lastSetPos = {
      x: startWinPos.x + (e.screenX - startMouse.x),
      y: startWinPos.y + (e.screenY - startMouse.y),
    };
    win.setPosition(new PhysicalPosition(lastSetPos.x, lastSetPos.y));
  });

  document.addEventListener("mouseup", () => {
    if (!isDragging) return;
    isDragging = false;
    invoke("save_window_position", { x: lastSetPos.x, y: lastSetPos.y });
  });
}

main();

