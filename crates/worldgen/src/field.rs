//! The field: ground shape baked into a cube map, one face per sector.
//!
//! A world whose recipe names a field is shaped by it above the texel and by
//! the generator's own noise below, the same way [`crate::noise::band`] fades
//! an octave finer than the mesh that asks. The field is the low frequencies
//! of some real body; the seed still owns everything smaller than a texel, so
//! one field is a family of worlds, not a world.
//!
//! Two channels per texel, because a level that only averaged would flatten
//! the mountains it covers:
//! - `elevation`, metres over the source body's sea level, as `i16`;
//! - `ruggedness`, how far elevation ranges inside one texel of the grain the
//!   bake measures it at (`scripts/field.ts`), in steps of `RUGGED_STEP`
//!   metres. What a level loses becomes roughness, as the sea's waves do,
//!   and it is what tells a cordillera from a plateau of the same height.
//!   Coarse levels average it rather than accumulate it, so it stays the
//!   same measure of the same ground at every level: a silhouette must not
//!   change because the camera moved away.
//!
//! A file holds the finest level alone ([`Field::parse`]). The parser builds
//! each coarser level from the one before by `mean`, down to a face of
//! `COARSEST` texels, and gives every face a one texel gutter filled from the
//! neighbouring face. The texel past a sector edge is the neighbour's first
//! texel, folded across the seam in address space the way a step across the
//! edge is ([`Grid::wrapped`]), and where three faces meet the corner texel is
//! the mean of their three corner texels. A sample at a seam then reads the
//! same ground from either side with no seam logic, and no branch, per sample.
//!
//! Nothing here reads a file: the bytes arrive from the shell, already whole.
//! The pyramid is integer arithmetic and a sample is integer fetches and
//! `+ - * /`, so every machine builds the same ground from the same bytes and
//! a field world is as deterministic as a generated one.

use core::iter;

use topology::{BLOCK_M, Grid, QuadSphere, Sector, SurfacePoint, Vec3};

/// Fields start with this, so a wrong file is refused instead of decoded.
const MAGIC: [u8; 8] = *b"PLFIELD2";
/// Magic, id, side.
const HEADER: usize = 8 + 32 + 4;
/// Texels per face side of the coarsest level, where the pyramid stops.
const COARSEST: u32 = 16;
/// Texels per face side of the finest level a file may hold: 75.5 MB of file
/// and 101 MB of pyramid at this side, which a phone's tab and a Pi hold
/// together. Twice the side is four times both, 0.7 GB of WASM memory that a
/// phone's tab cannot count on, so a larger file is refused before anything
/// is allocated.
const FINEST: u32 = 2048;
/// Metres of spread in one step of the ruggedness byte. Nearly all ground
/// ranges under a few hundred metres inside a texel, so the step is small and
/// the top of the byte is the Himalaya, not headroom nobody uses.
const RUGGED_STEP: f64 = 16.0;

/// Both channels of one texel: metres of elevation, steps of ruggedness.
type Texel = (i16, u8);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldError {
    /// The bytes do not start with the magic.
    NotAField,
    /// Faces are square, a power of two from `COARSEST` to `FINEST` texels.
    UnsupportedSide(u32),
    /// The bytes are not as many as a file of its side.
    WrongLength { want: usize, got: usize },
}

impl core::fmt::Display for FieldError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FieldError::NotAField => write!(f, "not a field: wrong magic"),
            FieldError::UnsupportedSide(side) => write!(
                f,
                "unsupported face side {side}: want a power of two in {COARSEST}..={FINEST}"
            ),
            FieldError::WrongLength { want, got } => {
                write!(f, "a field of this side is {want} bytes, not {got}")
            }
        }
    }
}

impl std::error::Error for FieldError {}

/// The ground one texel of the field describes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ground {
    /// Metres over the source body's sea level. The zero is its coastline.
    pub elevation_m: f64,
    /// How far elevation ranges inside what this sample covers, metres.
    pub ruggedness_m: f64,
}

/// A baked cube map of ground, ready to sample.
#[derive(Clone, Debug)]
pub struct Field {
    id: [u8; 32],
    /// Finest first, each half the side of the one before.
    levels: Vec<Level>,
}

impl Field {
    /// Reads a field file and builds its pyramid. The id is read, never
    /// recomputed: bytes that crossed a network were checked where they
    /// arrived, since the page hashes them against the id the recipe names
    /// before the engine sees them (`apps/web/src/lib/fields.ts`); a file the
    /// desktop reads from its own disk is trusted to be the ground its id
    /// says.
    ///
    /// The file, as `scripts/field.ts` writes it: `PLFIELD2`, the id, the face
    /// side as a little endian `u32`, then the finest level in three planes,
    /// each one face after another and row after row, with no gutter:
    /// 1. the high byte of each elevation's residual;
    /// 2. its low byte;
    /// 3. ruggedness, as it is.
    ///
    /// A residual is an elevation less its prediction from the texels before
    /// it (`unpredict` says which), zigzagged into a `u16`. Over most ground
    /// the residuals are small, so the high plane is nearly all zeros and the
    /// low one repeats itself, which is what brotli packs on the way.
    pub fn parse(bytes: &[u8]) -> Result<Field, FieldError> {
        let header = bytes
            .get(..HEADER)
            .filter(|header| header[..8] == MAGIC)
            .ok_or(FieldError::NotAField)?;
        let id = header[8..40].try_into().expect("an id is 32 bytes");
        let side = u32::from_le_bytes(header[40..].try_into().expect("a side is 4 bytes"));
        if !(COARSEST..=FINEST).contains(&side) || !side.is_power_of_two() {
            return Err(FieldError::UnsupportedSide(side));
        }
        let plane = Sector::COUNT * (side as usize).pow(2);
        let want = HEADER + 3 * plane;
        if bytes.len() != want {
            return Err(FieldError::WrongLength {
                want,
                got: bytes.len(),
            });
        }
        let (high, rest) = bytes[HEADER..].split_at(plane);
        let (low, ruggedness) = rest.split_at(plane);
        let finest = Level::read(side, high, low, ruggedness);
        let levels = iter::successors(Some(finest), |finer| {
            (finer.side > COARSEST).then(|| finer.halved())
        })
        .map(Level::fold_seams)
        .collect();
        Ok(Field { id, levels })
    }

    /// What the recipe names this field by.
    pub fn id(&self) -> [u8; 32] {
        self.id
    }

    /// Metres of ground one texel of the finest level covers, on the body
    /// the field is stretched over. A field is a shape, so the same bytes
    /// cover a small world at a proportionally finer grain.
    pub fn texel_m(&self, sphere: QuadSphere) -> f64 {
        f64::from(sphere.blocks().side()) * BLOCK_M / f64::from(self.levels[0].side)
    }

    /// The ground under a direction, as a mesh sampled every `footprint_m`
    /// metres should see it: levels finer than the mesh are blended out, the
    /// way `band` fades an octave, so no level change ever pops.
    pub fn sample(&self, sphere: QuadSphere, d: Vec3, footprint_m: f64) -> Ground {
        // Levels double in size, so the level that matches a footprint is its
        // log. Below the finest level there is nothing coarser to fall back
        // on: the generator's noise takes over there.
        let wanted = libm::log2((footprint_m / self.texel_m(sphere)).max(1.0));
        let last = (self.levels.len() - 1) as f64;
        let coarse = libm::floor(wanted).min(last);
        let blend = (wanted - coarse).clamp(0.0, 1.0);
        let near = self.level(sphere, coarse as usize, d);
        if blend <= 0.0 {
            return near;
        }
        let far = self.level(sphere, coarse as usize + 1, d);
        Ground {
            elevation_m: near.elevation_m + (far.elevation_m - near.elevation_m) * blend,
            ruggedness_m: near.ruggedness_m + (far.ruggedness_m - near.ruggedness_m) * blend,
        }
    }

    /// Bilinear fetch inside one level. The gutter makes the face edge an
    /// ordinary texel, so nothing here knows what a seam is.
    fn level(&self, sphere: QuadSphere, index: usize, d: Vec3) -> Ground {
        let level = &self.levels[index.min(self.levels.len() - 1)];
        let point = sphere.blocks().surface_point(d);
        let span = f64::from(level.side) / f64::from(sphere.blocks().side());
        // Texel `t` has its centre at `t + 0.5` across the face and lives at
        // `t + 1` in the array: adding 0.5 puts a texel centre on an integer.
        let x = point.u * span + 0.5;
        let y = point.v * span + 0.5;
        let last = f64::from(level.side + 1);
        let x0 = libm::floor(x).clamp(0.0, last - 1.0);
        let y0 = libm::floor(y).clamp(0.0, last - 1.0);
        let (fx, fy) = (x - x0, y - y0);
        let corner = level.index(point.sector, x0 as usize, y0 as usize);
        let stride = level.stride();
        let at = |index: usize| {
            let (e, r) = level.texel(index);
            (f64::from(e), f64::from(r) * RUGGED_STEP)
        };
        let (e00, r00) = at(corner);
        let (e10, r10) = at(corner + 1);
        let (e01, r01) = at(corner + stride);
        let (e11, r11) = at(corner + stride + 1);
        let mix = |a: f64, b: f64, c: f64, e: f64| {
            let top = a + (b - a) * fx;
            let bottom = c + (e - c) * fx;
            top + (bottom - top) * fy
        };
        Ground {
            elevation_m: mix(e00, e10, e01, e11),
            ruggedness_m: mix(r00, r10, r01, r11),
        }
    }
}

/// One level of the pyramid: six faces of `stride` texels a side, row after
/// row, the outer ring of each its gutter. A channel is a plane of its own,
/// so a texel weighs three bytes.
#[derive(Clone, Debug)]
struct Level {
    /// Texels per face side, gutter not counted.
    side: u32,
    elevation: Vec<i16>,
    ruggedness: Vec<u8>,
}

impl Level {
    /// Six faces of ground at sea level, gutters included.
    fn zeroed(side: u32) -> Level {
        let texels = Sector::COUNT * (side as usize + 2).pow(2);
        Level {
            side,
            elevation: vec![0; texels],
            ruggedness: vec![0; texels],
        }
    }

    /// The finest level, from the three planes of a file ([`Field::parse`]).
    fn read(side: u32, high: &[u8], low: &[u8], ruggedness: &[u8]) -> Level {
        let n = side as usize;
        let mut level = Level::zeroed(side);
        let residuals = high.chunks_exact(n).zip(low.chunks_exact(n));
        for (row, (high, low)) in interior(&mut level.elevation, n).zip(residuals) {
            for ((texel, &high), &low) in row.iter_mut().zip(high).zip(low) {
                *texel = unzigzag(u16::from_be_bytes([high, low]));
            }
        }
        let face = level.stride().pow(2);
        for residuals in level.elevation.chunks_exact_mut(face) {
            unpredict(interior(residuals, n));
        }
        for (row, stored) in interior(&mut level.ruggedness, n).zip(ruggedness.chunks_exact(n)) {
            row.copy_from_slice(stored);
        }
        level
    }

    /// The next coarser level: each texel the `mean` of the four it covers,
    /// the gutter left for [`Level::fold_seams`].
    fn halved(&self) -> Level {
        let mut coarse = Level::zeroed(self.side / 2);
        let stride = self.stride();
        for sector in Sector::ALL {
            for y in 1..=coarse.side as usize {
                for x in 1..=coarse.side as usize {
                    // Texel `t` lives at `t + 1`: coarse texel `x - 1` covers
                    // fine texels `2x - 2` and `2x - 1`, which live at `2x - 1`
                    // and `2x`.
                    let first = self.index(sector, 2 * x - 1, 2 * y - 1);
                    let four = [first, first + 1, first + stride, first + stride + 1];
                    let index = coarse.index(sector, x, y);
                    coarse.set(index, mean(four.map(|at| self.texel(at))));
                }
            }
        }
        coarse
    }

    /// Fills every gutter texel from the face across its seam. A gutter
    /// texel's centre lies half a texel past the edge; folded in address
    /// space that is the centre of the neighbour's first texel, so the pair
    /// each face interpolates along an edge is the same pair. Projecting
    /// the gutter's direction onto the neighbour instead would shift the
    /// coordinate along the edge, and each face would read its own boundary.
    /// Where three faces meet there is no fourth texel: the corner takes the
    /// mean of the three, the same number from whichever face reads it.
    fn fold_seams(mut self) -> Level {
        let grid = Grid::new(self.side.trailing_zeros()).expect("a face side is a grid");
        let side = f64::from(self.side);
        let last = self.side as usize + 1;
        for sector in Sector::ALL {
            for (x, y) in gutter(self.side) {
                // The texel centred at `(u, v)` of this sector, in texel
                // units, once folded into range.
                let across = |u: f64, v: f64| {
                    let column = grid.column_of(grid.wrapped(SurfacePoint::new(sector, u, v)));
                    let (u, v) = (usize::from(column.u), usize::from(column.v));
                    self.texel(self.index(column.sector, u + 1, v + 1))
                };
                let (u, v) = (x as f64 - 0.5, y as f64 - 0.5);
                let texel = if (x == 0 || x == last) && (y == 0 || y == last) {
                    let (cu, cv) = (u.clamp(0.5, side - 0.5), v.clamp(0.5, side - 0.5));
                    mean([(cu, cv), (u, cv), (cu, v)].map(|(u, v)| across(u, v)))
                } else {
                    across(u, v)
                };
                let index = self.index(sector, x, y);
                self.set(index, texel);
            }
        }
        self
    }

    /// Texels per face side with the gutter: the row stride of a face.
    fn stride(&self) -> usize {
        self.side as usize + 2
    }

    /// Where texel `(x, y)` of a sector's face lives. These are array
    /// coordinates: the gutter is row and column 0 and `side + 1`.
    fn index(&self, sector: Sector, x: usize, y: usize) -> usize {
        (sector.index() * self.stride() + y) * self.stride() + x
    }

    fn texel(&self, index: usize) -> Texel {
        (self.elevation[index], self.ruggedness[index])
    }

    fn set(&mut self, index: usize, (elevation, ruggedness): Texel) {
        self.elevation[index] = elevation;
        self.ruggedness[index] = ruggedness;
    }
}

/// The nearest mean of `N` texels, channel by channel, a half rounded up: a
/// coarser level's texel is the mean of the four it covers, and a gutter
/// corner the mean of its three faces. Integer, so it is the same number on
/// every machine.
fn mean<const N: usize>(texels: [Texel; N]) -> Texel {
    let n = N as i32;
    let (elevation, ruggedness) = texels.iter().fold((0, 0), |(e, r), &(te, tr)| {
        (e + i32::from(te), r + i32::from(tr))
    });
    (
        (elevation + n / 2).div_euclid(n) as i16,
        (ruggedness + n / 2).div_euclid(n) as u8,
    )
}

/// Turns one face's residuals back into elevations, in place, given its rows
/// top to bottom. The bake predicted each texel from the ones before it: the
/// texel to its left along the first row (zero before the first), the texel
/// above down the first column, and [`med`] everywhere else. So each row
/// needs only the row above, already undone.
fn unpredict<'a>(mut rows: impl Iterator<Item = &'a mut [i16]>) {
    let Some(mut above) = rows.next() else {
        return;
    };
    let mut left = 0i16;
    for texel in above.iter_mut() {
        left = left.wrapping_add(*texel);
        *texel = left;
    }
    for row in rows {
        row[0] = row[0].wrapping_add(above[0]);
        let mut left = row[0];
        for (texel, up) in row[1..].iter_mut().zip(above.windows(2)) {
            left = med(left, up[1], up[0]).wrapping_add(*texel);
            *texel = left;
        }
        above = row;
    }
}

/// LOCO-I's median edge detector, the predictor of JPEG-LS: the median of the
/// texel to the left, the texel above, and the plane through those two and
/// the one above-left. Across a coast or a ridge it follows the side the edge
/// runs along, where a plane alone would smear the step into both.
fn med(left: i16, up: i16, up_left: i16) -> i16 {
    let (a, b, c) = (i32::from(left), i32::from(up), i32::from(up_left));
    (a + b - c).clamp(a.min(b), a.max(b)) as i16
}

/// A residual from its zigzag: 0, 1, 2, 3 are 0, -1, 1, -2, so a small
/// residual of either sign is a small number.
fn unzigzag(zigzag: u16) -> i16 {
    (zigzag >> 1) as i16 ^ -((zigzag & 1) as i16)
}

/// The rows of a plane inside its gutters, face after face and top to
/// bottom: the order a file stores them in.
fn interior<T>(plane: &mut [T], side: usize) -> impl Iterator<Item = &mut [T]> {
    let stride = side + 2;
    plane
        .chunks_exact_mut(stride * stride)
        .flat_map(move |face| face.chunks_exact_mut(stride).skip(1).take(side))
        .map(move |row| &mut row[1..=side])
}

/// The array coordinates of the gutter around a face of `side` texels: its
/// outermost ring.
fn gutter(side: u32) -> impl Iterator<Item = (usize, usize)> {
    let last = side as usize + 1;
    (0..=last)
        .flat_map(move |x| [(x, 0), (x, last)])
        .chain((1..last).flat_map(move |y| [(0, y), (last, y)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a gutter holds before the fold fills it. No test lays this value
    /// inside a face, so a texel still holding it was never filled.
    const EMPTY: Texel = (-10_000, 255);

    /// A file of the finest level, `texel` giving each sector's texel
    /// `(x, y)`, `0..side`. Written the way the bake writes one, texel by
    /// texel, and with nothing of the parser's, so a parse is checked against
    /// the format and not against itself.
    fn file(side: usize, texel: impl Fn(usize, usize, usize) -> Texel) -> Vec<u8> {
        let mut planes = [Vec::new(), Vec::new(), Vec::new()];
        for sector in 0..Sector::COUNT {
            let elevation = |x: usize, y: usize| texel(sector, x, y).0;
            for y in 0..side {
                for x in 0..side {
                    let predicted = match (x, y) {
                        (0, 0) => 0,
                        (_, 0) => elevation(x - 1, 0),
                        (0, _) => elevation(0, y - 1),
                        // The median edge detector as JPEG-LS states it:
                        // the lesser of left and up under an up-left above
                        // both, the greater over one below both, the plane
                        // between.
                        _ => {
                            let (left, up) = (elevation(x - 1, y), elevation(x, y - 1));
                            let up_left = elevation(x - 1, y - 1);
                            if up_left >= left.max(up) {
                                left.min(up)
                            } else if up_left <= left.min(up) {
                                left.max(up)
                            } else {
                                (i32::from(left) + i32::from(up) - i32::from(up_left)) as i16
                            }
                        }
                    };
                    let residual = elevation(x, y).wrapping_sub(predicted);
                    let [high, low] = (((residual << 1) ^ (residual >> 15)) as u16).to_be_bytes();
                    planes[0].push(high);
                    planes[1].push(low);
                    planes[2].push(texel(sector, x, y).1);
                }
            }
        }
        let mut bytes = Vec::from(MAGIC);
        bytes.extend([0; 32]);
        bytes.extend((side as u32).to_le_bytes());
        for plane in planes {
            bytes.extend(plane);
        }
        bytes
    }

    /// Slopes, cliffs and noise over the whole range of both channels, so
    /// every prediction branch is taken, edges are crossed and residuals wrap.
    fn rough(sector: usize, x: usize, y: usize) -> Texel {
        let mut hash = ((sector * 4096 + y * 64 + x) as u32).wrapping_mul(0x9e37_79b9);
        hash ^= hash >> 15;
        hash = hash.wrapping_mul(0x85eb_ca6b);
        hash ^= hash >> 13;
        match sector {
            0 | 1 => ((x * 37 + y * 11) as i16 - 1500, (x + 2 * y) as u8),
            2 | 3 => (
                if (x / 7 + y / 5) % 2 == 0 {
                    8848
                } else {
                    -10_994
                },
                (x * y) as u8,
            ),
            _ => (hash as i16, (hash >> 16) as u8),
        }
    }

    /// A field laid by hand, level by level: `texel` gives each level's
    /// texel at a sector's array coordinates, `1..=side` inside the gutter.
    /// Its levels need not follow `mean`, so a sample can tell which level it
    /// read. The gutters start `EMPTY` and are folded.
    fn layered(side: u32, levels: u32, texel: impl Fn(u32, usize, usize, usize) -> Texel) -> Field {
        let levels = (0..levels)
            .map(|level| {
                let mut laid = Level::zeroed(side >> level);
                let last = laid.stride() - 1;
                for sector in Sector::ALL {
                    for y in 0..=last {
                        for x in 0..=last {
                            let inside = (1..last).contains(&x) && (1..last).contains(&y);
                            let value = if inside {
                                texel(level, sector.index(), x, y)
                            } else {
                                EMPTY
                            };
                            let index = laid.index(sector, x, y);
                            laid.set(index, value);
                        }
                    }
                }
                laid.fold_seams()
            })
            .collect();
        Field {
            id: [0; 32],
            levels,
        }
    }

    /// A field whose every texel of level `l` holds `elevation(l)`, so a
    /// sample says which level it read and whether it read it in the right
    /// place.
    fn constant(side: u32, levels: u32, elevation: impl Fn(u32) -> i16) -> Field {
        layered(side, levels, |level, _, _, _| {
            (elevation(level), level as u8)
        })
    }

    /// Unequal faces and slopes, so a seam read from the wrong texel, an edge
    /// folded the wrong way round or a corner taken from one face alone all
    /// show. Constant faces cannot tell a crack from a plain.
    fn sloped() -> Field {
        layered(64, 3, |level, sector, x, y| {
            (
                (sector * 1000 + x * 3 + y * 7 + level as usize * 200) as i16,
                (sector * 20 + x + y) as u8,
            )
        })
    }

    /// Directions into every sector, none of them on a seam.
    fn directions() -> Vec<Vec3> {
        let mut out = Vec::new();
        for i in -3i32..=3 {
            for j in -3i32..=3 {
                for k in -3i32..=3 {
                    if (i, j, k) != (0, 0, 0) {
                        let v = [f64::from(i), f64::from(j), f64::from(k)];
                        let len = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
                        out.push([v[0] / len, v[1] / len, v[2] / len]);
                    }
                }
            }
        }
        out
    }

    /// The body a field is stretched over in these tests: today's planet.
    fn planet() -> QuadSphere {
        QuadSphere::new(topology::MAX_BITS).expect("a legal size")
    }

    #[test]
    fn a_wrong_file_is_refused_instead_of_decoded() {
        let good = file(16, |_, _, _| (0, 0));
        assert!(Field::parse(&good).is_ok());
        let with = |at: usize, patch: &[u8]| {
            let mut bytes = good.clone();
            bytes[at..at + patch.len()].copy_from_slice(patch);
            bytes
        };
        let refused = |bytes: &[u8]| Field::parse(bytes).unwrap_err();
        assert_eq!(refused(&[0; 64]), FieldError::NotAField);
        assert_eq!(refused(&good[..HEADER - 1]), FieldError::NotAField);
        // Another version of the format is another file.
        assert_eq!(refused(&with(0, b"PLFIELD1")), FieldError::NotAField);
        for side in [COARSEST / 2, 24, FINEST * 2] {
            assert_eq!(
                refused(&with(40, &side.to_le_bytes())),
                FieldError::UnsupportedSide(side)
            );
        }
        // The finest side a file may hold is a side: what it lacks is bytes.
        assert!(matches!(
            refused(&with(40, &FINEST.to_le_bytes())),
            FieldError::WrongLength { .. }
        ));
        assert_eq!(
            refused(&with(40, &32u32.to_le_bytes())),
            FieldError::WrongLength {
                want: HEADER + 3 * Sector::COUNT * 32 * 32,
                got: good.len()
            }
        );
        assert_eq!(
            refused(&good[..good.len() - 1]),
            FieldError::WrongLength {
                want: good.len(),
                got: good.len() - 1
            }
        );
    }

    /// Bytes to pyramid: the finest level comes back as baked, to the bit,
    /// each coarser texel is the mean of the four under it, a half rounded
    /// up, down to a face of `COARSEST` texels, and every gutter is laid.
    #[test]
    fn a_file_becomes_the_pyramid_its_rule_defines() {
        let field = Field::parse(&file(64, rough)).expect("a file this test just wrote");
        let sides: Vec<u32> = field.levels.iter().map(|level| level.side).collect();
        assert_eq!(sides, [64, 32, COARSEST]);

        let finest = &field.levels[0];
        for sector in Sector::ALL {
            for y in 0..64 {
                for x in 0..64 {
                    assert_eq!(
                        finest.texel(finest.index(sector, x + 1, y + 1)),
                        rough(sector.index(), x, y),
                        "{sector:?} ({x}, {y})"
                    );
                }
            }
        }

        for pair in field.levels.windows(2) {
            let (fine, coarse) = (&pair[0], &pair[1]);
            for sector in Sector::ALL {
                for y in 1..=coarse.side as usize {
                    for x in 1..=coarse.side as usize {
                        let four = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| {
                            fine.texel(fine.index(sector, 2 * x - 1 + dx, 2 * y - 1 + dy))
                        });
                        let sum = |channel: fn(Texel) -> i32| four.map(channel).iter().sum::<i32>();
                        let want = (
                            ((sum(|t| t.0.into()) + 2) >> 2) as i16,
                            ((sum(|t| t.1.into()) + 2) >> 2) as u8,
                        );
                        assert_eq!(
                            coarse.texel(coarse.index(sector, x, y)),
                            want,
                            "side {} {sector:?} ({x}, {y})",
                            coarse.side
                        );
                    }
                }
            }
        }

        // Folding again changes nothing: the parser left no gutter unfolded.
        for level in &field.levels {
            let refolded = level.clone().fold_seams();
            assert_eq!(refolded.elevation, level.elevation, "side {}", level.side);
            assert_eq!(refolded.ruggedness, level.ruggedness, "side {}", level.side);
        }
    }

    #[test]
    fn every_direction_reads_the_level_its_footprint_asks_for() {
        let field = constant(64, 4, |level| 100 * (level as i16 + 1));
        let sphere = planet();
        let texel_m = field.texel_m(sphere);
        for (footprint_m, want) in [
            (0.0, 100.0),
            (texel_m, 100.0),
            (texel_m * 2.0, 200.0),
            (texel_m * 8.0, 400.0),
            // Past the coarsest level there is nothing coarser to read.
            (texel_m * 1024.0, 400.0),
        ] {
            for d in directions() {
                let ground = field.sample(sphere, d, footprint_m);
                assert!(
                    libm::fabs(ground.elevation_m - want) < 1e-9,
                    "{d:?} at {footprint_m} m: want {want}, got {}",
                    ground.elevation_m
                );
            }
        }
    }

    #[test]
    fn a_footprint_between_levels_blends_them() {
        let field = constant(64, 3, |level| 100 * (level as i16 + 1));
        let sphere = planet();
        let ground = field.sample(sphere, [1.0, 0.0, 0.0], field.texel_m(sphere) * 3.0);
        // log2(3) is 1.585: most of the way from level 1 to level 2.
        assert!(
            (258.0..259.0).contains(&ground.elevation_m),
            "{}",
            ground.elevation_m
        );
    }

    #[test]
    fn ruggedness_comes_back_in_metres() {
        let field = constant(32, 2, |_| 0);
        let sphere = planet();
        let ground = field.sample(sphere, [0.0, 1.0, 0.0], 0.0);
        assert_eq!(ground.ruggedness_m, 0.0);
        let coarse = field.sample(sphere, [0.0, 1.0, 0.0], field.texel_m(sphere) * 2.0);
        assert_eq!(coarse.ruggedness_m, RUGGED_STEP);
    }

    #[test]
    fn a_seam_is_an_ordinary_texel() {
        // Walking across a sector edge must not jump: the gutter carries the
        // ground from the other side, so the two faces agree there.
        let field = constant(64, 2, |_| 500);
        let step = 1e-4;
        let mut previous = None;
        for i in -20..=20 {
            // A path that crosses the +X / +Z seam.
            let angle = core::f64::consts::FRAC_PI_4 + f64::from(i) * step;
            let d = [libm::cos(angle), 0.0, libm::sin(angle)];
            let here = field.sample(planet(), d, 0.0).elevation_m;
            if let Some(before) = previous {
                assert!(libm::fabs(here - before) < 1e-9, "{before} -> {here}");
            }
            previous = Some(here);
        }
    }

    #[test]
    fn the_fold_fills_every_gutter_texel_and_only_those() {
        let field = sloped();
        for (mip, level) in field.levels.iter().enumerate() {
            let last = level.stride() - 1;
            for sector in Sector::ALL {
                for y in 0..=last {
                    for x in 0..=last {
                        let texel = level.texel(level.index(sector, x, y));
                        let inside = (1..last).contains(&x) && (1..last).contains(&y);
                        assert_ne!(texel, EMPTY, "{sector:?} ({x}, {y}) side {}", level.side);
                        if inside {
                            let want = (
                                (sector.index() * 1000 + x * 3 + y * 7 + mip * 200) as i16,
                                (sector.index() * 20 + x + y) as u8,
                            );
                            assert_eq!(texel, want, "{sector:?} ({x}, {y}) side {}", level.side);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn unequal_faces_meet_at_every_edge_and_corner_at_every_mip() {
        let field = sloped();
        let sphere = planet();
        for footprint in [0.0, 1.0, 1.5, 2.0, 3.0, 4.0, 16.0] {
            for a in 0..3 {
                for b in a + 1..3 {
                    let c = 3 - a - b;
                    for sa in [-1.0, 1.0] {
                        for sb in [-1.0, 1.0] {
                            for along in [-1.0, -0.9999, -0.75, -0.01, 0.0, 0.37, 0.9999, 1.0] {
                                let mut d = [0.0; 3];
                                d[a] = sa;
                                d[b] = sb;
                                d[c] = along;
                                // A hair onto one face, then a hair onto the
                                // other: the same ground from both.
                                let sample = |axis: usize| {
                                    let mut p = d;
                                    p[axis] *= 1.0 + 1e-10;
                                    field.sample(sphere, p, footprint * field.texel_m(sphere))
                                };
                                let (left, right) = (sample(a), sample(b));
                                assert!(
                                    (left.elevation_m - right.elevation_m).abs() < 1e-4
                                        && (left.ruggedness_m - right.ruggedness_m).abs() < 1e-4,
                                    "{d:?}, footprint {footprint}: {left:?} != {right:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn interior_texel_centres_keep_the_laid_values() {
        let field = sloped();
        let sphere = planet();
        for (mip, level) in field.levels.iter().enumerate() {
            let grid = Grid::new(level.side.trailing_zeros()).unwrap();
            for sector in Sector::ALL {
                for y in 0..level.side {
                    for x in 0..level.side {
                        let d = grid.direction(SurfacePoint::new(
                            sector,
                            f64::from(x) + 0.5,
                            f64::from(y) + 0.5,
                        ));
                        let ground =
                            field.sample(sphere, d, field.texel_m(sphere) * (1 << mip) as f64);
                        let want = (sector.index() * 1000
                            + (x as usize + 1) * 3
                            + (y as usize + 1) * 7
                            + mip * 200) as f64;
                        assert!((ground.elevation_m - want).abs() < 1e-8);
                    }
                }
            }
        }
    }

    #[test]
    fn a_corner_averages_its_three_faces() {
        let ground = sloped().sample(planet(), [1.0; 3], 0.0);
        assert!((ground.elevation_m - 2640.0).abs() < 1e-9, "{ground:?}");
        assert!((ground.ruggedness_m - 2688.0).abs() < 1e-9, "{ground:?}");
    }

    /// Seam sampling, frozen: a change here moves ground under every field
    /// world's feet at its sector edges, so it happens on purpose.
    #[test]
    fn field_seam_samples_are_frozen() {
        let field = sloped();
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for i in -3i32..=3 {
            for j in -3i32..=3 {
                for k in -3i32..=3 {
                    if (i, j, k) == (0, 0, 0) {
                        continue;
                    }
                    let d = [f64::from(i), f64::from(j), f64::from(k)];
                    for footprint in [0.0, 512.0, 768.0, 1024.0, 1536.0, 2048.0] {
                        let ground = field.sample(planet(), d, footprint);
                        for value in [ground.elevation_m, ground.ruggedness_m] {
                            for byte in value.to_bits().to_le_bytes() {
                                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(
            hash, 0xba4b_c69c_5175_03a4,
            "field seam sampling changed: {hash:#x}"
        );
    }
}
