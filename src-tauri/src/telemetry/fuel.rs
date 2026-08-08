use super::rolling::{consecutive_deltas, median, CappedBuffer};

pub struct SFInputs<'a> {
    pub fuel_max_l: f64,
    pub fuel_level_l: f64,
    pub last_lap_time_s: f64,
    pub time_remain_s: f64,
    pub time_total_s: f64,
    pub time_elapsed_s: f64,
    pub laps_remaining: i32,
    pub fuel_history: &'a mut CappedBuffer<f64>,
    pub lap_time_history: &'a mut CappedBuffer<f64>,
}

pub struct SFResult {
    pub fuel_level_l: f64,
    pub laps_left: i32,
    pub fuel_per_lap: f64,
    pub fuel_needed_l: f64,
    pub refuel_l: f64,
    pub fits_in_tank: bool,
    pub confidence: &'static str,
}

/// iRacing reports this as "no time limit" in `SessionTimeRemain` (7 days).
const TIME_UNLIMITED_S: f64 = 604800.0;

/// Laps remaining in the session, using iRacing telemetry directly.
///
/// `SessionLapsRemainEx` reports the laps left till the session ends, but only
/// for lap-limited sessions: iRacing reports `32767` (and occasionally `-1` or
/// `0`) when the session is not lap-limited, so only sane lap counts are
/// accepted. For time-limited sessions the primary source is `SessionTimeRemain`
/// (iRacing documents it as `-1` until the session state is Racing, and `604800`
/// for sessions without a time limit), falling back to
/// `SessionTimeTotal - SessionTime` when it is unusable.
fn compute_remaining_laps(
    laps_remaining: i32,
    time_remain_s: f64,
    time_total_s: f64,
    time_elapsed_s: f64,
    lap_times: &[f64],
) -> i32 {
    if (1..=1000).contains(&laps_remaining) {
        return laps_remaining;
    }

    let remaining = if time_remain_s > 0.0 && time_remain_s < TIME_UNLIMITED_S {
        time_remain_s
    } else if time_total_s > 0.0 && time_total_s < TIME_UNLIMITED_S {
        (time_total_s - time_elapsed_s).max(0.0)
    } else {
        0.0
    };

    if remaining > 0.0 && !lap_times.is_empty() {
        let med = median(lap_times);
        if med > 0.0 {
            return (remaining / med).ceil().max(0.0) as i32;
        }
    }

    0
}

pub fn compute_on_sf_crossing(inputs: &mut SFInputs) -> SFResult {
    let fuel_level_l = inputs.fuel_level_l;

    inputs.fuel_history.push(fuel_level_l);
    inputs.lap_time_history.push(inputs.last_lap_time_s);

    let fuel_deltas = consecutive_deltas(inputs.fuel_history.values());
    let lap_times = inputs.lap_time_history.values();

    let have_fuel_data = !fuel_deltas.is_empty();
    let fuel_per_lap = if have_fuel_data {
        median(&fuel_deltas)
    } else {
        0.0
    };

    let laps_left = compute_remaining_laps(
        inputs.laps_remaining,
        inputs.time_remain_s,
        inputs.time_total_s,
        inputs.time_elapsed_s,
        lap_times,
    );

    let fuel_needed = (laps_left as f64) * fuel_per_lap;
    let refuel_l = fuel_needed - fuel_level_l;
    let fits_in_tank = fuel_level_l + refuel_l <= inputs.fuel_max_l + 1e-6;

    let confidence = if fuel_deltas.len() >= 3 && lap_times.len() >= 3 {
        "high"
    } else {
        "low"
    };

    SFResult {
        fuel_level_l,
        laps_left,
        fuel_per_lap,
        fuel_needed_l: fuel_needed,
        refuel_l,
        fits_in_tank,
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create an SFInputs borrowing the given buffers.
    fn with_bufs<'a>(
        fh: &'a mut CappedBuffer<f64>,
        lh: &'a mut CappedBuffer<f64>,
        overrides: impl FnOnce(&mut SFInputs),
    ) -> SFInputs<'a> {
        let mut inputs = SFInputs {
            fuel_max_l: 100.0,
            fuel_level_l: 50.0,
            last_lap_time_s: 90.0,
            time_remain_s: 1800.0,
            time_total_s: 0.0,
            time_elapsed_s: 0.0,
            laps_remaining: -1,
            fuel_history: fh,
            lap_time_history: lh,
        };
        overrides(&mut inputs);
        inputs
    }

    #[test]
    fn first_sf_confidence_low_fuel_per_lap_zero() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |_| {});
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.confidence, "low");
        assert_eq!(r.fuel_per_lap, 0.0);
        assert_eq!(r.laps_left, 20);
        assert!((r.fuel_level_l - 50.0).abs() < 1e-9);
    }

    #[test]
    fn five_stable_laps_time_limited() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);

        for i in 0..5 {
            let mut ins = with_bufs(&mut fh, &mut lh, |x| {
                x.fuel_level_l = 60.0 - (i as f64) * 2.0;
                x.last_lap_time_s = 90.0;
                x.time_remain_s = 1800.0 - (i as f64) * 90.0;
            });
            compute_on_sf_crossing(&mut ins);
        }

        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.fuel_level_l = 50.0;
            x.last_lap_time_s = 90.0;
            x.time_remain_s = 1350.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert!((r.fuel_per_lap - 2.0).abs() < 1e-9);
        assert_eq!(r.laps_left, (1350.0_f64 / 90.0_f64).ceil() as i32);
        assert_eq!(r.confidence, "high");
    }

    #[test]
    fn lap_limited() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);

        for i in 0..5 {
            let mut ins = with_bufs(&mut fh, &mut lh, |x| {
                x.fuel_level_l = 50.0 - (i as f64) * 2.0;
                x.last_lap_time_s = 80.0;
                x.laps_remaining = 24 - i;
                x.time_remain_s = 0.0;
            });
            compute_on_sf_crossing(&mut ins);
        }

        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.fuel_level_l = 40.0;
            x.last_lap_time_s = 80.0;
            x.laps_remaining = 22;
            x.time_remain_s = 0.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 22);
        assert!((r.fuel_per_lap - 2.0).abs() < 1e-9);
        assert!(r.refuel_l > 0.0);
    }

    #[test]
    fn refuel_negative_when_enough_fuel() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);

        for i in 0..5 {
            let mut ins = with_bufs(&mut fh, &mut lh, |x| {
                x.fuel_level_l = 90.0 - (i as f64) * 2.0;
                x.last_lap_time_s = 80.0;
                x.laps_remaining = 3;
                x.time_remain_s = 0.0;
            });
            compute_on_sf_crossing(&mut ins);
        }

        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.fuel_level_l = 80.0;
            x.last_lap_time_s = 80.0;
            x.laps_remaining = 3;
            x.time_remain_s = 0.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert!((r.fuel_per_lap - 2.0).abs() < 1e-9);
        assert_eq!(r.laps_left, 3);
        let expected_refuel = 6.0 - 80.0;
        assert!((r.refuel_l - expected_refuel).abs() < 1e-9);
        assert!(r.fits_in_tank);
    }

    #[test]
    fn pit_lap_spike_trust_median() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let fuel = [0.5, 0.48, 0.46, 0.44, 0.42, 0.4, 0.6, 0.58, 0.56, 0.54, 0.52];
        let lap_times = [90.0; 11];

        for i in 0..fuel.len() {
            let mut ins = with_bufs(&mut fh, &mut lh, |x| {
                x.fuel_level_l = fuel[i] * 100.0;
                x.last_lap_time_s = lap_times[i];
                x.laps_remaining = 10;
                x.time_remain_s = 0.0;
            });
            compute_on_sf_crossing(&mut ins);
        }

        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.fuel_level_l = 50.0;
            x.last_lap_time_s = 90.0;
            x.laps_remaining = 10;
            x.time_remain_s = 0.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert!((r.fuel_per_lap - 2.0).abs() < 1e-9);
    }

    #[test]
    fn time_based_fallback_without_session_info() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.time_remain_s = 1800.0;
            x.last_lap_time_s = 90.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 20);
        assert!((r.fuel_level_l - 50.0).abs() < 1e-9);
    }

    #[test]
    fn frame_laps_remaining_used_without_session_info() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.laps_remaining = 18;
            x.time_remain_s = 0.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 18);
    }

    #[test]
    fn laps_remaining_sentinel_32767_falls_back_to_time() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.laps_remaining = 32767;
            x.time_remain_s = 0.0;
            x.time_total_s = 900.0;
            x.time_elapsed_s = 0.0;
            x.last_lap_time_s = 90.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 10);
    }

    #[test]
    fn session_time_remain_is_primary_source() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.laps_remaining = 32767;
            x.time_remain_s = 450.0;
            x.time_total_s = 900.0;
            x.time_elapsed_s = 450.0;
            x.last_lap_time_s = 90.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 5);
    }

    #[test]
    fn time_limit_from_total_minus_elapsed() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.laps_remaining = 32767;
            x.time_remain_s = 0.0;
            x.time_total_s = 900.0;
            x.time_elapsed_s = 540.0;
            x.last_lap_time_s = 90.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 4);
    }

    #[test]
    fn zero_laps_remaining_is_not_lap_limited() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.laps_remaining = 0;
            x.time_remain_s = 0.0;
            x.time_total_s = 900.0;
            x.time_elapsed_s = 540.0;
            x.last_lap_time_s = 90.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 4);
    }

    #[test]
    fn unlimited_time_sentinel_604800_ignored() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.laps_remaining = 32767;
            x.time_remain_s = 604800.0;
            x.time_total_s = 0.0;
            x.last_lap_time_s = 90.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 0);
    }

    #[test]
    fn time_total_sentinel_604800_ignored() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.laps_remaining = 32767;
            x.time_remain_s = -1.0;
            x.time_total_s = 604800.0;
            x.time_elapsed_s = 0.0;
            x.last_lap_time_s = 90.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 0);
    }

    #[test]
    fn time_limited_no_lap_history_returns_zero() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.time_remain_s = 1800.0;
            x.last_lap_time_s = 0.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 0);
    }
}
