//! A kind's branching as an L-system: an axiom rewritten a few times, then
//! read by a turtle that draws the wood and says where the leaves gather.
//!
//! The alphabet:
//! - `F`: grow one step forward, drawing wood;
//! - `L`: gather leaves where the turtle stands;
//! - `+` `-` turn, `&` `^` pitch, `/` `\` roll, each by its angle either way;
//! - `[` starts a branch, shorter and thinner than what it grows from, and
//!   `]` comes back from it;
//! - any other letter is a bud: rewritten by the rule, never drawn.
//!
//! Every step and angle wanders a little, drawn from the variant's stream, so
//! one rule grows a family of plants and never the same one twice.

use super::stream::Stream;

/// What one letter may become at each rewriting, each with its weight.
#[derive(Debug)]
pub(crate) struct Production {
    pub letter: u8,
    pub into: &'static [(&'static str, u32)],
}

/// How a kind branches, and how big it grows.
#[derive(Debug)]
pub(crate) struct Rule {
    pub axiom: &'static str,
    pub productions: &'static [Production],
    /// How many times the axiom is rewritten.
    pub steps: u32,
    /// Blocks an `F` of the trunk grows.
    pub step: f64,
    /// The trunk's radius, blocks.
    pub girth: f64,
    /// How much shorter, and how much thinner, a branch is than what it
    /// grows from.
    pub shorten: f64,
    pub thin: f64,
    /// Degrees of a pitch, a turn and a roll.
    pub pitch: f64,
    pub turn: f64,
    pub roll: f64,
    /// How far each step and angle wanders, a share of itself.
    pub wander: f64,
    /// How much a growing branch bends toward the sky each step, or toward
    /// the ground when below zero.
    pub rise: f64,
    /// The radius of a gathering of leaves, blocks.
    pub crown: f64,
}

/// A stretch of wood, in blocks: the plant's root block spans `0..1` on
/// each axis, and the ground runs through its bottom layer.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Segment {
    pub from: [f64; 3],
    pub to: [f64; 3],
    /// Its radius, blocks.
    pub radius: f64,
}

/// What a rule grows: its wood, and where its leaves gather and how wide.
pub(crate) struct Growth {
    pub segments: Vec<Segment>,
    pub leaves: Vec<([f64; 3], f64)>,
}

impl Growth {
    /// Moves all of it `depth` blocks down.
    pub(crate) fn lower(&mut self, depth: f64) {
        let down = |p: &mut [f64; 3]| p[2] -= depth;
        for segment in &mut self.segments {
            down(&mut segment.from);
            down(&mut segment.to);
        }
        for (middle, _) in &mut self.leaves {
            down(middle);
        }
    }
}

/// How a plant thickens as it grows bigger: a little slower than it
/// lengthens, or an elder's trunk would be a tower.
const GIRTH_GROWS: f64 = 0.9;

/// Grows a rule from a point, upright, `size` times as big as the rule
/// says, deciding by the stream.
pub(crate) fn grow(rule: &Rule, start: [f64; 3], size: f64, stream: &mut Stream) -> Growth {
    let word = rewrite(rule, stream);
    let mut turtle = Turtle {
        at: start,
        heading: [0.0, 0.0, 1.0],
        left: [0.0, 1.0, 0.0],
        up: [-1.0, 0.0, 0.0],
        length: rule.step * size,
        radius: rule.girth * libm::pow(size, GIRTH_GROWS),
    };
    let mut stack = Vec::new();
    let mut growth = Growth {
        segments: Vec::new(),
        leaves: Vec::new(),
    };
    let angle = |degrees: f64, stream: &mut Stream| {
        degrees * (1.0 + rule.wander * stream.wander()) * (core::f64::consts::PI / 180.0)
    };
    for letter in word {
        match letter {
            b'F' => {
                let length = turtle.length * (1.0 + rule.wander * stream.wander());
                let to = add(turtle.at, scale(turtle.heading, length));
                growth.segments.push(Segment {
                    from: turtle.at,
                    to,
                    radius: turtle.radius,
                });
                turtle.at = to;
                turtle.bend(rule.rise);
            }
            b'L' => growth.leaves.push((
                turtle.at,
                rule.crown * size * (1.0 + rule.wander * stream.wander()),
            )),
            b'+' => turtle.turn(angle(rule.turn, stream)),
            b'-' => turtle.turn(-angle(rule.turn, stream)),
            b'&' => turtle.pitch(angle(rule.pitch, stream)),
            b'^' => turtle.pitch(-angle(rule.pitch, stream)),
            b'/' => turtle.roll(angle(rule.roll, stream)),
            b'\\' => turtle.roll(-angle(rule.roll, stream)),
            b'[' => {
                stack.push(turtle);
                turtle.length *= rule.shorten;
                turtle.radius *= rule.thin;
            }
            b']' => {
                turtle = stack
                    .pop()
                    .expect("a rule closes only the branches it opens")
            }
            _ => {}
        }
    }
    growth
}

/// The axiom rewritten `steps` times, every letter at once.
fn rewrite(rule: &Rule, stream: &mut Stream) -> Vec<u8> {
    let mut word = rule.axiom.as_bytes().to_vec();
    for _ in 0..rule.steps {
        let mut next = Vec::with_capacity(word.len() * 4);
        for &letter in &word {
            match rule.productions.iter().find(|p| p.letter == letter) {
                Some(production) => next.extend_from_slice(pick(production.into, stream)),
                None => next.push(letter),
            }
        }
        word = next;
    }
    word
}

/// One of the ways a letter may grow, by weight.
fn pick(into: &'static [(&'static str, u32)], stream: &mut Stream) -> &'static [u8] {
    let total = into.iter().map(|(_, weight)| weight).sum();
    let mut roll = stream.below(total);
    for (word, weight) in into {
        if roll < *weight {
            return word.as_bytes();
        }
        roll -= weight;
    }
    unreachable!("a roll below the total lands on some weight")
}

/// Where the drawing stands and which way it faces: `heading` forward,
/// `left` and `up` square to it.
#[derive(Clone, Copy)]
struct Turtle {
    at: [f64; 3],
    heading: [f64; 3],
    left: [f64; 3],
    up: [f64; 3],
    length: f64,
    radius: f64,
}

impl Turtle {
    fn turn(&mut self, radians: f64) {
        (self.heading, self.left) = rotate(self.heading, self.left, radians);
    }

    fn pitch(&mut self, radians: f64) {
        (self.heading, self.up) = rotate(self.heading, self.up, radians);
    }

    fn roll(&mut self, radians: f64) {
        (self.left, self.up) = rotate(self.left, self.up, radians);
    }

    /// Bends the heading toward the sky by `rise`, and squares the frame
    /// back to it. A branch heading straight down keeps going.
    fn bend(&mut self, rise: f64) {
        let bent = add(self.heading, [0.0, 0.0, rise]);
        let length = libm::sqrt(dot(bent, bent));
        if length < 1e-9 {
            return;
        }
        self.heading = scale(bent, 1.0 / length);
        let left = add(
            self.left,
            scale(self.heading, -dot(self.left, self.heading)),
        );
        self.left = scale(left, 1.0 / libm::sqrt(dot(left, left)));
        self.up = cross(self.heading, self.left);
    }
}

/// Turns `a` toward `b` by an angle, in the plane of the two.
fn rotate(a: [f64; 3], b: [f64; 3], radians: f64) -> ([f64; 3], [f64; 3]) {
    let (sin, cos) = (libm::sin(radians), libm::cos(radians));
    (
        add(scale(a, cos), scale(b, sin)),
        add(scale(b, cos), scale(a, -sin)),
    )
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
