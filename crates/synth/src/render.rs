//! A voice rendered to samples: each layer its source, its filter and its
//! envelope, summed. Only `+ - * /` and `libm` touch a sample, so a seed
//! gives the same bits on every platform; nothing is allocated or read
//! from a clock.

use crate::voice::{Envelope, Filter, Glide, LONGEST_S, Layer, Pitch, Source, Vary, Voice, Wave};

/// Where a layer's release ends: -60 dB, then shifted so it lands on zero.
const FLOOR: f32 = 0.001;

/// The highest pitch an oscillator or a filter reaches, as a fraction of the
/// rate: below Nyquist, with room for the vibrato.
const TOP: f32 = 0.45;

/// The shortest a release is: a layer let go at once would end on a click.
const SHORTEST_RELEASE_S: f32 = 0.002;

/// A moment of a layer, `s` seconds stretched by its seed's `time`, in
/// samples: held within `0..=LONGEST_S`, and none for a time that is no
/// number.
fn samples(s: f32, time: f32, rate: f32) -> usize {
    ((s * time).clamp(0.0, LONGEST_S) * rate + 0.5) as usize
}

impl Voice {
    /// How many samples it lasts at `rate` for `seed`: to the end of its
    /// longest layer.
    pub fn length(&self, seed: u32, rate: u32) -> usize {
        let rate = rate as f32;
        (0..self.layers.len())
            .map(|index| {
                let spread = Spread::draw(self.vary, seed, index);
                Timing::of(&self.layers[index], &spread, rate).end()
            })
            .max()
            .unwrap_or(0)
    }

    /// Renders it into `out`, which is cleared first: every layer from its
    /// start to the end of its envelope, or of `out`, and the sum held
    /// within -1..1, silence where a number given was none. `out` as long
    /// as [`Voice::length`] holds all of it.
    pub fn render(&self, seed: u32, rate: u32, out: &mut [f32]) {
        out.fill(0.0);
        let rate = rate as f32;
        for (index, layer) in self.layers.iter().enumerate() {
            let spread = Spread::draw(self.vary, seed, index);
            render_layer(layer, &spread, rate, noise_seed(seed, index), out);
        }
        for sample in out.iter_mut() {
            *sample = if sample.is_nan() {
                0.0
            } else {
                sample.clamp(-1.0, 1.0)
            };
        }
    }
}

fn render_layer(layer: &Layer, spread: &Spread, rate: f32, noise: u32, out: &mut [f32]) {
    let timing = Timing::of(layer, spread, rate);
    let end = timing.end().min(out.len());
    let Some(span) = out.get_mut(timing.onset..end) else {
        return;
    };
    let mut source = Running::new(layer.source, spread, rate, noise);
    let mut filter = Edge::new(layer.filter, layer.sweep, spread, rate);
    let mut envelope = Contour::new(&timing, &layer.envelope);
    let gain = layer.gain * spread.gain;
    for sample in span {
        let raw = source.next();
        let coloured = match &mut filter {
            Some(filter) => filter.next(raw),
            None => raw,
        };
        *sample += coloured * envelope.next() * gain;
    }
}

/// What a seed does to one layer: a pitch ratio, and factors on its gain
/// and its times.
struct Spread {
    pitch: f32,
    gain: f32,
    time: f32,
}

impl Spread {
    fn draw(vary: Vary, seed: u32, layer: usize) -> Spread {
        let unit = |which: u64| unit(seed, layer, which);
        Spread {
            pitch: libm::exp2f(vary.cents * unit(0) / 1200.0),
            gain: 1.0 + vary.gain * unit(1),
            time: 1.0 + vary.time * unit(2),
        }
    }
}

/// A number in -1..1 from a seed, a layer and which draw: the same on every
/// platform, and far apart for neighbouring seeds.
fn unit(seed: u32, layer: usize, which: u64) -> f32 {
    let bits = mix(u64::from(seed) | (layer as u64) << 32 | which << 48);
    (bits >> 40) as f32 * (2.0 / 16_777_216.0) - 1.0
}

/// The noise of a layer: its own stream for each seed.
fn noise_seed(seed: u32, layer: usize) -> u32 {
    (mix(u64::from(seed) | (layer as u64) << 32 | 3 << 48) >> 32) as u32 | 1
}

/// SplitMix64's finaliser: every bit of the input moves half the output.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A layer's moments in samples, from the voice's start.
struct Timing {
    onset: usize,
    attack: usize,
    hold: usize,
    release: usize,
}

impl Timing {
    fn of(layer: &Layer, spread: &Spread, rate: f32) -> Timing {
        let samples = |s: f32| samples(s, spread.time, rate);
        let Envelope {
            attack_s,
            hold_s,
            release_s,
            ..
        } = layer.envelope;
        Timing {
            onset: samples(layer.at_s),
            attack: samples(attack_s),
            hold: samples(hold_s),
            release: samples(release_s.max(SHORTEST_RELEASE_S)),
        }
    }

    fn end(&self) -> usize {
        self.onset
            .saturating_add(self.attack)
            .saturating_add(self.hold)
            .saturating_add(self.release)
    }
}

/// A layer's source as it runs.
enum Running {
    Wave(Oscillator),
    Noise(Noise),
}

impl Running {
    fn new(source: Source, spread: &Spread, rate: f32, noise: u32) -> Running {
        match source {
            Source::Wave(wave, pitch) => Running::Wave(Oscillator::new(wave, pitch, spread, rate)),
            Source::Noise => Running::Noise(Noise(noise)),
        }
    }

    fn next(&mut self) -> f32 {
        match self {
            Running::Wave(oscillator) => oscillator.next(),
            Running::Noise(noise) => noise.next(),
        }
    }
}

/// A wave at a pitch: its phase in 0..1 and how far it moves a sample.
struct Oscillator {
    wave: Wave,
    phase: f32,
    step: f32,
    /// What the step is multiplied by each sample of the glide, and how
    /// many samples it has left.
    glide: f32,
    gliding: usize,
    vibrato: Option<Wavering>,
}

/// A vibrato as it runs: a slow sine on the step.
struct Wavering {
    phase: f32,
    step: f32,
    /// The step's swing at the sine's top: 2^(cents/1200) - 1.
    depth: f32,
}

impl Oscillator {
    fn new(wave: Wave, pitch: Pitch, spread: &Spread, rate: f32) -> Oscillator {
        let step_of = |hz: f32| (hz * spread.pitch / rate).clamp(0.0, TOP);
        let step = step_of(pitch.hz);
        let (glide, gliding) = match pitch.glide {
            Some(glide) if step > 0.0 => {
                let samples = samples(glide.s, spread.time, rate);
                let ratio = step_of(glide.to_hz) / step;
                match samples {
                    0 => (1.0, 0),
                    _ => (libm::powf(ratio, 1.0 / samples as f32), samples),
                }
            }
            _ => (1.0, 0),
        };
        Oscillator {
            wave,
            phase: 0.0,
            step,
            glide,
            gliding,
            vibrato: pitch.vibrato.map(|vibrato| Wavering {
                phase: 0.0,
                step: vibrato.hz / rate,
                depth: libm::exp2f(vibrato.cents / 1200.0) - 1.0,
            }),
        }
    }

    fn next(&mut self) -> f32 {
        let step = match &mut self.vibrato {
            Some(wavering) => {
                let swing = sine(wavering.phase);
                wavering.phase = wrap(wavering.phase + wavering.step);
                self.step * (1.0 + wavering.depth * swing)
            }
            None => self.step,
        };
        let t = self.phase;
        let out = match self.wave {
            Wave::Sine => sine(t),
            Wave::Triangle => triangle(t),
            Wave::Saw => 2.0 * t - 1.0 - blep(t, step),
            Wave::Square => {
                let level = if t < 0.5 { 1.0 } else { -1.0 };
                level + blep(t, step) - blep(wrap(t + 0.5), step)
            }
        };
        self.phase = wrap(t + step);
        if self.gliding > 0 {
            self.step *= self.glide;
            self.gliding -= 1;
        }
        out
    }
}

/// A phase back into 0..1, for a step under 1.
fn wrap(phase: f32) -> f32 {
    if phase >= 1.0 { phase - 1.0 } else { phase }
}

/// sin(2 pi t) for t in 0..1: folded onto a quarter turn around a zero,
/// then Taylor's polynomial to x^9, under 4e-6 off.
fn sine(t: f32) -> f32 {
    // sin(2 pi t) = -sin(2 pi (t - 1/2)), and past a quarter it mirrors.
    let p = t - 0.5;
    let q = if p > 0.25 {
        0.5 - p
    } else if p < -0.25 {
        -0.5 - p
    } else {
        p
    };
    let x = q * core::f32::consts::TAU;
    let x2 = x * x;
    let poly = 1.0 + x2 * (-1.0 / 6.0 + x2 * (1.0 / 120.0 + x2 * (-1.0 / 5040.0 + x2 / 362_880.0)));
    -(x * poly)
}

/// Up from 0 to 1 at a quarter, down to -1 at three, back to 0.
fn triangle(t: f32) -> f32 {
    if t < 0.25 {
        4.0 * t
    } else if t < 0.75 {
        2.0 - 4.0 * t
    } else {
        4.0 * t - 4.0
    }
}

/// The step a jump of 2 leaves out, smoothed over the sample either side
/// (PolyBLEP): a saw or a square without the alias a hard edge makes.
fn blep(t: f32, step: f32) -> f32 {
    if step <= 0.0 {
        0.0
    } else if t < step {
        let x = t / step;
        x + x - x * x - 1.0
    } else if t > 1.0 - step {
        let x = (t - 1.0) / step;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// White noise from xorshift32: the same stream for the same start.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
    }
}

/// How many samples a sliding edge keeps its coefficients: a step of a
/// slide too short to hear, and a sine and a cosine each.
const EDGE_BLOCK: usize = 32;

/// A filter as it runs: one second-order section, the cookbook's
/// (Bristow-Johnson), in transposed direct form II, its edge recomputed
/// every [`EDGE_BLOCK`] samples while it slides.
struct Edge {
    filter: Filter,
    rate: f32,
    hz: f32,
    /// What the edge is multiplied by each block of the slide, how many
    /// blocks it has left, and the samples of this block so far.
    slide: f32,
    sliding: usize,
    n: usize,
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Edge {
    /// `None` for a filter that lets everything through.
    fn new(filter: Filter, sweep: Option<Glide>, spread: &Spread, rate: f32) -> Option<Edge> {
        let hz = match filter {
            Filter::Open => return None,
            Filter::Low { hz, .. } | Filter::High { hz, .. } | Filter::Band { hz, .. } => hz,
        };
        let (slide, sliding) = match sweep {
            Some(glide) if hz > 0.0 => {
                let blocks = samples(glide.s, spread.time, rate) / EDGE_BLOCK;
                match blocks {
                    0 => (1.0, 0),
                    _ => (libm::powf(glide.to_hz / hz, 1.0 / blocks as f32), blocks),
                }
            }
            _ => (1.0, 0),
        };
        let mut edge = Edge {
            filter,
            rate,
            hz,
            slide,
            sliding,
            n: 0,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        };
        edge.place();
        Some(edge)
    }

    /// The coefficients for the edge at `hz`.
    fn place(&mut self) {
        let q = match self.filter {
            Filter::Open => return,
            Filter::Low { q, .. } | Filter::High { q, .. } | Filter::Band { q, .. } => q,
        };
        let w0 = core::f32::consts::TAU * self.hz.clamp(10.0, TOP * self.rate) / self.rate;
        let (sin, cos) = (libm::sinf(w0), libm::cosf(w0));
        let alpha = sin / (2.0 * q.max(0.1));
        let (b0, b1, b2) = match self.filter {
            Filter::Low { .. } => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0),
            Filter::High { .. } => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0),
            _ => (alpha, 0.0, -alpha),
        };
        let a0 = 1.0 + alpha;
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * cos / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    fn next(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        if self.sliding > 0 {
            self.n += 1;
            if self.n == EDGE_BLOCK {
                self.n = 0;
                self.hz *= self.slide;
                self.sliding -= 1;
                self.place();
            }
        }
        y
    }
}

/// An envelope as it runs, sample by sample.
struct Contour {
    attack: usize,
    held: usize,
    n: usize,
    /// Where the hold starts, what it adds a sample, and the level the
    /// release falls from: where the hold ends, or where the attack does
    /// when there is no hold, so a tone too short to slope never jumps.
    from: f32,
    step: f32,
    end: f32,
    /// The release's level before its shift to zero, and its ratio a sample.
    level: f32,
    fall: f32,
}

impl Contour {
    fn new(timing: &Timing, envelope: &Envelope) -> Contour {
        let fall = match timing.release {
            0 => 0.0,
            release => libm::powf(FLOOR, 1.0 / release as f32),
        };
        let Envelope { from, to, .. } = *envelope;
        let (step, end) = match timing.hold {
            0 => (0.0, from),
            _ if from == to => (0.0, to),
            hold => ((to - from) / hold as f32, to),
        };
        Contour {
            attack: timing.attack,
            held: timing.attack + timing.hold,
            n: 0,
            from,
            step,
            end,
            level: 1.0,
            fall,
        }
    }

    // A level that stays, `from` and `to` both 1, has a step of 0 and ends
    // at 1: the same bits as an envelope with no slope.
    fn next(&mut self) -> f32 {
        let n = self.n;
        self.n += 1;
        if n < self.attack {
            (n + 1) as f32 / self.attack as f32 * self.from
        } else if n < self.held {
            self.from + self.step * (n - self.attack) as f32
        } else {
            let out = (self.level - FLOOR) / (1.0 - FLOOR);
            self.level *= self.fall;
            out.max(0.0) * self.end
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_polynomial_sine_is_the_sine() {
        let worst = (0..4096)
            .map(|i| {
                let t = i as f32 / 4096.0;
                (sine(t) - libm::sinf(core::f32::consts::TAU * t)).abs()
            })
            .fold(0.0, f32::max);
        assert!(worst < 1e-5, "{worst}");
    }

    #[test]
    fn a_release_lands_on_zero() {
        let timing = Timing {
            onset: 0,
            attack: 0,
            hold: 0,
            release: 480,
        };
        let mut contour = Contour::new(&timing, &Envelope::new(0.0, 0.0, 0.01));
        let levels: Vec<f32> = (0..480).map(|_| contour.next()).collect();
        assert_eq!(levels[0], 1.0);
        assert!(levels.windows(2).all(|pair| pair[1] <= pair[0]));
        assert!(levels[479] < 1e-4, "{}", levels[479]);
        assert_eq!(contour.next(), 0.0, "past its end, silence");
    }

    #[test]
    fn a_seed_spreads_within_its_bounds() {
        let vary = Vary {
            cents: 100.0,
            gain: 0.2,
            time: 0.1,
        };
        for seed in 0..500 {
            let spread = Spread::draw(vary, seed, 0);
            assert!((0.943..=1.06).contains(&spread.pitch), "{}", spread.pitch);
            assert!((0.8..=1.2).contains(&spread.gain));
            assert!((0.9..=1.1).contains(&spread.time));
        }
        let a = Spread::draw(vary, 1, 0);
        let b = Spread::draw(vary, 2, 0);
        assert_ne!(a.pitch, b.pitch);
    }
}
