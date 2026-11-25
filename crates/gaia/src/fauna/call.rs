//! An animal called: it sets off at the speed it had, gathers speed up to
//! the pace it is called at, slows as it nears whoever called it and
//! stands ([`Run`]). In closed form from how far it has to go and the time
//! since it set off, so every client that knows the call draws it at the
//! same place.
//!
//! A pet answers a whistle within [`ANSWERS_M`]. Within earshot,
//! [`super::HEARS_M`], it runs in from where it is seen; past earshot it is
//! out of sight a while, its wait growing a little with how far it was,
//! then comes into sight [`COMES_M`] from whoever called it and runs the
//! rest.

/// How far from whoever whistles a pet answers, metres along the ground:
/// farther, it does not come.
pub const ANSWERS_M: f64 = 500.0;

/// The fastest any animal is told to set off at, metres a second: what the
/// world takes of a call's speed.
pub const FASTEST_MPS: f64 = 30.0;

/// How far from whoever called it an animal called from past earshot
/// comes into sight, metres.
pub const COMES_M: f64 = 40.0;

/// How long an animal called from past earshot is out of sight before it
/// comes into it, seconds: a moment, a little more for each kilometre it
/// was away, under 1.75 s within [`ANSWERS_M`].
const COMING_S: f64 = 1.5;
const COMING_PER_KM_S: f64 = 0.5;

/// How long an animal `far_m` away when called is out of sight, seconds.
pub fn coming_s(far_m: f64) -> f64 {
    COMING_S + COMING_PER_KM_S * far_m.max(0.0) / 1000.0
}

/// The run of a called animal: from a standstill or on at the speed it
/// had, it gathers speed to `top_mps`, runs, and slows to a halt `far_m`
/// on, so that its speed never steps. A short run never reaches the top: a
/// few metres are a trot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    far_m: f64,
    from_mps: f64,
    /// The fastest it goes on this run.
    peak_mps: f64,
    accel_mps2: f64,
    /// How fast it slows over its last metres: its own, or harder, from a
    /// speed too high to stop at its own within `far_m`.
    slow_mps2: f64,
    gather_s: f64,
    gather_m: f64,
    flat_s: f64,
    flat_m: f64,
    slow_s: f64,
}

impl Run {
    /// A run over `far_m` set off at `from_mps`, at most `top_mps`, gaining
    /// speed at `accel_mps2` and losing it at `decel_mps2`.
    pub fn new(far_m: f64, from_mps: f64, top_mps: f64, accel_mps2: f64, decel_mps2: f64) -> Run {
        let far_m = far_m.max(0.0);
        let top_mps = top_mps.max(0.0);
        let from_mps = from_mps.clamp(0.0, top_mps);
        let still = Run {
            far_m,
            from_mps,
            peak_mps: from_mps,
            accel_mps2,
            slow_mps2: decel_mps2,
            gather_s: 0.0,
            gather_m: 0.0,
            flat_s: 0.0,
            flat_m: 0.0,
            slow_s: 0.0,
        };
        if far_m == 0.0 || top_mps == 0.0 || accel_mps2 <= 0.0 || decel_mps2 <= 0.0 {
            return still;
        }
        if from_mps * from_mps / (2.0 * decel_mps2) >= far_m {
            // Too fast to stop at its own pace: it slows the whole way.
            return Run {
                slow_mps2: from_mps * from_mps / (2.0 * far_m),
                slow_s: 2.0 * far_m / from_mps,
                ..still
            };
        }
        let (a, d) = (accel_mps2, decel_mps2);
        // The speed at which gathering up from `from_mps` and slowing down
        // to a halt cover `far_m` exactly, held to the top.
        let reach = (2.0 * a * d * far_m + d * from_mps * from_mps) / (a + d);
        let peak_mps = libm::sqrt(reach).min(top_mps);
        let gather_m = (peak_mps * peak_mps - from_mps * from_mps) / (2.0 * a);
        let slow_m = peak_mps * peak_mps / (2.0 * d);
        let flat_m = (far_m - gather_m - slow_m).max(0.0);
        Run {
            peak_mps,
            gather_s: (peak_mps - from_mps) / a,
            gather_m,
            flat_s: flat_m / peak_mps,
            flat_m,
            slow_s: peak_mps / d,
            ..still
        }
    }

    /// How long it takes, seconds.
    pub fn whole_s(&self) -> f64 {
        self.gather_s + self.flat_s + self.slow_s
    }

    /// Where it is `t_s` after it set off: how far along, metres, and how
    /// fast it goes, metres a second. Before it sets off it is at the
    /// start; once there, it stands at the end.
    pub fn at(&self, t_s: f64) -> (f64, f64) {
        let t = t_s.max(0.0);
        if t < self.gather_s {
            let along = self.from_mps * t + self.accel_mps2 * t * t / 2.0;
            return (along, self.from_mps + self.accel_mps2 * t);
        }
        let t = t - self.gather_s;
        if t < self.flat_s {
            return (self.gather_m + self.peak_mps * t, self.peak_mps);
        }
        let t = t - self.flat_s;
        if t < self.slow_s {
            let along =
                self.gather_m + self.flat_m + self.peak_mps * t - self.slow_mps2 * t * t / 2.0;
            return (along.min(self.far_m), self.peak_mps - self.slow_mps2 * t);
        }
        (self.far_m, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_called_from_afar_waits_a_little_longer_the_farther() {
        assert_eq!(coming_s(0.0), COMING_S);
        assert!(coming_s(ANSWERS_M) > coming_s(ANSWERS_M / 2.0));
        assert!(coming_s(ANSWERS_M) < 1.75 + 1e-9, "a moment, within reach");
    }

    /// A horse's: 9.6 m/s, gaining 3 m/s a second and losing 5.
    fn horse(far_m: f64, from_mps: f64) -> Run {
        Run::new(far_m, from_mps, 9.6, 3.0, 5.0)
    }

    /// Its speed sampled at 120 Hz over the whole run, past its end.
    fn speeds(run: &Run) -> Vec<(f64, f64)> {
        let steps = (run.whole_s() * 120.0) as usize + 120;
        (0..=steps)
            .map(|step| run.at(step as f64 / 120.0))
            .collect()
    }

    #[test]
    fn a_called_animal_gathers_runs_slows_and_stands_where_it_was_called() {
        for (far_m, from_mps) in [
            (100.0, 0.0),
            (100.0, 6.0),
            (4.0, 0.0),
            (4.0, 3.0),
            (1.0, 9.6),
        ] {
            let run = horse(far_m, from_mps);
            assert_eq!(run.at(0.0), (0.0, from_mps), "it sets off at its speed");
            let mut was = (0.0, from_mps);
            for (along, speed) in speeds(&run) {
                assert!(
                    along >= was.0 && along <= far_m + 1e-9,
                    "never back, never past"
                );
                assert!((0.0..=9.6 + 1e-9).contains(&speed));
                // No step: at most as hard as it slows from one too fast.
                let most = run.slow_mps2.max(3.0) / 120.0 + 1e-9;
                assert!(
                    (speed - was.1).abs() <= most,
                    "{far_m} m from {from_mps}: a step"
                );
                was = (along, speed);
            }
            assert!(
                (was.0 - far_m).abs() < 1e-9 && was.1 == 0.0,
                "it stands there"
            );
            let (end, _) = run.at(run.whole_s());
            assert!((end - far_m).abs() < 1e-9);
        }
        let long = horse(100.0, 0.0);
        assert_eq!(long.at(10.0).1, 9.6, "a long run gallops");
        let short = horse(4.0, 0.0);
        let top = speeds(&short)
            .iter()
            .map(|&(_, speed)| speed)
            .fold(0.0, f64::max);
        assert!(top < 4.0, "a few metres are a trot: {top} m/s");
    }
}
