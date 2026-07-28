use super::rolling::{consecutive_deltas, median, CappedBuffer};

pub struct SFInputs<'a> {
    pub fuel_max_l: f64,
    pub current_fuel_pct: f64,
    pub last_lap_time_s: f64,
    pub time_remain_s: f64,
    pub laps_remaining: i32,
    pub session_laps: Option<i32>,
    pub session_time_sec: Option<f64>,
    pub fuel_history: &'a mut CappedBuffer<f64>,
    pub lap_time_history: &'a mut CappedBuffer<f64>,
}

pub struct SFResult {
    pub fuel_level_l: f64,
    pub laps_left: i32,
    pub fuel_per_lap: f64,
    pub refuel_l: f64,
    pub fits_in_tank: bool,
    pub confidence: &'static str,
}

fn compute_remaining_laps(
    time_remain_s: f64,
    laps_remaining: i32,
    session_laps: Option<i32>,
    session_time_sec: Option<f64>,
    lap_times: &[f64],
) -> i32 {
    let has_time_limit = session_time_sec.is_some_and(|s| s > 0.0);
    let has_lap_limit = session_laps.is_some_and(|l| l > 0);

    if has_time_limit && !has_lap_limit {
        if lap_times.is_empty() {
            return 0;
        }
        let med = median(lap_times);
        if med <= 0.0 {
            return 0;
        }
        return (time_remain_s / med).ceil().max(0.0) as i32;
    }

    if has_lap_limit {
        return laps_remaining.max(0);
    }

    0
}

pub fn compute_on_sf_crossing(inputs: &mut SFInputs) -> SFResult {
    let fuel_level_l = (inputs.current_fuel_pct / 100.0) * inputs.fuel_max_l;

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
        inputs.time_remain_s,
        inputs.laps_remaining,
        inputs.session_laps,
        inputs.session_time_sec,
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
            current_fuel_pct: 50.0,
            last_lap_time_s: 90.0,
            time_remain_s: 1800.0,
            laps_remaining: 25,
            session_laps: None,
            session_time_sec: Some(1800.0),
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
                x.current_fuel_pct = 60.0 - (i as f64) * 2.0;
                x.last_lap_time_s = 90.0;
                x.time_remain_s = 1800.0 - (i as f64) * 90.0;
                x.session_time_sec = Some(1800.0);
            });
            compute_on_sf_crossing(&mut ins);
        }

        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.current_fuel_pct = 50.0;
            x.last_lap_time_s = 90.0;
            x.time_remain_s = 1350.0;
            x.session_time_sec = Some(1800.0);
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
                x.current_fuel_pct = 50.0 - (i as f64) * 2.0;
                x.last_lap_time_s = 80.0;
                x.session_laps = Some(25);
                x.session_time_sec = None;
                x.time_remain_s = 0.0;
            });
            compute_on_sf_crossing(&mut ins);
        }

        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.current_fuel_pct = 40.0;
            x.last_lap_time_s = 80.0;
            x.laps_remaining = 22;
            x.session_laps = Some(25);
            x.session_time_sec = None;
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
                x.current_fuel_pct = 90.0 - (i as f64) * 2.0;
                x.last_lap_time_s = 80.0;
                x.session_laps = Some(3);
                x.session_time_sec = None;
                x.time_remain_s = 0.0;
            });
            compute_on_sf_crossing(&mut ins);
        }

        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.current_fuel_pct = 80.0;
            x.last_lap_time_s = 80.0;
            x.laps_remaining = 3;
            x.session_laps = Some(3);
            x.session_time_sec = None;
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
        let fuel = [50.0, 48.0, 46.0, 44.0, 42.0, 40.0, 60.0, 58.0, 56.0, 54.0, 52.0];
        let lap_times = [90.0; 11];

        for i in 0..fuel.len() {
            let mut ins = with_bufs(&mut fh, &mut lh, |x| {
                x.current_fuel_pct = fuel[i];
                x.last_lap_time_s = lap_times[i];
                x.session_laps = Some(10);
                x.session_time_sec = None;
                x.time_remain_s = 0.0;
            });
            compute_on_sf_crossing(&mut ins);
        }

        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.current_fuel_pct = 50.0;
            x.last_lap_time_s = 90.0;
            x.session_laps = Some(10);
            x.session_time_sec = None;
            x.time_remain_s = 0.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert!((r.fuel_per_lap - 2.0).abs() < 1e-9);
    }

    #[test]
    fn time_limited_no_lap_history_returns_zero() {
        let mut fh = CappedBuffer::new(5);
        let mut lh = CappedBuffer::new(5);
        let mut ins = with_bufs(&mut fh, &mut lh, |x| {
            x.session_laps = None;
            x.session_time_sec = Some(1800.0);
            x.time_remain_s = 1800.0;
            x.last_lap_time_s = 0.0;
        });
        let r = compute_on_sf_crossing(&mut ins);
        assert_eq!(r.laps_left, 0);
    }
}
