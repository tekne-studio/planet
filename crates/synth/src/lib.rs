//! Sound made by code. A [`Voice`] is data: a few [`Layer`]s, each a source
//! (an oscillator's [`Wave`] at a [`Pitch`] with its glide and vibrato, or
//! noise), an [`Envelope`], a [`Filter`] whose edge may slide, and a gain.
//! Its owner writes it as a constant; nothing here knows what it is the
//! sound of.
//!
//! Given a seed and a rate, a voice renders to samples
//! ([`Voice::render`]): the seed moves each layer a little ([`Vary`]) and
//! seeds its noise, so two strikes of one voice are never one sample, and the
//! same seed gives the same bits on native and WASM, as the generator does.
//! A voice of a second or less is rendered once and played as a buffer.
//! A voice that holds, sustained for as long as its owner wants it, is
//! rendered once as a loop ([`Voice::looped`]) and played round.

mod render;
mod voice;

pub use voice::{
    Envelope, Filter, Glide, LAYERS, LONGEST_S, Layer, Pitch, Source, Vary, Vibrato, Voice, Wave,
};

impl Voice {
    /// Its data as bytes, name and every number: what a rendered buffer is
    /// kept by, with its seed. Two keys are equal only when the voices are,
    /// so no numbers a cartridge picks can reach another voice's buffer.
    pub fn key(&self) -> Vec<u8> {
        let mut key = Key::default();
        key.bytes(self.name.as_bytes());
        key.floats(&[self.vary.cents, self.vary.gain, self.vary.time]);
        for layer in self.layers.iter() {
            let envelope = layer.envelope;
            key.floats(&[
                layer.at_s,
                layer.gain,
                envelope.attack_s,
                envelope.hold_s,
                envelope.release_s,
                envelope.from,
                envelope.to,
            ]);
            match layer.source {
                Source::Noise => key.tag(0, &[]),
                Source::Wave(wave, pitch) => {
                    key.tag(1 + wave as u8, &[pitch.hz]);
                    let glide = pitch.glide.map(|glide| [glide.to_hz, glide.s]);
                    let vibrato = pitch.vibrato.map(|vibrato| [vibrato.hz, vibrato.cents]);
                    for part in [glide, vibrato] {
                        match part {
                            Some(numbers) => key.tag(1, &numbers),
                            None => key.tag(0, &[]),
                        }
                    }
                }
            }
            match layer.filter {
                Filter::Open => key.tag(0, &[]),
                Filter::Low { hz, q } => key.tag(1, &[hz, q]),
                Filter::High { hz, q } => key.tag(2, &[hz, q]),
                Filter::Band { hz, q } => key.tag(3, &[hz, q]),
            }
            match layer.sweep {
                Some(sweep) => key.tag(1, &[sweep.to_hz, sweep.s]),
                None => key.tag(0, &[]),
            }
        }
        key.0
    }
}

/// How long the end of a loop is faded into its start, seconds.
pub const LOOP_FADE_S: f32 = 0.25;

impl Voice {
    /// It rendered for `seed` as a loop of `s` seconds at `rate`, to play
    /// round without a seam: rendered [`LOOP_FADE_S`] longer, the overhang
    /// faded into the start at equal power, so the last sample runs on
    /// into the first. A voice to loop holds through `s` and the fade, its
    /// attack inside the fade. Equal power keeps a noise as loud across the
    /// fade; a steady tone whose cycles do not fit `s` meets itself out of
    /// phase there and dips, to silence midway at half a cycle. The same bits
    /// on every platform, as [`Voice::render`].
    pub fn looped(&self, seed: u32, rate: u32, s: f32) -> Vec<f32> {
        let length = (s.max(LOOP_FADE_S) * rate as f32) as usize;
        let fade = (LOOP_FADE_S * rate as f32) as usize;
        let mut out = vec![0.0; length + fade];
        self.render(seed, rate, &mut out);
        let (head, overhang) = out.split_at_mut(length);
        for (n, (start, over)) in head.iter_mut().zip(overhang.iter()).enumerate() {
            let t = core::f32::consts::FRAC_PI_2 * n as f32 / fade as f32;
            *start = *start * libm::sinf(t) + *over * libm::cosf(t);
        }
        out.truncate(length);
        out
    }
}

/// A voice's bytes, gathered in order.
#[derive(Default)]
struct Key(Vec<u8>);

impl Key {
    fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn floats(&mut self, floats: &[f32]) {
        for float in floats {
            self.bytes(&float.to_bits().to_le_bytes());
        }
    }

    /// Which of a choice, then its numbers.
    fn tag(&mut self, which: u8, numbers: &[f32]) {
        self.bytes(&[which]);
        self.floats(numbers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beep(hz: f32, to: f32) -> Voice {
        Voice::made(
            "beep",
            Vary::NONE,
            vec![Layer::new(
                0.0,
                Source::Wave(Wave::Square, Pitch::steady(hz)),
                Envelope::new(0.005, 0.1, 0.005).sloping(1.0, to),
                Filter::Open,
                0.5,
            )],
        )
    }

    #[test]
    fn a_key_is_the_same_only_for_the_same_voice() {
        assert_eq!(beep(440.0, 1.0).key(), beep(440.0, 1.0).key());
        assert_ne!(beep(440.0, 1.0).key(), beep(441.0, 1.0).key());
        assert_ne!(beep(440.0, 1.0).key(), beep(440.0, 0.5).key());
    }
}
