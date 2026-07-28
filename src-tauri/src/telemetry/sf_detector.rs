pub struct SFDetector {
    last_lap: i32,
}

impl SFDetector {
    pub fn new() -> Self {
        Self { last_lap: -1 }
    }

    /// Returns `true` if this lap is a valid S/F crossing (lap > last_lap).
    /// First valid lap does not fire. Zero/negative laps are ignored.
    pub fn on_lap(&mut self, lap: i32) -> bool {
        if lap <= 0 {
            return false;
        }
        if self.last_lap == -1 {
            self.last_lap = lap;
            return false;
        }
        if lap > self.last_lap {
            self.last_lap = lap;
            return true;
        }
        false
    }

    pub fn reset(&mut self) {
        self.last_lap = -1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_valid_lap_does_not_fire() {
        let mut d = SFDetector::new();
        assert!(!d.on_lap(1));
    }

    #[test]
    fn increasing_lap_fires_exactly_once() {
        let mut d = SFDetector::new();
        d.on_lap(1);
        assert!(d.on_lap(2));
        assert!(!d.on_lap(2));
    }

    #[test]
    fn same_lap_does_not_fire() {
        let mut d = SFDetector::new();
        d.on_lap(5);
        assert!(!d.on_lap(5));
        assert!(!d.on_lap(5));
    }

    #[test]
    fn zero_or_negative_lap_ignored() {
        let mut d = SFDetector::new();
        assert!(!d.on_lap(0));
        assert!(!d.on_lap(-1));
        d.on_lap(3);
        assert!(!d.on_lap(0));
        assert!(d.on_lap(4));
    }

    #[test]
    fn reset_clears_state() {
        let mut d = SFDetector::new();
        d.on_lap(1);
        d.reset();
        assert!(!d.on_lap(1));
    }
}
