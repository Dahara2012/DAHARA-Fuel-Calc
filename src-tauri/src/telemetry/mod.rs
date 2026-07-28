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
    #[field_name = "LapLastLapTime"]
    last_lap_time_s: f32,
    #[field_name = "SessionTimeRemain"]
    time_remain_s: f32,
    #[field_name = "LapCompleted"]
    lap_completed: i32,
    #[field_name = "SessionLapsRemainEx"]
    laps_remaining: i32,
}

// ── Telemetry Events ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
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
    session_laps: Option<i32>,
    session_time_sec: Option<f64>,
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
            session_laps: None,
            session_time_sec: None,
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

        self.session_laps = key.session_laps;
        self.session_time_sec = key.session_time_sec;
    }

    fn process_frame(&mut self, frame: &FuelTelemetry) -> Option<TelemetryEvent> {
        if self.detector.on_lap(frame.lap_completed) {
            eprintln!(
                "[telemetry] SF crossing: lap={}, fuel={:.0}%, max={:.1}L",
                frame.lap_completed,
                frame.fuel_level_pct * 100.0,
                self.fuel_max_l,
            );

            let mut ins = SFInputs {
                fuel_max_l: self.fuel_max_l,
                current_fuel_pct: frame.fuel_level_pct as f64,
                last_lap_time_s: frame.last_lap_time_s as f64,
                time_remain_s: frame.time_remain_s as f64,
                laps_remaining: frame.laps_remaining,
                session_laps: self.session_laps,
                session_time_sec: self.session_time_sec,
                fuel_history: &mut self.fuel_history,
                lap_time_history: &mut self.lap_time_history,
            };

            let result = compute_on_sf_crossing(&mut ins);

            eprintln!(
                "[telemetry] result: confidence={}, refuel={:.1}L, laps_left={}, fuel_per_lap={:.1}L",
                result.confidence, result.refuel_l, result.laps_left, result.fuel_per_lap,
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

pub async fn run_telemetry(
    conn: LiveConnection,
    channel: tauri::ipc::Channel<TelemetryEvent>,
) {
    let mut frame_stream = conn.subscribe::<FuelTelemetry>(UpdateRate::Native);
    let mut session_stream = Box::pin(conn.session_updates());

    let mut coordinator = TelemetryCoordinator::new();
    let mut last_status = Instant::now();
    let status_interval = Duration::from_millis(500);

    loop {
        tokio::select! {
            maybe_frame = frame_stream.next() => {
                let frame = match maybe_frame {
                    Some(f) => f,
                    None => break,
                };

                if last_status.elapsed() >= status_interval {
                    if channel.send(TelemetryEvent::Status {
                        connected: true,
                    }).is_err() {
                        break;
                    }
                    last_status = Instant::now();
                }

                if let Some(event) = coordinator.process_frame(&frame) {
                    if channel.send(event).is_err() {
                        break;
                    }
                }
            }
            maybe_session = session_stream.next() => {
                let session: Arc<SessionInfo> = match maybe_session {
                    Some(s) => s,
                    None => break,
                };
                coordinator.on_session_update(&session);
            }
        }
    }
}
