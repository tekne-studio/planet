//! A stream of numbers drawn from a key: everything flora decides by chance
//! is drawn from one, so the same key decides the same way on every machine.

/// Splitmix64: one multiply-xorshift round a draw, exact in integers.
#[derive(Clone, Debug)]
pub(crate) struct Stream(u64);

impl Stream {
    /// The stream a key starts, the key folded from its parts in order.
    pub(crate) fn new(parts: &[u64]) -> Stream {
        Stream(
            parts
                .iter()
                .fold(0x6a09_e667_f3bc_c909, |key, &part| mix(key ^ part)),
        )
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.0)
    }

    /// A number in `0..1`, with the 53 bits an `f64` holds exactly.
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// A number in `0..n`.
    pub(crate) fn below(&mut self, n: u32) -> u32 {
        (((self.next() >> 32) * u64::from(n)) >> 32) as u32
    }

    /// A number in `-1..1`: how far a measure wanders from its middle.
    pub(crate) fn wander(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }
}

fn mix(x: u64) -> u64 {
    let x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_draws_the_same_numbers_and_another_does_not() {
        let draw = |parts: &[u64]| {
            let mut stream = Stream::new(parts);
            [stream.next(), stream.next(), stream.next()]
        };
        assert_eq!(draw(&[1, 2, 3]), draw(&[1, 2, 3]));
        assert_ne!(draw(&[1, 2, 3]), draw(&[1, 3, 2]));
    }

    #[test]
    fn draws_stay_in_their_ranges() {
        let mut stream = Stream::new(&[7]);
        for _ in 0..10_000 {
            assert!((0.0..1.0).contains(&stream.unit()));
            assert!(stream.below(5) < 5);
            assert!((-1.0..1.0).contains(&stream.wander()));
        }
    }
}
