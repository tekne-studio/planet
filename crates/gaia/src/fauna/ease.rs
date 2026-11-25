//! Easing: a smooth step, and a follower that comes after a moving target.

/// `0` below `edge0`, `1` above `edge1`, smooth between.
pub(crate) fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Comes after a target that moves, critically damped and solved in closed
/// form: it starts and ends with ease, never passes the target, and a long
/// frame settles further.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Follow {
    /// Where the target was.
    last: f64,
    /// How far behind it what follows is, and how fast that closes.
    pub(crate) off: f64,
    rate: f64,
}

impl Follow {
    pub(crate) fn at(target: f64) -> Follow {
        Follow {
            last: target,
            ..Follow::default()
        }
    }

    pub(crate) fn value(&self) -> f64 {
        self.last + self.off
    }

    /// Leaves behind what the target moved since, then closes on it at
    /// `per_s` for `dt` seconds. Returns where what follows is.
    pub(crate) fn follow(&mut self, target: f64, per_s: f64, dt: f64) -> f64 {
        self.off -= target - self.last;
        self.last = target;
        let pull = (self.rate + per_s * self.off) * dt;
        let decay = libm::exp(-per_s * dt);
        self.off = (self.off + pull) * decay;
        self.rate = (self.rate - per_s * pull) * decay;
        target + self.off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_closes_without_passing() {
        let mut follow = Follow::at(0.0);
        let mut last = 0.0;
        for _ in 0..120 {
            let value = follow.follow(1.0, 16.0, 1.0 / 60.0);
            assert!(value <= 1.0 + 1e-9 && value >= last - 1e-12);
            last = value;
        }
        assert!((last - 1.0).abs() < 1e-3);
    }
}
