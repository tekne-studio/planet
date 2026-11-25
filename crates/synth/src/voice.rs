//! A voice as data: a short list of layers, written as a constant by whoever
//! owns the sound. No code a sound: what differs between two voices is
//! numbers.

use std::borrow::Cow;

/// The most layers a voice sums. A voice is a short list, and what it costs
/// to render is its layers times its length.
pub const LAYERS: usize = 8;

/// The longest a moment of a layer is, its start, attack, hold, release or
/// glide, seeds' stretch included: a voice is struck and over, and a longer
/// time is held to this.
pub const LONGEST_S: f32 = 30.0;

/// A sound made by code: a few layers summed, each one source shaped in
/// pitch, loudness and colour, and how far a seed moves them.
#[derive(Clone, PartialEq, Debug)]
pub struct Voice {
    /// What its owner calls it: how a command and a board name it.
    pub name: &'static str,
    /// How much a seed moves each layer, so no two strikes are one sample.
    pub vary: Vary,
    /// At most [`LAYERS`].
    pub layers: Cow<'static, [Layer]>,
}

impl Voice {
    /// A voice written as a constant. More than [`LAYERS`] layers does not
    /// compile.
    pub const fn new(name: &'static str, vary: Vary, layers: &'static [Layer]) -> Voice {
        assert!(layers.len() <= LAYERS, "a voice has at most LAYERS layers");
        Voice {
            name,
            vary,
            layers: Cow::Borrowed(layers),
        }
    }

    /// The voice louder or softer: each layer's gain times `gain`. A voice
    /// of another loudness is another voice, rendered on its own.
    pub fn scaled(&self, gain: f32) -> Voice {
        let layers = self.layers.iter().map(|layer| Layer {
            gain: layer.gain * gain,
            ..*layer
        });
        Voice {
            name: self.name,
            vary: self.vary,
            layers: Cow::Owned(layers.collect()),
        }
    }

    /// The voice higher or lower: every pitch and every filter times
    /// `ratio`, so its colour moves with it, as a smaller throat's does. A
    /// voice of another pitch is another voice, rendered on its own.
    pub fn pitched(&self, ratio: f32) -> Voice {
        let filter = |filter: Filter| match filter {
            Filter::Open => Filter::Open,
            Filter::Low { hz, q } => Filter::Low { hz: hz * ratio, q },
            Filter::High { hz, q } => Filter::High { hz: hz * ratio, q },
            Filter::Band { hz, q } => Filter::Band { hz: hz * ratio, q },
        };
        let source = |source: Source| match source {
            Source::Noise => Source::Noise,
            Source::Wave(wave, pitch) => Source::Wave(
                wave,
                Pitch {
                    hz: pitch.hz * ratio,
                    glide: pitch.glide.map(|glide| Glide {
                        to_hz: glide.to_hz * ratio,
                        ..glide
                    }),
                    ..pitch
                },
            ),
        };
        let layers = self.layers.iter().map(|layer| Layer {
            source: source(layer.source),
            filter: filter(layer.filter),
            ..*layer
        });
        Voice {
            name: self.name,
            vary: self.vary,
            layers: Cow::Owned(layers.collect()),
        }
    }

    /// A voice made while running, from numbers that come from elsewhere:
    /// a cartridge's tone. The layers past [`LAYERS`] are left out.
    pub fn made(name: &'static str, vary: Vary, mut layers: Vec<Layer>) -> Voice {
        layers.truncate(LAYERS);
        Voice {
            name,
            vary,
            layers: Cow::Owned(layers),
        }
    }
}

/// One source, shaped, starting at its own moment.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layer {
    /// When it starts, seconds after the voice does.
    pub at_s: f32,
    pub source: Source,
    pub envelope: Envelope,
    pub filter: Filter,
    /// How loud, 0 to 1. The voice is held within -1..1 at the end: the
    /// gains of layers that sound together stay within 1, or it clips.
    pub gain: f32,
    /// The filter's edge sliding from its `hz` to `to_hz` over `s` seconds
    /// from the layer's start, by equal ratios; none for an edge that
    /// stays. An open filter has no edge to slide.
    pub sweep: Option<Glide>,
}

impl Layer {
    /// A layer, its fields in the order a voice reads: when, what, how
    /// loud over its life, through what, how loud.
    pub const fn new(
        at_s: f32,
        source: Source,
        envelope: Envelope,
        filter: Filter,
        gain: f32,
    ) -> Layer {
        Layer {
            at_s,
            source,
            envelope,
            filter,
            gain,
            sweep: None,
        }
    }

    /// The same, its filter's edge sliding to `to_hz` over `s` seconds.
    pub const fn swept(self, to_hz: f32, s: f32) -> Layer {
        Layer {
            sweep: Some(Glide { to_hz, s }),
            ..self
        }
    }
}

/// What sounds: an oscillator at a pitch, or noise.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    Wave(Wave, Pitch),
    /// White noise, coloured by the layer's filter.
    Noise,
}

/// The shape an oscillator repeats.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wave {
    Sine,
    Triangle,
    Saw,
    /// Half up, half down.
    Square,
}

/// Where an oscillator sits, and how it moves from there.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pitch {
    pub hz: f32,
    pub glide: Option<Glide>,
    pub vibrato: Option<Vibrato>,
}

impl Pitch {
    /// A pitch that stays where it starts.
    pub const fn steady(hz: f32) -> Pitch {
        Pitch {
            hz,
            glide: None,
            vibrato: None,
        }
    }

    /// The same, sliding to `to_hz` over `s` seconds from the layer's start.
    pub const fn glide(self, to_hz: f32, s: f32) -> Pitch {
        Pitch {
            glide: Some(Glide { to_hz, s }),
            ..self
        }
    }

    /// The same, wavering `cents` either way `hz` times a second.
    pub const fn vibrato(self, hz: f32, cents: f32) -> Pitch {
        Pitch {
            vibrato: Some(Vibrato { hz, cents }),
            ..self
        }
    }
}

/// A slide in pitch, or of a filter's edge, by equal ratios: an octave
/// takes as long at any height.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Glide {
    pub to_hz: f32,
    pub s: f32,
}

/// A wavering of pitch.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Vibrato {
    pub hz: f32,
    /// How far either way, hundredths of a semitone.
    pub cents: f32,
}

/// How loud a layer is over its life: up in a straight line to `from`,
/// from there in a straight line to `to` while it holds, then down by
/// equal ratios to silence, over two milliseconds at the least. It ends at
/// zero, never cut.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Envelope {
    pub attack_s: f32,
    pub hold_s: f32,
    pub release_s: f32,
    /// Where the hold starts and where it ends, as fractions of the
    /// layer's gain: both 1 for a level that stays.
    pub from: f32,
    pub to: f32,
}

impl Envelope {
    /// One held at the layer's gain.
    pub const fn new(attack_s: f32, hold_s: f32, release_s: f32) -> Envelope {
        Envelope {
            attack_s,
            hold_s,
            release_s,
            from: 1.0,
            to: 1.0,
        }
    }

    /// The same, its hold moving from `from` to `to`: a swell, a fade.
    pub const fn sloping(self, from: f32, to: f32) -> Envelope {
        Envelope { from, to, ..self }
    }
}

/// What a layer lets through: all of it, or what lies below, above or
/// around `hz`. `q` is how sharp the edge is; past 0.7 it rings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Filter {
    Open,
    Low { hz: f32, q: f32 },
    High { hz: f32, q: f32 },
    Band { hz: f32, q: f32 },
}

/// How far a seed moves each layer, either way: its pitch in cents, its
/// gain and its times as fractions of themselves. The seed also seeds noise.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Vary {
    pub cents: f32,
    pub gain: f32,
    pub time: f32,
}

impl Vary {
    /// A voice a seed does not move, but for its noise.
    pub const NONE: Vary = Vary {
        cents: 0.0,
        gain: 0.0,
        time: 0.0,
    };
}
