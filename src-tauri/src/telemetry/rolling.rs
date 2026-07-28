pub struct CappedBuffer<T> {
    buf: Vec<T>,
    cap: usize,
}

impl<T> CappedBuffer<T> {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "CappedBuffer cap must be > 0");
        Self {
            buf: Vec::with_capacity(cap),
            cap,
        }
    }

    pub fn push(&mut self, value: T) {
        if self.buf.len() >= self.cap {
            self.buf.remove(0);
        }
        self.buf.push(value);
    }

    pub fn values(&self) -> &[T] {
        &self.buf
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

pub fn median(values: &[f64]) -> f64 {
    assert!(!values.is_empty(), "median() requires at least one value");
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() >> 1;
    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    }
}

pub fn consecutive_deltas(oldest_first: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(oldest_first.len().saturating_sub(1));
    for i in 0..oldest_first.len().saturating_sub(1) {
        out.push(oldest_first[i] - oldest_first[i + 1]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_odd_length() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
    }

    #[test]
    fn median_even_length() {
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), 2.5);
    }

    #[test]
    fn median_ignores_input_order() {
        assert_eq!(median(&[10.0, 90.0, 50.0, 30.0, 70.0]), 50.0);
    }

    #[test]
    #[should_panic(expected = "median() requires at least one value")]
    fn median_empty_panics() {
        median(&[]);
    }

    #[test]
    fn capped_buffer_caps_at_size() {
        let mut buf = CappedBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        buf.push(4);
        assert_eq!(buf.values(), &[2, 3, 4]);
        assert_eq!(buf.values().len(), 3);
    }

    #[test]
    fn capped_buffer_clear() {
        let mut buf = CappedBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.clear();
        assert_eq!(buf.values().len(), 0);
        assert!(buf.values().is_empty());
    }

    #[test]
    #[should_panic(expected = "CappedBuffer cap must be > 0")]
    fn capped_buffer_rejects_zero_cap() {
        CappedBuffer::<i32>::new(0);
    }

    #[test]
    fn consecutive_deltas_oldest_to_newest() {
        assert_eq!(consecutive_deltas(&[5.0, 3.0, 1.0]), [2.0, 2.0]);
    }

    #[test]
    fn consecutive_deltas_empty() {
        let r = consecutive_deltas(&[]);
        assert!(r.is_empty());
    }

    #[test]
    fn consecutive_deltas_single() {
        let r = consecutive_deltas(&[5.0]);
        assert!(r.is_empty());
    }
}
