//! What trails a target and closes on it: critically damped, solved in
//! closed form, so a long frame settles further and never past, and nothing
//! bounces. The figure and the eye after a step, a body in a way after the
//! ground under it.

/// How far behind its target something is, and how fast that closes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Spring {
    pub(crate) off: f64,
    pub(crate) rate: f64,
}

impl Spring {
    /// The target moved by `moved` at once; what trails it stays where it was.
    pub(crate) fn left_by(&mut self, moved: f64) {
        self.off -= moved;
    }

    /// Closes the gap at `per_s`, exactly for any `dt`.
    pub(crate) fn settle(&mut self, per_s: f64, dt: f64) {
        let pull = (self.rate + per_s * self.off) * dt;
        let decay = (-per_s * dt).exp();
        self.off = (self.off + pull) * decay;
        self.rate = (self.rate - per_s * pull) * decay;
    }
}

/// A value that follows a target: where the target goes, it is left behind
/// and closes at the spring's pace.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Follow {
    target: f64,
    spring: Spring,
}

impl Follow {
    /// Resting on `target`.
    pub(crate) fn at(target: f64) -> Follow {
        Follow {
            target,
            spring: Spring::default(),
        }
    }

    pub(crate) fn value(&self) -> f64 {
        self.target + self.spring.off
    }

    /// The target is now `target`: follows it for `dt` at `per_s`, and
    /// returns where it is.
    pub(crate) fn follow(&mut self, target: f64, per_s: f64, dt: f64) -> f64 {
        self.spring.left_by(target - self.target);
        self.target = target;
        self.spring.settle(per_s, dt);
        self.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_follow_closes_on_a_step_without_passing_it() {
        let mut follow = Follow::at(0.0);
        let mut last = 0.0;
        for _ in 0..120 {
            let now = follow.follow(1.0, 6.0, 1.0 / 60.0);
            assert!(now >= last && now <= 1.0, "{now}");
            last = now;
        }
        assert!((last - 1.0).abs() < 1e-3, "{last}");
    }
}
