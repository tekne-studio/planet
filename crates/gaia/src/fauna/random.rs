//! A small stream of numbers that is the same for the same seed, so an
//! animal and its herd are the same on every client.

/// Mixes bits into a well spread `u64`: splitmix64's finaliser.
pub fn mix(bits: u64) -> u64 {
    let mut z = bits.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Splitmix64 as a stream.
#[derive(Clone, Debug)]
pub struct Random(pub u64);

impl Random {
    /// A stream of its own for each of several numbers mixed together.
    pub fn of(parts: &[u64]) -> Random {
        Random(parts.iter().fold(0, |seed, part| mix(seed ^ part)))
    }

    pub fn draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// Uniform in `0..1`.
    pub fn unit(&mut self) -> f64 {
        (self.draw() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `from..to`.
    pub fn range(&mut self, (from, to): (f64, f64)) -> f64 {
        from + (to - from) * self.unit()
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// Uniform in `from..=to`.
    pub fn count(&mut self, (from, to): (usize, usize)) -> usize {
        from + (self.draw() % (to.saturating_sub(from) as u64 + 1)) as usize
    }

    /// An index into `weights`, each picked as often as its weight says.
    /// None from no weight at all.
    pub fn weighted(&mut self, weights: impl Iterator<Item = u32> + Clone) -> Option<usize> {
        let total: u64 = weights.clone().map(u64::from).sum();
        if total == 0 {
            return None;
        }
        let mut left = self.draw() % total;
        weights.enumerate().find_map(|(index, weight)| {
            let weight = u64::from(weight);
            if left < weight {
                Some(index)
            } else {
                left -= weight;
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_are_kept() {
        let mut random = Random(7);
        let mut seen = [0u32; 3];
        for _ in 0..30_000 {
            seen[random.weighted([1, 0, 2].into_iter()).unwrap()] += 1;
        }
        assert_eq!(seen[1], 0);
        let share = f64::from(seen[2]) / 30_000.0;
        assert!((0.64..0.69).contains(&share), "{share}");
    }
}
