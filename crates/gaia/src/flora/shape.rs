//! A plant's blocks, carved from the wood and leaves its rule grew, and its
//! crown, gathered from those leaves into the few round masses it is
//! painted as.

use core::ops::Range;

use super::kinds::{Look, Tissue};
use super::lsystem::{Growth, Segment};
use super::stream::Stream;

/// The farthest a block of a plant stands from its root block, across, in
/// blocks. Placement keeps plants this far apart, so a shape never reaches
/// past it.
pub(crate) const MAX_REACH: u32 = 8;
/// The highest a block of a plant stands over its root block, in blocks,
/// 40 m: what is built higher over the ground meets no plant.
pub const TALLEST: u32 = 80;
/// Layers of trunk under the ground, so a tree on a slope never stands on
/// air on its low side.
pub(crate) const ROOTS: i32 = 2;
/// The fewest and the most puffs a crown is painted as, and how many of the
/// leaves' gatherings make one, as many as a crown has.
const PUFFS: [usize; 2] = [3, 8];
const GATHERINGS_A_PUFF: usize = 4;
/// Rounds of gathering the leaves about their nearest puffs.
const ROUNDS: usize = 4;
/// How far a puff reaches against the farthest leaves it gathers, so a crown
/// is a few masses with hollows between them, never one ball.
const HUG: f64 = 0.9;

/// A round mass of leaves, in blocks: a crown is painted as a few of them,
/// each the leaves of the boughs about it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Puff {
    pub middle: [f64; 3],
    /// How far it reaches from its middle, across and up.
    pub radii: [f64; 2],
}

/// A plant's blocks, and the branches they were carved from, in blocks: the
/// root block spans `0..1` on each axis, `z` is up, and the ground runs
/// through layer `0`. Its blocks say how far it reaches, which keeps plants
/// apart, and its trunk, which alone stops a body; a client paints it from
/// its branches and its crown.
#[derive(Clone, PartialEq, Debug)]
pub struct Shape {
    /// The lowest block of the grid.
    low: [i32; 3],
    size: [u32; 3],
    /// `x` fastest, then `y`, then `z`.
    blocks: Vec<Option<Tissue>>,
    segments: Vec<Segment>,
    crown: Vec<Puff>,
    trunk: Vec<([i32; 2], Range<i32>)>,
    reach: u32,
    wood: Tissue,
    wood_shows: bool,
    leaves: Tissue,
}

impl Shape {
    /// Carves a growth: its leaves first, then its wood through them, the
    /// leaves' edges ragged by the stream and nothing past [`MAX_REACH`].
    pub(crate) fn carve(growth: &Growth, look: &Look, stream: &mut Stream) -> Shape {
        let key = stream.next();
        let mut grid = Grid::around(growth, look);
        // A block's draws are its own, whichever leaves reach it: it takes
        // if the nearest of them reaches it past its ragged edge. So each
        // block keeps its nearest reach, and is drawn for once.
        let mut nearest = vec![f64::INFINITY; grid.blocks.len()];
        for &(centre, radius) in &growth.leaves {
            let radii = [radius, radius, radius * look.squash];
            for p in grid.span(centre, radii) {
                let reach: f64 = (0..3)
                    .map(|i| {
                        let d = (f64::from(p[i]) + 0.5 - centre[i]) / radii[i];
                        d * d
                    })
                    .sum();
                let near = &mut nearest[grid.index(p)];
                *near = near.min(reach);
            }
        }
        for (i, &reach) in nearest.iter().enumerate() {
            // Past the leaves' rim no draw takes a block.
            if reach > 1.0 {
                continue;
            }
            let p = grid.point(i);
            let mut block = Stream::new(&[key, p[0] as u64, p[1] as u64, p[2] as u64]);
            if reach <= 1.0 - look.ragged * block.unit() {
                let tissue = look.leaves[usize::from(block.unit() < look.dapple)];
                grid.set(p, tissue);
            }
        }
        for segment in &growth.segments {
            // The blocks the branch passes through, so a thin one is never
            // broken, and those whose middle lies within its radius.
            let length = libm::sqrt(distance2(segment.from, segment.to));
            let steps = libm::ceil(length * 4.0).max(1.0) as u32;
            for k in 0..=steps {
                let t = f64::from(k) / f64::from(steps);
                let at = [0, 1, 2].map(|i| segment.from[i] + (segment.to[i] - segment.from[i]) * t);
                grid.set(at.map(|x| libm::floor(x) as i32), look.wood);
            }
            let radius = segment.radius;
            let low = [0, 1, 2].map(|i| segment.from[i].min(segment.to[i]) - radius);
            let high = [0, 1, 2].map(|i| segment.from[i].max(segment.to[i]) + radius);
            for p in grid.between(low, high) {
                let middle = p.map(|x| f64::from(x) + 0.5);
                if off_segment2(middle, segment) <= radius * radius {
                    grid.set(p, look.wood);
                }
            }
        }
        let mut shape = grid.into_shape(growth.segments.clone(), look.wood);
        shape.crown = crown(&growth.leaves, look.squash);
        shape.leaves = look.leaves[0];
        shape.wood_shows = look.sunk.is_none();
        // Wood hidden in leaves is undergrowth, walked through as leaves
        // are: what a viewer may thin out never stops a body.
        if !shape.wood_shows {
            shape.trunk.clear();
        }
        shape
    }

    /// The lowest block of the shape's box.
    pub fn low(&self) -> [i32; 3] {
        self.low
    }

    /// Blocks across, along and up its box.
    pub fn size(&self) -> [u32; 3] {
        self.size
    }

    /// What the block at a point is made of, or `None` for air.
    pub fn at(&self, p: [i32; 3]) -> Option<Tissue> {
        let local = [0, 1, 2].map(|i| p[i] - self.low[i]);
        let inside = (0..3).all(|i| 0 <= local[i] && (local[i] as u32) < self.size[i]);
        inside
            .then(|| self.blocks[index(self.size, local.map(|x| x as u32))])
            .flatten()
    }

    /// Every block of it, with what it is made of.
    pub fn blocks(&self) -> impl Iterator<Item = ([i32; 3], Tissue)> + '_ {
        let [sx, sy, _] = self.size;
        self.blocks
            .iter()
            .enumerate()
            .filter_map(move |(i, tissue)| {
                let i = i as u32;
                let local = [i % sx, i / sx % sy, i / (sx * sy)];
                let p = [0, 1, 2].map(|k| self.low[k] + local[k] as i32);
                tissue.map(|tissue| (p, tissue))
            })
    }

    /// The branches it was carved from.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Its crown, as the round masses of leaves it is painted as.
    pub fn crown(&self) -> &[Puff] {
        &self.crown
    }

    /// What its wood is made of, and its leaves where the sun reaches them.
    pub fn tissues(&self) -> [Tissue; 2] {
        [self.wood, self.leaves]
    }

    /// Whether its wood shows, or is hidden in its leaves.
    pub fn wood_shows(&self) -> bool {
        self.wood_shows
    }

    /// Its trunk: the wood the ground runs through. Each column whose block
    /// in layer `0` is wood, with the layers of the run of wood through it;
    /// none where the wood hides in the leaves.
    pub fn trunk(&self) -> &[([i32; 2], Range<i32>)] {
        &self.trunk
    }

    /// How far its farthest block stands from the root block, across, in
    /// blocks, rounded up.
    pub fn reach(&self) -> u32 {
        self.reach
    }
}

/// The blocks a shape is carved into while it grows: a box around
/// everything the growth reaches, cut to [`MAX_REACH`] across.
struct Grid {
    low: [i32; 3],
    size: [u32; 3],
    blocks: Vec<Option<Tissue>>,
}

impl Grid {
    fn around(growth: &Growth, look: &Look) -> Grid {
        let mut low = [f64::MAX; 3];
        let mut high = [f64::MIN; 3];
        let mut take = |p: [f64; 3], radii: [f64; 3]| {
            for i in 0..3 {
                low[i] = low[i].min(p[i] - radii[i]);
                high[i] = high[i].max(p[i] + radii[i]);
            }
        };
        for segment in &growth.segments {
            take(segment.from, [segment.radius; 3]);
            take(segment.to, [segment.radius; 3]);
        }
        for &(centre, radius) in &growth.leaves {
            take(centre, [radius, radius, radius * look.squash]);
        }
        let reach = MAX_REACH as i32;
        let low = [0, 1, 2].map(|i| {
            let floor = libm::floor(low[i]) as i32;
            if i < 2 { floor.max(-reach) } else { floor }
        });
        let high = [0, 1, 2].map(|i| {
            let ceil = libm::ceil(high[i]) as i32;
            if i < 2 { ceil.min(reach + 1) } else { ceil }
        });
        let size = [0, 1, 2].map(|i| (high[i] - low[i]).max(1) as u32);
        Grid {
            low,
            size,
            blocks: vec![None; (size[0] * size[1] * size[2]) as usize],
        }
    }

    /// Where a point of the box is kept in `blocks`.
    fn index(&self, p: [i32; 3]) -> usize {
        index(self.size, [0, 1, 2].map(|i| (p[i] - self.low[i]) as u32))
    }

    /// The point kept at an index of `blocks`.
    fn point(&self, i: usize) -> [i32; 3] {
        let [sx, sy, _] = self.size;
        let i = i as u32;
        let local = [i % sx, i / sx % sy, i / (sx * sy)];
        [0, 1, 2].map(|k| self.low[k] + local[k] as i32)
    }

    /// Sets a block, unless it is out of the box or out of reach.
    fn set(&mut self, p: [i32; 3], tissue: Tissue) {
        let reach = MAX_REACH as i32;
        if p[0] * p[0] + p[1] * p[1] > reach * reach {
            return;
        }
        let local = [0, 1, 2].map(|i| p[i] - self.low[i]);
        if (0..3).all(|i| 0 <= local[i] && (local[i] as u32) < self.size[i]) {
            self.blocks[index(self.size, local.map(|x| x as u32))] = Some(tissue);
        }
    }

    /// The blocks of the box a body with these radii about a centre touches.
    fn span(&self, centre: [f64; 3], radii: [f64; 3]) -> impl Iterator<Item = [i32; 3]> + use<> {
        let low = [0, 1, 2].map(|i| centre[i] - radii[i]);
        let high = [0, 1, 2].map(|i| centre[i] + radii[i]);
        self.between(low, high)
    }

    /// The blocks of the box between two points.
    fn between(&self, low: [f64; 3], high: [f64; 3]) -> impl Iterator<Item = [i32; 3]> + use<> {
        let from = [0, 1, 2].map(|i| (libm::floor(low[i]) as i32).max(self.low[i]));
        let to = [0, 1, 2]
            .map(|i| (libm::floor(high[i]) as i32).min(self.low[i] + self.size[i] as i32 - 1));
        (from[2]..=to[2]).flat_map(move |z| {
            (from[1]..=to[1]).flat_map(move |y| (from[0]..=to[0]).map(move |x| [x, y, z]))
        })
    }

    /// The shape: the box trimmed to the blocks there are, and its trunk
    /// found in them.
    fn into_shape(self, segments: Vec<Segment>, wood: Tissue) -> Shape {
        let mut low = [i32::MAX; 3];
        let mut high = [i32::MIN; 3];
        let mut reach2 = 0;
        let solid = Shape {
            low: self.low,
            size: self.size,
            blocks: self.blocks,
            segments: Vec::new(),
            crown: Vec::new(),
            trunk: Vec::new(),
            reach: 0,
            wood,
            wood_shows: true,
            leaves: wood,
        };
        for (p, _) in solid.blocks() {
            for i in 0..3 {
                low[i] = low[i].min(p[i]);
                high[i] = high[i].max(p[i]);
            }
            reach2 = reach2.max(p[0] * p[0] + p[1] * p[1]);
        }
        if low[0] > high[0] {
            return Shape {
                low: [0; 3],
                size: [0; 3],
                blocks: Vec::new(),
                segments,
                crown: Vec::new(),
                trunk: Vec::new(),
                reach: 0,
                wood,
                wood_shows: true,
                leaves: wood,
            };
        }
        let size = [0, 1, 2].map(|i| (high[i] - low[i] + 1) as u32);
        let mut blocks = vec![None; (size[0] * size[1] * size[2]) as usize];
        for (p, tissue) in solid.blocks() {
            let local = [0, 1, 2].map(|i| (p[i] - low[i]) as u32);
            blocks[index(size, local)] = Some(tissue);
        }
        let mut reach = 0;
        while reach * reach < reach2 {
            reach += 1;
        }
        let mut shape = Shape {
            low,
            size,
            blocks,
            segments,
            crown: Vec::new(),
            trunk: Vec::new(),
            reach: reach as u32,
            wood,
            wood_shows: true,
            leaves: wood,
        };
        shape.trunk = trunk(&shape, wood);
        shape
    }
}

/// The columns of a shape whose block in layer `0` is wood, each with the
/// run of wood through that block, `y` then `x`.
fn trunk(shape: &Shape, wood: Tissue) -> Vec<([i32; 2], Range<i32>)> {
    let [low, size] = [shape.low, shape.size.map(|n| n as i32)];
    let columns =
        (low[1]..low[1] + size[1]).flat_map(|y| (low[0]..low[0] + size[0]).map(move |x| [x, y]));
    columns
        .filter(|&[x, y]| shape.at([x, y, 0]) == Some(wood))
        .map(|[x, y]| {
            let wooden = |z: &i32| shape.at([x, y, *z]) == Some(wood);
            let bottom = (low[2]..=0).rev().take_while(wooden).last().unwrap_or(0);
            let top = (0..low[2] + size[2]).take_while(wooden).last().unwrap_or(0);
            ([x, y], bottom..top + 1)
        })
        .collect()
}

/// The leaves' gatherings, each a middle and a radius, gathered into the
/// puffs of a crown: as many as there are gatherings to share, spread from
/// the highest as far apart as they go, then moved a few rounds to the
/// middle of the gatherings nearest each. `squash` is how much flatter than
/// wide a gathering is.
pub(crate) fn crown(leaves: &[([f64; 3], f64)], squash: f64) -> Vec<Puff> {
    let Some(highest) = leaves.iter().max_by(|a, b| a.0[2].total_cmp(&b.0[2])) else {
        return Vec::new();
    };
    let count = (leaves.len() / GATHERINGS_A_PUFF)
        .clamp(PUFFS[0], PUFFS[1])
        .min(leaves.len());
    let nearest = |middles: &[[f64; 3]], p: [f64; 3]| {
        (0..middles.len())
            .map(|m| (m, distance2(middles[m], p)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("a crown has a middle")
    };
    let mut middles = vec![highest.0];
    while middles.len() < count {
        let farthest = leaves
            .iter()
            .max_by(|a, b| {
                nearest(&middles, a.0)
                    .1
                    .total_cmp(&nearest(&middles, b.0).1)
            })
            .expect("a crown has leaves");
        middles.push(farthest.0);
    }
    // Each gathering with the puff it is nearest.
    let gathered = |middles: &[[f64; 3]]| -> Vec<(usize, [f64; 3], f64)> {
        leaves
            .iter()
            .map(|&(p, r)| (nearest(middles, p).0, p, r))
            .collect()
    };
    for _ in 0..ROUNDS {
        let mut sums = vec![([0.0; 3], 0.0); middles.len()];
        for (m, p, r) in gathered(&middles) {
            let weight = r * r;
            for (sum, x) in sums[m].0.iter_mut().zip(p) {
                *sum += x * weight;
            }
            sums[m].1 += weight;
        }
        for (middle, (sum, weight)) in middles.iter_mut().zip(sums) {
            if weight > 0.0 {
                *middle = sum.map(|s| s / weight);
            }
        }
    }
    let mut radii = vec![[0.0f64; 2]; middles.len()];
    for (m, p, r) in gathered(&middles) {
        let [dx, dy, dz] = [0, 1, 2].map(|i| p[i] - middles[m][i]);
        let across = libm::sqrt(dx * dx + dy * dy) + r;
        let up = dz.abs() + r * squash;
        radii[m] = [radii[m][0].max(across), radii[m][1].max(up)];
    }
    middles
        .into_iter()
        .zip(radii)
        .filter(|(_, radii)| radii[0] > 0.0)
        .map(|(middle, radii)| Puff {
            middle,
            radii: radii.map(|r| r * HUG),
        })
        .collect()
}

fn index(size: [u32; 3], [x, y, z]: [u32; 3]) -> usize {
    ((z * size[1] + y) * size[0] + x) as usize
}

fn distance2(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (b[i] - a[i]) * (b[i] - a[i])).sum()
}

/// How far a point lies from a segment, squared.
fn off_segment2(p: [f64; 3], segment: &Segment) -> f64 {
    let (a, b) = (segment.from, segment.to);
    let ab = [0, 1, 2].map(|i| b[i] - a[i]);
    let ap = [0, 1, 2].map(|i| p[i] - a[i]);
    let length2: f64 = (0..3).map(|i| ab[i] * ab[i]).sum();
    let t = if length2 > 0.0 {
        ((0..3).map(|i| ap[i] * ab[i]).sum::<f64>() / length2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (0..3)
        .map(|i| {
            let d = ap[i] - ab[i] * t;
            d * d
        })
        .sum()
}
