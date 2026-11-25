//! An arcade cabinet (`Shape::Cabinet`): a body of the engine's own drawing
//! filling the part's box, the layout a skin is painted on, and where its
//! screen stands.
//!
//! The body is one side profile, ten points from the marquee down the
//! front to the floor, drawn across the box: two side panels standing proud
//! of a front set back between them, as a cabinet's sides frame its
//! marquee, screen and deck. It is a mesh drawn with pictures in three
//! runs: the body, the marquee, lit with its own light, and the dark glass
//! of the screen. With no skin each face is in the part's paint or a dark
//! trim; with one, every face shows its place in the skin.
//!
//! The skin is a picture [`SKIN_SIDE`] texels a side, laid on by a fixed
//! layout, the body unfolded: the front in a column down the middle, from
//! the marquee's top to the floor, each side folded out from it as seen
//! from that side, and a strip at the foot for the top and the trim. One
//! table lays the skin on the body and draws [`template`], the layout for
//! someone to paint over, so a skin painted on it lands where it was
//! painted.
//!
//! The game runs on a quad over the glass, in the middle of the screen face
//! ([`screen`], `crate::carts`).

use glam::{DAffine3, DMat3, DVec3};
use scene::{Image, RunImage, TexturedMesh, TexturedRun, TexturedVertex};

/// A cabinet's size as a hand places it, metres: across, deep and tall.
pub const SIZE_M: [f64; 3] = [0.8, 0.7, 1.8];

/// The side profile, metres back from the front and up from the floor,
/// counter clockwise seen from the right: down the front from the
/// marquee's top to the floor, then along the bottom, up the back and along
/// the top.
const PROFILE: [[f32; 2]; 10] = [
    [0.04, 1.80], // the marquee's top
    [0.04, 1.52], // the marquee's foot, the hood's front
    [0.20, 1.47], // the hood's back, the screen's top
    [0.10, 1.02], // the screen's foot, the deck's back
    [0.00, 0.96], // the deck's front
    [0.00, 0.86], // the deck's front face's foot
    [0.06, 0.82], // under the deck, the front panel's top
    [0.06, 0.00], // the front panel's foot
    [0.70, 0.00], // the back's foot
    [0.70, 1.80], // the back's top
];

/// The profile in triangles, counter clockwise seen from the right: the
/// head over the hood, the screen's wedge, then a fan of the body from the
/// screen's foot.
const TRIANGLES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [0, 2, 9],
    [2, 3, 9],
    [3, 4, 5],
    [3, 5, 6],
    [3, 6, 7],
    [3, 7, 8],
    [3, 8, 9],
];

/// The points of the front, the marquee's top to the floor: the faces
/// between them make the skin's front column.
const FRONT_POINTS: usize = 8;
/// The faces of the front by their first point.
const MARQUEE: usize = 0;
const SCREEN_FACE: usize = 2;

/// How far the front stands back of the side panels, and how thick a side
/// panel is, metres.
const SET_BACK_M: f32 = 0.015;
const SIDE_M: f32 = 0.025;

/// How much of the screen face's height the game fills, and how far over
/// the face its glass and its picture stand, metres.
const SCREEN_FILL: f64 = 0.8;
const GLASS_OVER_M: f64 = 0.002;
const PICTURE_OVER_M: f64 = 0.004;
/// The game's width over its height, the emulator's `arcade::WIDTH` by
/// `HEIGHT`, which a test holds it to: the client draws a game's screen without knowing
/// the contract it runs by.
const SCREEN_ASPECT: f64 = 4.0 / 3.0;

/// How much the marquee shines with its own colour: a lamp, brighter than
/// any sunlit white, so it is seen lit by day and glows by night.
pub const MARQUEE_SHINE: f32 = 1.5;

/// The side of a skin, texels.
pub const SKIN_SIDE: u32 = 512;

/// A place of the skin, texels from its top left corner, and how the
/// template shows it: its name, which a shell words, and its tint. The
/// front's place is named face by face ([`FACES`]).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Region {
    rect: [u32; 4],
    name: Option<&'static str>,
    tint: [u8; 3],
}

/// Where the skin lays each part of the body: the left side seen from the
/// left, its front at its right edge; the front, a column from the
/// marquee's top to the floor; the right side seen from the right, its
/// front at its left edge; at the foot, the trim (the back, the bottom, the
/// panels' edges and inner faces) and the top, seen from above by someone
/// standing in front, its front at its lower edge. The rest is unused.
const LEFT: Region = Region {
    rect: [0, 0, 176, 448],
    name: Some("left"),
    tint: [196, 214, 240],
};
const FRONT: Region = Region {
    rect: [176, 0, 336, 448],
    name: None,
    tint: [240, 226, 196],
};
const RIGHT: Region = Region {
    rect: [336, 0, 512, 448],
    name: Some("right"),
    tint: LEFT.tint,
};
const TRIM: Region = Region {
    rect: [0, 448, 176, 512],
    name: Some("trim"),
    tint: [190, 190, 200],
};
const TOP: Region = Region {
    rect: [176, 448, 336, 512],
    name: Some("top"),
    tint: [206, 236, 206],
};
const REGIONS: [Region; 5] = [LEFT, FRONT, RIGHT, TRIM, TOP];

/// A face of the front, from one of its points to the next: whether it is
/// the bezel round the screen, dark with no skin, its tint on the template,
/// and its name there when it has room for one.
struct Face {
    bezel: bool,
    tint: [u8; 3],
    name: Option<&'static str>,
}

/// The faces of the front, down its column from the marquee's top.
const FACES: [Face; FRONT_POINTS - 1] = [
    face(false, [255, 246, 160], Some("marquee")),
    face(true, BEZEL_TINT, None),
    face(true, BEZEL_TINT, None),
    face(false, FRONT.tint, Some("deck")),
    face(false, FRONT.tint, None),
    face(false, FRONT.tint, None),
    face(false, FRONT.tint, Some("panel")),
];
const fn face(bezel: bool, tint: [u8; 3], name: Option<&'static str>) -> Face {
    Face { bezel, tint, name }
}
const BEZEL_TINT: [u8; 3] = [150, 150, 160];
/// The game's rectangle on the template, which hides what is painted there.
const GAME_TINT: [u8; 3] = [40, 44, 52];
/// The template's rules and words, and the words on the game's rectangle.
const INK: [u8; 3] = [60, 60, 70];
const INK_ON_DARK: [u8; 3] = [236, 236, 240];

/// The colours of a cabinet with no skin, sRGB: its trim, the bezel and
/// hood round its screen, the glass of its screen, and its marquee.
const TRIM_RGB: [u8; 3] = [38, 38, 44];
const BEZEL_RGB: [u8; 3] = [20, 20, 24];
const GLASS_RGB: [u8; 3] = [8, 10, 12];
const MARQUEE_RGB: [u8; 3] = [240, 236, 224];

/// The profile in the unit box, `[y, z]`: the front at `y = -0.5`, the
/// floor at `z = 0`. `front` sets the front back between the panels.
fn profile(front: bool) -> [[f32; 2]; 10] {
    let [_, deep, tall] = SIZE_M.map(|m| m as f32);
    let mut points = PROFILE;
    if front {
        for point in &mut points[..FRONT_POINTS] {
            point[0] += SET_BACK_M;
        }
    }
    points.map(|[d, h]| [d / deep - 0.5, h / tall])
}

/// Where each point of the set back front falls down the skin's front
/// column, texels from its top: by how far along the front it is.
fn front_stops() -> [f32; FRONT_POINTS] {
    let mut along = [0.0; FRONT_POINTS];
    for i in 1..FRONT_POINTS {
        let ([d0, h0], [d1, h1]) = (PROFILE[i - 1], PROFILE[i]);
        along[i] = along[i - 1] + (d1 - d0).hypot(h1 - h0);
    }
    let [_, top, _, bottom] = FRONT.rect.map(|n| n as f32);
    let total = along[FRONT_POINTS - 1];
    along.map(|s| top + s / total * (bottom - top))
}

/// The half width of the front between the panels, in the unit box.
fn inner_half() -> f32 {
    0.5 - SIDE_M / SIZE_M[0] as f32
}

/// A texel of the skin as a corner's place on it.
fn uv(x: f32, y: f32) -> [f32; 2] {
    let side = SKIN_SIDE as f32;
    [x / side, y / side]
}

/// Linear from `a` to `b` as `t` goes from 0 to 1.
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A mesh drawn with pictures, built a face at a time, its runs in order.
#[derive(Default)]
struct Builder {
    vertices: Vec<TexturedVertex>,
    indices: Vec<u32>,
    runs: Vec<TexturedRun>,
}

impl Builder {
    /// A quad, its corners counter clockwise seen from outside, lit flat.
    fn quad(&mut self, corners: [[f32; 3]; 4], uvs: [[f32; 2]; 4], rgb: [u8; 3]) {
        let normal = normal_of(corners[0], corners[1], corners[2]);
        let base = self.vertices.len() as u32;
        for (position, uv) in corners.into_iter().zip(uvs) {
            self.vertices.push(TexturedVertex {
                position,
                normal,
                uv,
                color: [rgb[0], rgb[1], rgb[2], u8::MAX],
            });
        }
        self.indices
            .extend([0, 1, 2, 0, 2, 3].map(|corner| base + corner));
    }

    /// The profile standing at `x`, facing `+x` or `-x`, each corner at the
    /// place `uv` gives it.
    fn side(
        &mut self,
        points: &[[f32; 2]; 10],
        x: f32,
        facing_right: bool,
        uv: impl Fn([f32; 2]) -> [f32; 2],
        rgb: [u8; 3],
    ) {
        let base = self.vertices.len() as u32;
        let normal = [if facing_right { 1.0 } else { -1.0 }, 0.0, 0.0];
        for &[y, z] in points {
            self.vertices.push(TexturedVertex {
                position: [x, y, z],
                normal,
                uv: uv([y, z]),
                color: [rgb[0], rgb[1], rgb[2], u8::MAX],
            });
        }
        for [a, b, c] in TRIANGLES {
            let [a, b, c] = [a, b, c].map(|i| base + i as u32);
            match facing_right {
                true => self.indices.extend([a, b, c]),
                false => self.indices.extend([a, c, b]),
            }
        }
    }

    /// Closes the run of every index since the last, drawn with `image`.
    fn run(&mut self, image: RunImage, shine: f32) {
        let start = self.runs.last().map_or(0, |run| run.indices.end);
        self.runs.push(TexturedRun {
            indices: start..self.indices.len() as u32,
            image,
            cutoff: 0.0,
            shine,
        });
    }
}

fn normal_of(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let (a, b, c) = (
        glam::Vec3::from(a),
        glam::Vec3::from(b),
        glam::Vec3::from(c),
    );
    (b - a).cross(c - a).normalize_or_zero().to_array()
}

/// The strip across the box along one edge of a profile, from `x0` to
/// `x1`, facing out of the profile: `p` to `q` counter clockwise seen from
/// the right.
fn strip(p: [f32; 2], q: [f32; 2], x0: f32, x1: f32) -> [[f32; 3]; 4] {
    [
        [x0, p[0], p[1]],
        [x0, q[0], q[1]],
        [x1, q[0], q[1]],
        [x1, p[0], p[1]],
    ]
}

/// How a cabinet's body is drawn: in the part's paint, or dressed in a
/// skin, the picture its draw names ([`RunImage::Drawn`]), so one dressed
/// body serves every skin.
#[derive(Clone, Copy)]
pub(crate) enum Dress {
    Paint([u8; 3]),
    Skin,
}

/// A cabinet's body, in three runs: the body, then the marquee, then the
/// screen's glass, which no skin dresses.
pub(crate) fn body(dress: Dress) -> TexturedMesh {
    let (dressed, paint_rgb) = match dress {
        Dress::Paint(rgb) => (false, rgb),
        Dress::Skin => (true, [u8::MAX; 3]),
    };
    let image = match dressed {
        true => RunImage::Drawn,
        false => RunImage::Plain,
    };
    // A dressed face is the skin's colour alone.
    let hue = |bare: [u8; 3]| if dressed { [u8::MAX; 3] } else { bare };
    let outer = profile(false);
    let front = profile(true);
    let half = inner_half();
    let stops = front_stops();
    let [front_x0, _, front_x1, _] = FRONT.rect.map(|n| n as f32);
    let [left_x0, top_y, left_x1, foot_y] = LEFT.rect.map(|n| n as f32);
    let [right_x0, ..] = RIGHT.rect.map(|n| n as f32);
    let [trim_x0, trim_y0, trim_x1, trim_y1] = TRIM.rect.map(|n| n as f32);
    let [top_x0, top_y0, top_x1, top_y1] = TOP.rect.map(|n| n as f32);
    let side_width = left_x1 - left_x0;
    // A point of the profile, `[y, z]` in the box, as fractions back from
    // the front and down from the top.
    let back = |y: f32| y + 0.5;
    let down = |z: f32| 1.0 - z;
    let mut mesh = Builder::default();

    // The side panels: each outer face where its side of the skin says,
    // each inner face and edge on the trim.
    mesh.side(
        &outer,
        -0.5,
        false,
        |[y, z]| uv(left_x1 - back(y) * side_width, mix(top_y, foot_y, down(z))),
        hue(paint_rgb),
    );
    mesh.side(
        &outer,
        0.5,
        true,
        |[y, z]| uv(right_x0 + back(y) * side_width, mix(top_y, foot_y, down(z))),
        hue(paint_rgb),
    );
    let on_trim = |[y, z]: [f32; 2]| {
        uv(
            mix(trim_x0, trim_x1, back(y)),
            mix(trim_y0, trim_y1, down(z)),
        )
    };
    mesh.side(&outer, -half, true, on_trim, hue(TRIM_RGB));
    mesh.side(&outer, half, false, on_trim, hue(TRIM_RGB));
    for i in 0..outer.len() {
        let (p, q) = (outer[i], outer[(i + 1) % outer.len()]);
        let along = [i, i + 1].map(|n| mix(trim_x0, trim_x1, n as f32 / outer.len() as f32));
        let uvs = |x0: f32, x1: f32| {
            [
                uv(along[0], mix(trim_y0, trim_y1, x0)),
                uv(along[1], mix(trim_y0, trim_y1, x0)),
                uv(along[1], mix(trim_y0, trim_y1, x1)),
                uv(along[0], mix(trim_y0, trim_y1, x1)),
            ]
        };
        mesh.quad(strip(p, q, -0.5, -half), uvs(0.0, 1.0), hue(TRIM_RGB));
        mesh.quad(strip(p, q, half, 0.5), uvs(0.0, 1.0), hue(TRIM_RGB));
    }

    // The front between the panels, face by face down the column, but the
    // marquee; then the bottom and the back on the trim, the top on its own.
    let column = |p: usize, q: usize| {
        [
            uv(front_x0, stops[p]),
            uv(front_x0, stops[q]),
            uv(front_x1, stops[q]),
            uv(front_x1, stops[p]),
        ]
    };
    for face in (0..FRONT_POINTS - 1).filter(|&face| face != MARQUEE) {
        let rgb = if FACES[face].bezel {
            BEZEL_RGB
        } else {
            paint_rgb
        };
        mesh.quad(
            strip(front[face], front[face + 1], -half, half),
            column(face, face + 1),
            hue(rgb),
        );
    }
    let [floor_front, floor_back, top_back, top_front] = [
        front[FRONT_POINTS - 1],
        front[FRONT_POINTS],
        front[FRONT_POINTS + 1],
        front[0],
    ];
    let spread = |y0: f32, y1: f32| {
        [
            uv(trim_x0, mix(trim_y0, trim_y1, y0)),
            uv(trim_x0, mix(trim_y0, trim_y1, y1)),
            uv(trim_x1, mix(trim_y0, trim_y1, y1)),
            uv(trim_x1, mix(trim_y0, trim_y1, y0)),
        ]
    };
    mesh.quad(
        strip(floor_front, floor_back, -half, half),
        spread(back(floor_front[0]), back(floor_back[0])),
        hue(TRIM_RGB),
    );
    mesh.quad(
        strip(floor_back, top_back, -half, half),
        spread(1.0, 0.0),
        hue(TRIM_RGB),
    );
    // Seen from above by someone in front: the front at the lower edge.
    let lid = |y: f32| mix(top_y1, top_y0, back(y));
    mesh.quad(
        strip(top_back, top_front, -half, half),
        [
            uv(top_x0, lid(top_back[0])),
            uv(top_x0, lid(top_front[0])),
            uv(top_x1, lid(top_front[0])),
            uv(top_x1, lid(top_back[0])),
        ],
        hue(paint_rgb),
    );
    mesh.run(image, 0.0);

    mesh.quad(
        strip(front[MARQUEE], front[MARQUEE + 1], -half, half),
        column(MARQUEE, MARQUEE + 1),
        hue(MARQUEE_RGB),
    );
    mesh.run(image, MARQUEE_SHINE);

    let glass = screen_at(GLASS_OVER_M);
    let corner = |x: f64, y: f64| glass.transform_point3(DVec3::new(x, y, 0.0)).as_vec3();
    let corners = [
        corner(-0.5, -0.5),
        corner(0.5, -0.5),
        corner(0.5, 0.5),
        corner(-0.5, 0.5),
    ]
    .map(|corner| corner.to_array());
    mesh.quad(corners, [[0.0; 2]; 4], GLASS_RGB);
    mesh.run(RunImage::Plain, 0.0);

    TexturedMesh {
        vertices: mesh.vertices,
        indices: mesh.indices,
        runs: mesh.runs,
        images: Vec::new(),
    }
}

/// Where the game's picture stands in a cabinet's box: the unit quad,
/// facing `+z`, laid over the middle of the screen face, its top at the
/// face's top.
pub fn screen() -> DAffine3 {
    screen_at(PICTURE_OVER_M)
}

/// The game's rectangle on the screen face, `over_m` out of it.
fn screen_at(over_m: f64) -> DAffine3 {
    let size = DVec3::from(SIZE_M);
    // Metres from the box's front and floor, and back into the box.
    let metres =
        |[d, h]: [f32; 2]| DVec3::new(0.0, f64::from(d) + f64::from(SET_BACK_M), f64::from(h));
    let boxed = |m: DVec3| DVec3::new(m.x / size.x, m.y / size.y - 0.5, m.z / size.z);
    let (top, foot) = (
        metres(PROFILE[SCREEN_FACE]),
        metres(PROFILE[SCREEN_FACE + 1]),
    );
    let up = (top - foot).normalize();
    // Out of the face: toward the front, and up as the face leans back.
    let out = DVec3::new(0.0, -up.z, up.y);
    let tall_m = (top - foot).length() * SCREEN_FILL;
    let wide_m = tall_m * SCREEN_ASPECT;
    let centre = (top + foot) / 2.0 + out * over_m;
    DAffine3 {
        matrix3: DMat3::from_cols(
            DVec3::new(wide_m / size.x, 0.0, 0.0),
            DVec3::new(0.0, up.y * tall_m / size.y, up.z * tall_m / size.z),
            DVec3::new(0.0, out.y / size.y, out.z / size.z) * 0.01,
        ),
        translation: boxed(centre),
    }
}

/// The layout a skin is painted on, drawn for a person to paint over, and
/// where its places are named. A shell writes each name in its own words.
pub struct Template {
    pub image: Image,
    pub labels: Vec<Label>,
}

/// A place named on the template: `left`, `right`, `trim`, `top`,
/// `marquee`, `game`, `deck` or `panel`, the rectangle its word is written
/// in the middle of, texels, and the colour that reads on it, sRGB.
#[derive(serde::Serialize)]
pub struct Label {
    pub name: &'static str,
    pub rect: [f32; 4],
    pub ink: [u8; 3],
}

/// The template, drawn from the table that lays the skin on the body: each
/// place in its tint and ruled round, the sides in their silhouettes, the
/// front's faces ruled apart, the game's rectangle, which hides what is
/// painted under it, dark. What is unused is a grey check.
pub fn template() -> Template {
    const UNUSED: [[u8; 3]; 2] = [[200, 200, 200], [228, 228, 228]];
    let side = SKIN_SIDE as usize;
    let mut rgba = vec![u8::MAX; side * side * 4];
    let mut put = |x: usize, y: usize, [r, g, b]: [u8; 3]| {
        rgba[(y * side + x) * 4..][..3].copy_from_slice(&[r, g, b]);
    };
    let inside = |region: &Region, x: f32, y: f32| {
        let [x0, y0, x1, y1] = region.rect.map(|n| n as f32);
        (x0..x1).contains(&x) && (y0..y1).contains(&y)
    };
    let stops = front_stops();
    let outer = profile(false);
    let [gx0, gy0, gx1, gy1] = game_on_skin();
    let face_at = |y: f32| {
        (0..FRONT_POINTS - 1)
            .find(|&face| y < stops[face + 1])
            .unwrap_or(FRONT_POINTS - 2)
    };
    for y in 0..side {
        for x in 0..side {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let unused = UNUSED[(x / 8 + y / 8) % 2];
            let region = REGIONS.iter().find(|region| inside(region, fx, fy));
            let colour = match region {
                None => unused,
                Some(side @ (&LEFT | &RIGHT)) => {
                    let [x0, y0, x1, y1] = side.rect.map(|n| n as f32);
                    let across = (fx - x0) / (x1 - x0);
                    // Fractions back from the front and up from the floor.
                    let back = if *side == LEFT { 1.0 - across } else { across };
                    let up = 1.0 - (fy - y0) / (y1 - y0);
                    match within(&outer, [back - 0.5, up]) {
                        true => side.tint,
                        false => unused,
                    }
                }
                Some(&FRONT) if (gx0..gx1).contains(&fx) && (gy0..gy1).contains(&fy) => GAME_TINT,
                Some(&FRONT) => FACES[face_at(fy)].tint,
                Some(region) => region.tint,
            };
            put(x, y, colour);
        }
    }
    // The rules: round every place, and between the front's faces.
    for region in REGIONS {
        let [x0, y0, x1, y1] = region.rect.map(|n| n as usize);
        for x in x0..x1 {
            put(x, y0, INK);
            put(x, y1 - 1, INK);
        }
        for y in y0..y1 {
            put(x0, y, INK);
            put(x1 - 1, y, INK);
        }
    }
    let [x0, _, x1, _] = FRONT.rect.map(|n| n as usize);
    for &stop in &stops[1..FRONT_POINTS - 1] {
        for x in x0..x1 {
            put(x, stop as usize, INK);
        }
    }

    let [fx0, _, fx1, _] = FRONT.rect.map(|n| n as f32);
    let regions = REGIONS.iter().filter_map(|region| {
        Some(Label {
            name: region.name?,
            rect: region.rect.map(|n| n as f32),
            ink: INK,
        })
    });
    let faces = FACES.iter().enumerate().filter_map(|(i, face)| {
        Some(Label {
            name: face.name?,
            rect: [fx0, stops[i], fx1, stops[i + 1]],
            ink: INK,
        })
    });
    let game = Label {
        name: "game",
        rect: [gx0, gy0, gx1, gy1],
        ink: INK_ON_DARK,
    };
    Template {
        image: Image {
            width: SKIN_SIDE,
            height: SKIN_SIDE,
            rgba,
        },
        labels: regions.chain(faces).chain([game]).collect(),
    }
}

/// The game's rectangle on the skin's front column, texels: what the
/// picture covers of the screen face.
fn game_on_skin() -> [f32; 4] {
    let stops = front_stops();
    let [x0, _, x1, _] = FRONT.rect.map(|n| n as f32);
    let (top, foot) = (stops[SCREEN_FACE], stops[SCREEN_FACE + 1]);
    let tall = (foot - top) * SCREEN_FILL as f32;
    let wide_m = SCREEN_FILL * SCREEN_ASPECT * {
        let ([d0, h0], [d1, h1]) = (PROFILE[SCREEN_FACE], PROFILE[SCREEN_FACE + 1]);
        f64::from((d1 - d0).hypot(h1 - h0))
    };
    let front_m = SIZE_M[0] - 2.0 * f64::from(SIDE_M);
    let wide = (x1 - x0) * (wide_m / front_m) as f32;
    let (middle_x, middle_y) = ((x0 + x1) / 2.0, (top + foot) / 2.0);
    [
        middle_x - wide / 2.0,
        middle_y - tall / 2.0,
        middle_x + wide / 2.0,
        middle_y + tall / 2.0,
    ]
}

/// Whether a point `[y, z]` of the box's side lies inside a profile.
fn within(points: &[[f32; 2]; 10], [y, z]: [f32; 2]) -> bool {
    let mut inside = false;
    for i in 0..points.len() {
        let ([ay, az], [by, bz]) = (points[i], points[(i + 1) % points.len()]);
        if (az > z) != (bz > z) && y < ay + (z - az) / (bz - az) * (by - ay) {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twice the signed area of a triangle of the profile: positive when it
    /// turns counter clockwise seen from the right.
    fn turn([a, b, c]: [[f32; 2]; 3]) -> f32 {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }

    #[test]
    fn the_profile_is_counter_clockwise_and_its_triangles_cover_it_once() {
        let points = profile(false);
        let mut area = 0.0;
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            area += a[0] * b[1] - b[0] * a[1];
        }
        let triangles: f32 = TRIANGLES
            .iter()
            .map(|t| {
                let turned = turn(t.map(|i| points[i]));
                assert!(turned > 0.0, "{t:?} turns clockwise");
                turned
            })
            .sum();
        assert!(area > 0.0);
        assert!((triangles - area).abs() < 1e-5, "{triangles} over {area}");
    }

    /// Whether a point of the box is inside the body: in a side panel, or
    /// in the front between them.
    fn solid(at: glam::Vec3) -> bool {
        let x = at.x.abs();
        let panel = x <= 0.5 && x >= inner_half() && within(&profile(false), [at.y, at.z]);
        panel || x < inner_half() && within(&profile(true), [at.y, at.z])
    }

    #[test]
    fn the_body_fills_its_box_and_every_face_looks_out() {
        let mesh = body(Dress::Paint([200, 40, 40]));
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for vertex in &mesh.vertices {
            for i in 0..3 {
                low[i] = low[i].min(vertex.position[i]);
                high[i] = high[i].max(vertex.position[i]);
            }
        }
        assert_eq!(low, [-0.5, -0.5, 0.0]);
        assert_eq!(high, [0.5, 0.5, 1.0]);
        for triangle in mesh.indices.chunks_exact(3) {
            let [a, b, c] =
                [0, 1, 2].map(|k| glam::Vec3::from(mesh.vertices[triangle[k] as usize].position));
            let normal = (b - a).cross(c - a).normalize();
            let middle = (a + b + c) / 3.0;
            // A panel's inner face is the one that looks in, toward the
            // middle of the body, and is seen only where the front stands
            // back of it.
            if normal.x != 0.0 && (middle.x.abs() - inner_half()).abs() < 1e-6 {
                assert!(normal.x * middle.x < 0.0, "a panel's inner face looks out");
                continue;
            }
            assert!(!solid(middle + normal * 1e-3), "{triangle:?} looks in");
        }
        let runs: Vec<_> = mesh.runs.iter().map(|run| (run.image, run.shine)).collect();
        assert_eq!(
            runs,
            [
                (RunImage::Plain, 0.0),
                (RunImage::Plain, MARQUEE_SHINE),
                (RunImage::Plain, 0.0)
            ]
        );
        assert_eq!(
            mesh.runs.last().map(|run| run.indices.end),
            Some(mesh.indices.len() as u32)
        );
    }

    #[test]
    fn a_dressed_body_shows_its_skin_and_its_glass_stays_dark() {
        let mesh = body(Dress::Skin);
        let runs: Vec<_> = mesh.runs.iter().map(|run| run.image).collect();
        assert_eq!(runs, [RunImage::Drawn, RunImage::Drawn, RunImage::Plain]);
        assert!(mesh.images.is_empty());
        let glass = &mesh.runs[2];
        for &index in &mesh.indices[glass.indices.start as usize..glass.indices.end as usize] {
            assert_eq!(mesh.vertices[index as usize].color, [8, 10, 12, 255]);
        }
        for vertex in &mesh.vertices {
            assert!(vertex.uv.iter().all(|t| (0.0..=1.0).contains(t)));
        }
    }

    #[test]
    fn the_screen_lies_on_its_face_facing_out_and_shaped_as_a_game() {
        let game = f64::from(arcade::WIDTH) / f64::from(arcade::HEIGHT);
        assert!((SCREEN_ASPECT - game).abs() < 1e-12);
        let quad = screen();
        let size = DVec3::from(SIZE_M);
        let metres = |v: DVec3| v * size;
        let (wide, tall) = (
            metres(quad.matrix3.x_axis).length(),
            metres(quad.matrix3.y_axis).length(),
        );
        assert!((wide / tall - SCREEN_ASPECT).abs() < 1e-9);
        // It faces the front and leans back.
        let facing = quad.matrix3.x_axis.cross(quad.matrix3.y_axis);
        assert!(facing.y < 0.0 && facing.z > 0.0);
        // Inside the box, between the panels.
        for corner in [[-0.5, -0.5], [0.5, 0.5]] {
            let at = quad.transform_point3(DVec3::new(corner[0], corner[1], 0.0));
            assert!(at.x.abs() < f64::from(inner_half()));
            assert!((-0.5..0.5).contains(&at.y) && (0.0..1.0).contains(&at.z));
        }
    }

    /// The template's colour at a place of the skin, `uv` from 0 to 1.
    fn sample(template: &Image, [u, v]: [f32; 2]) -> [u8; 3] {
        let side = SKIN_SIDE as f32;
        let (x, y) = ((u * side) as usize, (v * side) as usize);
        let at = (y * SKIN_SIDE as usize + x) * 4;
        [0, 1, 2].map(|k| template.rgba[at + k])
    }

    #[test]
    fn the_template_is_the_skin_side_and_marks_the_game() {
        let template = template().image;
        assert_eq!((template.width, template.height), (SKIN_SIDE, SKIN_SIDE));
        let [x0, y0, x1, y1] = game_on_skin();
        let middle = uv((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        assert_eq!(sample(&template, middle), GAME_TINT);
        assert!(x0 > FRONT.rect[0] as f32 && x1 < FRONT.rect[2] as f32);
    }

    /// A skin painted over the template lands where it was painted: the
    /// middle of every face of a dressed body reads the template's place
    /// for it, never what is unused, and the front's faces read their own
    /// tint.
    #[test]
    fn every_face_of_the_body_lands_on_its_place_in_the_template() {
        let template = template().image;
        let mesh = body(Dress::Skin);
        let tints: Vec<[u8; 3]> = REGIONS
            .iter()
            .map(|region| region.tint)
            .chain(FACES.iter().map(|face| face.tint))
            .chain([GAME_TINT])
            .collect();
        let dressed = mesh.runs.iter().filter(|run| run.image == RunImage::Drawn);
        for run in dressed {
            for triangle in
                mesh.indices[run.indices.start as usize..run.indices.end as usize].chunks_exact(3)
            {
                let corners = [0, 1, 2].map(|k| &mesh.vertices[triangle[k] as usize]);
                let middle = [0, 1].map(|k| corners.iter().map(|c| c.uv[k]).sum::<f32>() / 3.0);
                let read = sample(&template, middle);
                assert!(
                    tints.contains(&read),
                    "{triangle:?} reads {read:?} at {middle:?}"
                );
                let normal = corners[0].normal;
                let [x, y] = middle.map(|t| t * SKIN_SIDE as f32);
                let on_front = (FRONT.rect[0] as f32..FRONT.rect[2] as f32).contains(&x)
                    && y < FRONT.rect[3] as f32;
                if on_front && normal[0] == 0.0 {
                    let stops = front_stops();
                    let face = (0..FRONT_POINTS - 1)
                        .find(|&face| y < stops[face + 1])
                        .expect("on the front");
                    assert!([FACES[face].tint, GAME_TINT].contains(&read));
                }
            }
        }
    }

    #[test]
    fn the_template_names_each_place_once_inside_it() {
        let labels = template().labels;
        let mut names: Vec<_> = labels.iter().map(|label| label.name).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "deck", "game", "left", "marquee", "panel", "right", "top", "trim"
            ]
        );
        for label in &labels {
            let [x0, y0, x1, y1] = label.rect;
            assert!(
                0.0 <= x0 && x0 < x1 && x1 <= SKIN_SIDE as f32,
                "{}",
                label.name
            );
            assert!(
                0.0 <= y0 && y0 < y1 && y1 <= SKIN_SIDE as f32,
                "{}",
                label.name
            );
        }
    }
}
