mod fuel;
mod rolling;
mod sf_detector;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use pitwall::{LiveConnection, PitwallFrame, SessionInfo, UpdateRate};
use serde::{Deserialize, Serialize};

use self::fuel::{compute_on_sf_crossing, SFInputs};
use self::rolling::CappedBuffer;
use self::sf_detector::SFDetector;

/// Fallback fuel tank capacity (liters) used when iRacing session info
/// does not provide a value via `DriverInfo.DriverCarFuelMaxLtr`.
const DEFAULT_FUEL_MAX_L: f64 = 100.0;

// ── Pitwall Frame Type ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PitwallFrame)]
#[serde(rename_all = "camelCase")]
pub struct FuelTelemetry {
    #[field_name = "FuelLevelPct"]
    fuel_level_pct: f32,
    #[field_name = "FuelLevel"]
    fuel_level_l: f32,
    #[field_name = "LapLastLapTime"]
    last_lap_time_s: f32,
    #[field_name = "SessionTimeRemain"]
    time_remain_s: i32,
    #[field_name = "SessionTime"]
    time_elapsed_s: f64,
    #[field_name = "SessionTimeTotal"]
    time_total_s: f64,
    #[field_name = "LapCompleted"]
    lap_completed: i32,
    #[field_name = "SessionLapsRemainEx"]
    laps_remaining: i32,
    #[field_name = "SessionLapsTotal"]
    laps_total: i32,
    #[field_name = "SessionState"]
    session_state: i32,
}

// ── Telemetry Events ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TelemetryEvent {
    #[serde(rename = "state")]
    FuelState {
        lap: i32,
        fuel_level_l: f64,
        fuel_max_l: f64,
        lap_time_s: f64,
        time_remain_s: f64,
        laps_left: i32,
        fuel_per_lap: f64,
        refuel_l: f64,
        fits_in_tank: bool,
        confidence: String,
        timestamp: u64,
    },
    #[serde(rename = "status")]
    Status {
        connected: bool,
    },
}

// ── Internal session tracking ────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct SessionKey {
    kind: String,
    fuel_max_l: f64,
    session_laps: Option<i32>,
    session_time_sec: Option<f64>,
}

fn parse_laps(s: &str) -> Option<i32> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("unlimited") {
        return None;
    }
    t.parse::<i32>().ok().filter(|&n| n > 0)
}

fn parse_time(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("unlimited") {
        return None;
    }
    t.parse::<f64>().ok().filter(|&n| n > 0.0)
}

fn session_kind(session_type: &str) -> &str {
    match session_type {
        "Race" => "race",
        "Practice" | "OfflineTesting" => "practice",
        "Qualify" => "qualifying",
        _ => "other",
    }
}

fn get_fuel_max(session: &SessionInfo) -> f64 {
    session
        .driver_info
        .as_ref()
        .and_then(|di| {
            if let Some(fuel) = di.driver_car_fuel_max_ltr {
                if fuel > 0.0 {
                    return Some(fuel);
                }
            }
            None
        })
        .unwrap_or(0.0)
}

impl SessionKey {
    fn from_session(session: &SessionInfo) -> Self {
        let current = session.session_info.current_session_num.max(0) as usize;
        let session_data = session
            .session_info
            .sessions
            .get(current)
            .cloned()
            .unwrap_or_default();

        Self {
            kind: session_kind(&session_data.session_type).to_string(),
            fuel_max_l: get_fuel_max(session),
            session_laps: parse_laps(&session_data.session_laps),
            session_time_sec: parse_time(&session_data.session_time),
        }
    }

}

// ── Telemetry Coordinator ────────────────────────────────────────────

struct TelemetryCoordinator {
    detector: SFDetector,
    fuel_history: CappedBuffer<f64>,
    lap_time_history: CappedBuffer<f64>,
    last_session_key: SessionKey,
    fuel_max_l: f64,
}

impl TelemetryCoordinator {
    fn new() -> Self {
        Self {
            detector: SFDetector::new(),
            fuel_history: CappedBuffer::new(5),
            lap_time_history: CappedBuffer::new(5),
            last_session_key: SessionKey {
                kind: String::new(),
                fuel_max_l: 0.0,
                session_laps: None,
                session_time_sec: None,
            },
            fuel_max_l: DEFAULT_FUEL_MAX_L,
        }
    }

    fn reset_buffers(&mut self) {
        self.fuel_history.clear();
        self.lap_time_history.clear();
        self.detector.reset();
    }

    fn on_session_update(&mut self, session: &SessionInfo) {
        let key = SessionKey::from_session(session);
        if key == self.last_session_key {
            return;
        }
        self.last_session_key = key.clone();
        self.reset_buffers();

        if key.fuel_max_l > 0.0 {
            eprintln!(
                "[telemetry] session fuel capacity: {:.1} L (kind={}, laps={:?}, time={:?})",
                key.fuel_max_l, key.kind, key.session_laps, key.session_time_sec
            );
            self.fuel_max_l = key.fuel_max_l;
        } else {
            eprintln!(
                "[telemetry] session did not provide fuel capacity, keeping default {:.1} L",
                self.fuel_max_l,
            );
        }
    }

    fn process_frame(&mut self, frame: &FuelTelemetry) -> Option<TelemetryEvent> {
        if self.detector.on_lap(frame.lap_completed) {
            let fuel_level_l = frame.fuel_level_l as f64;
            let key = &self.last_session_key;
            eprintln!(
                "[telemetry] SF crossing: lap={}, fuel={:.1}%, level={:.1}L, max={:.1}L, last_lap={:.1}s, time_left={:.0}s, time_total={:.0}s, time_elapsed={:.0}s, laps_remaining={}, laps_total={}, state={} (session: {} laps={:?} time={:?})",
                frame.lap_completed,
                frame.fuel_level_pct * 100.0,
                fuel_level_l,
                self.fuel_max_l,
                frame.last_lap_time_s,
                frame.time_remain_s,
                frame.time_total_s,
                frame.time_elapsed_s,
                frame.laps_remaining,
                frame.laps_total,
                frame.session_state,
                key.kind,
                key.session_laps,
                key.session_time_sec,
            );

            let mut ins = SFInputs {
                fuel_max_l: self.fuel_max_l,
                fuel_level_l: frame.fuel_level_l as f64,
                last_lap_time_s: frame.last_lap_time_s as f64,
                time_remain_s: frame.time_remain_s as f64,
                time_total_s: frame.time_total_s,
                time_elapsed_s: frame.time_elapsed_s,
                laps_remaining: frame.laps_remaining,
                fuel_history: &mut self.fuel_history,
                lap_time_history: &mut self.lap_time_history,
            };

            let result = compute_on_sf_crossing(&mut ins);

            eprintln!(
                "[telemetry] inputs: fuel_hist=[{}] lap_hist=[{}]",
                format_values(self.fuel_history.values()),
                format_values(self.lap_time_history.values()),
            );
            eprintln!(
                "[telemetry] result: confidence={}, fuel_per_lap={:.1}L, laps_left={}, fuel_needed={:.1}L, refuel={:.1}L, fits_in_tank={}",
                result.confidence,
                result.fuel_per_lap,
                result.laps_left,
                result.fuel_needed_l,
                result.refuel_l,
                result.fits_in_tank,
            );

            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            return Some(TelemetryEvent::FuelState {
                lap: frame.lap_completed,
                fuel_level_l: result.fuel_level_l,
                fuel_max_l: self.fuel_max_l,
                lap_time_s: frame.last_lap_time_s as f64,
                time_remain_s: frame.time_remain_s as f64,
                laps_left: result.laps_left,
                fuel_per_lap: result.fuel_per_lap,
                refuel_l: result.refuel_l,
                fits_in_tank: result.fits_in_tank,
                confidence: result.confidence.to_string(),
                timestamp: ts,
            });
        }
        None
    }
}

// ── Public entry point ───────────────────────────────────────────────

fn format_values(values: &[f64]) -> String {
    let parts: Vec<String> = values
        .iter()
        .map(|v| format!("{v:.1}"))
        .collect();
    parts.join(", ")
}

pub async fn run_telemetry(
    conn: LiveConnection,
    channel: tauri::ipc::Channel<TelemetryEvent>,
) {
    let mut frame_stream = conn.subscribe::<FuelTelemetry>(UpdateRate::Native);
    let mut session_stream = Box::pin(conn.session_updates());

    let mut coordinator = TelemetryCoordinator::new();
    let mut last_status = Instant::now();
    let status_interval = Duration::from_millis(500);

    let mut first_frame = true;
    let mut frame_count: u64 = 0;
    let mut last_frame = Instant::now();
    let mut warned_stall = false;
    let mut last_heartbeat = Instant::now();
    const STALL_WARN: Duration = Duration::from_secs(30);
    const STALL_RECONNECT: Duration = Duration::from_secs(90);
    const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
    let mut watchdog = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            maybe_frame = frame_stream.next() => {
                let frame = match maybe_frame {
                    Some(f) => f,
                    None => {
                        eprintln!("[telemetry] ERROR: frame stream ended");
                        break;
                    }
                };

                last_frame = Instant::now();
                warned_stall = false;
                frame_count += 1;

                if first_frame {
                    first_frame = false;
                    eprintln!(
                        "[telemetry] first frame: lap_completed={}, fuel={:.1}%, last_lap={:.1}s, time_left={:.0}s, laps_remaining={}",
                        frame.lap_completed,
                        frame.fuel_level_pct * 100.0,
                        frame.last_lap_time_s,
                        frame.time_remain_s,
                        frame.laps_remaining,
                    );
                }

                if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
                    last_heartbeat = Instant::now();
                    eprintln!(
                        "[telemetry] alive: frames={}, lap={}, fuel={:.1}%",
                        frame_count,
                        frame.lap_completed,
                        frame.fuel_level_pct * 100.0,
                    );
                }

                if last_status.elapsed() >= status_interval {
                    if let Err(e) = channel.send(TelemetryEvent::Status {
                        connected: true,
                    }) {
                        eprintln!("[telemetry] ERROR: failed to send status event: {e}");
                        break;
                    }
                    last_status = Instant::now();
                }

                if let Some(event) = coordinator.process_frame(&frame) {
                    if let Err(e) = channel.send(event) {
                        eprintln!("[telemetry] ERROR: failed to send fuel event: {e}");
                        break;
                    }
                }
            }
            maybe_session = session_stream.next() => {
                let session: Arc<SessionInfo> = match maybe_session {
                    Some(s) => s,
                    None => {
                        eprintln!("[telemetry] ERROR: session stream ended");
                        break;
                    }
                };
                coordinator.on_session_update(&session);
            }
            _ = watchdog.tick() => {
                let stalled_for = last_frame.elapsed();
                if stalled_for > STALL_RECONNECT {
                    eprintln!(
                        "[telemetry] WARN: no telemetry frames for {:.0}s, reconnecting",
                        stalled_for.as_secs_f64()
                    );
                    break;
                }
                if stalled_for > STALL_WARN && !warned_stall {
                    warned_stall = true;
                    eprintln!(
                        "[telemetry] WARN: no telemetry frames for {:.0}s, still waiting",
                        stalled_for.as_secs_f64()
                    );
                }
            }
        }
    }
}
