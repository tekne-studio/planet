//! A screen the eye is held on, laid over the view as drawn
//! ([`scene::Overlay`]): while the controls are lent to a game, its picture
//! is the person's screen, not a thing in the world. It is drawn after the
//! compositor, in the view's own pixels, each texel the same whole number
//! of them: the largest that fits inside where the world's screen stands
//! on the view, and inside the part of the view the shell leaves clear of
//! what it lays over it ([`clear`]), one at least.

use glam::{DAffine3, DVec3};
use scene::{Camera, Overlay, PictureId};

/// How much of the view is left clear, across and down, when the shell
/// says nothing of what it lays over it: room round the middle for a row
/// of keys over a screen and a card under it, as a page with keys lays
/// them.
const CLEAR_FILLS: f64 = 0.7;

/// How far the clear part's middle sits over the view's, of the view's
/// height, when the shell says nothing: under it the card offers leaving.
const CLEAR_RISES: f64 = 0.06;

/// The part of a view `view` pixels the shell leaves clear,
/// `[left, top, right, bottom]` in pixels from its top left: `said`, as
/// far as it lies inside the view and is a pixel at least each way; with
/// none said, a margin round the middle ([`CLEAR_FILLS`],
/// [`CLEAR_RISES`]).
pub(crate) fn clear(view: [u32; 2], said: Option<[f64; 4]>) -> [f64; 4] {
    let [wide, tall] = view.map(|n| f64::from(n.max(1)));
    let inside =
        said.filter(|part| part.iter().all(|n| n.is_finite()))
            .map(|[left, top, right, bottom]| {
                [
                    left.clamp(0.0, wide),
                    top.clamp(0.0, tall),
                    right.clamp(0.0, wide),
                    bottom.clamp(0.0, tall),
                ]
            });
    match inside {
        Some(part @ [left, top, right, bottom]) if right - left >= 1.0 && bottom - top >= 1.0 => {
            part
        }
        _ => {
            let (half_wide, half_tall) = (wide * CLEAR_FILLS / 2.0, tall * CLEAR_FILLS / 2.0);
            let middle = tall * (0.5 - CLEAR_RISES);
            [
                wide / 2.0 - half_wide,
                middle - half_tall,
                wide / 2.0 + half_wide,
                middle + half_tall,
            ]
        }
    }
}

/// The overlay of a picture `texels` wide and tall shown on the unit quad
/// `quad` (world from quad, `-0.5..=0.5` across `x` and `y`), as `camera`
/// sees it on a view `view` pixels whose `clear` part ([`clear`]) is left
/// for it: centred where the quad's middle lands, moved inside the clear
/// part where it would cross its edge. `None` for a quad behind the eye,
/// or a picture larger than the view.
pub(crate) fn laid(
    camera: &Camera,
    view: [u32; 2],
    clear: [f64; 4],
    quad: DAffine3,
    picture: PictureId,
    texels: (u32, u32),
    opacity: f32,
) -> Option<Overlay> {
    let (texels_wide, texels_tall) = (texels.0.max(1), texels.1.max(1));
    let [view_wide, view_tall] = view;
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for (x, y) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
        let [px, py] = on_view(camera, view, quad.transform_point3(DVec3::new(x, y, 0.0)))?;
        low = [low[0].min(px), low[1].min(py)];
        high = [high[0].max(px), high[1].max(py)];
    }
    let [left, top, right, bottom] = clear;
    // The largest whole scale inside the quad as seen, and inside the
    // clear part.
    let fits = |pixels: f64, texels: u32| (pixels / f64::from(texels)).floor().max(0.0) as u32;
    let scale = fits(high[0] - low[0], texels_wide)
        .min(fits(high[1] - low[1], texels_tall))
        .min(fits(right - left, texels_wide))
        .min(fits(bottom - top, texels_tall))
        .max(1);
    let (wide, tall) = (texels_wide * scale, texels_tall * scale);
    if wide > view_wide || tall > view_tall {
        return None;
    }
    Some(Overlay {
        picture,
        rect: [
            corner((low[0] + high[0]) / 2.0, wide, [left, right], view_wide),
            corner((low[1] + high[1]) / 2.0, tall, [top, bottom], view_tall),
            wide,
            tall,
        ],
        opacity,
    })
}

/// Where a picture `size` pixels along one way of the view starts, its
/// middle at `middle`: inside the whole pixels of the clear part `clear`
/// along that way, or of the view, `room` pixels, where it is wider than
/// the clear part.
fn corner(middle: f64, size: u32, [low, high]: [f64; 2], room: u32) -> u32 {
    let size = f64::from(size);
    let (first, last) = match (low.ceil(), (high - size).floor()) {
        (first, last) if first <= last => (first, last),
        _ => (0.0, f64::from(room) - size),
    };
    (middle - size / 2.0).round().clamp(first, last) as u32
}

/// Where a point lands on a view `view` pixels from its top left, as
/// `camera` sees it; `None` behind the eye.
fn on_view(camera: &Camera, view: [u32; 2], point: DVec3) -> Option<[f64; 2]> {
    let seen = camera.rotation.inverse() * (point - camera.position);
    if seen.z >= 0.0 {
        return None;
    }
    let [wide, tall] = view.map(|n| f64::from(n.max(1)));
    let half_tall = (f64::from(camera.fov_y) / 2.0).tan();
    let ndc = [
        seen.x / -seen.z / (half_tall * wide / tall),
        seen.y / -seen.z / half_tall,
    ];
    Some([(ndc[0] + 1.0) / 2.0 * wide, (1.0 - ndc[1]) / 2.0 * tall])
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat3, DQuat};

    /// A game's picture on a quad facing the eye, `away_m` ahead, a metre
    /// by 0.75, its middle `up_m` over the eye's line, seen on a view
    /// `view` pixels by an eye a radian tall, the view clear as the shell
    /// says nothing.
    fn lay(view: [u32; 2], away_m: f64, up_m: f64) -> Option<Overlay> {
        lay_in(view, clear(view, None), away_m, up_m)
    }

    fn lay_in(view: [u32; 2], clear: [f64; 4], away_m: f64, up_m: f64) -> Option<Overlay> {
        let camera = Camera {
            position: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            fov_y: 1.0,
            near: 0.1,
        };
        let quad = DAffine3 {
            matrix3: DMat3::from_diagonal(DVec3::new(1.0, 0.75, 1.0)),
            translation: DVec3::new(0.0, up_m, -away_m),
        };
        laid(&camera, view, clear, quad, PictureId(3), (160, 120), 1.0)
    }

    /// How far ahead the quad fills `share` of the view's height.
    fn filling_m(share: f64) -> f64 {
        0.75 / (2.0 * share * 0.5f64.tan())
    }

    /// Where the quad fills the clear part a silent shell leaves, as a
    /// cabinet's screen does when played: how far ahead, and how far over
    /// the eye's line, so its middle is the clear part's.
    fn played() -> (f64, f64) {
        let away_m = filling_m(CLEAR_FILLS);
        (away_m, CLEAR_RISES * 2.0 * away_m * 0.5f64.tan())
    }

    #[test]
    fn the_largest_whole_scale_inside_the_screen_as_seen() {
        let (away_m, up_m) = played();
        // 504 pixels tall of 720: four a texel, its middle the screen's.
        let wide = lay([1280, 720], away_m, up_m).unwrap();
        assert_eq!(wide.rect, [320, 77, 640, 480]);
        assert_eq!(wide.picture, PictureId(3));
        // A phone held sideways, 2532 by 1170 pixels: six a texel.
        let phone = lay([2532, 1170], away_m, up_m).unwrap();
        assert_eq!(phone.rect[2..], [960, 720]);
        // A small window, 800 by 450: two.
        let small = lay([800, 450], away_m, up_m).unwrap();
        assert_eq!(small.rect[2..], [320, 240]);
    }

    #[test]
    fn a_screen_smaller_than_its_picture_is_laid_at_one_pixel_a_texel() {
        let far = lay([1280, 720], 20.0, 0.0).unwrap();
        assert_eq!(far.rect, [560, 300, 160, 120]);
    }

    #[test]
    fn it_stays_inside_the_clear_part() {
        // A screen whose middle is near the view's top edge.
        let view = [1280, 720];
        let [_, top, ..] = clear(view, None);
        let high = lay(view, 2.0, 1.0).unwrap();
        let [x, y, wide, tall] = high.rect;
        assert!(x + wide <= 1280 && y + tall <= 720);
        assert_eq!(f64::from(y), top.ceil());
        // A view smaller than the picture lays nothing; nor does a screen
        // behind the eye.
        assert!(lay([100, 100], 2.0, 0.0).is_none());
        assert!(lay([1280, 720], -2.0, 0.0).is_none());
    }

    #[test]
    fn a_phone_upright_lays_the_game_across_the_part_its_pad_leaves() {
        // 1170 by 2532 pixels, a pad over the lowest 700: the quad fills
        // the 1170 across and the game takes seven pixels a texel, its
        // middle where the quad's lands, inside the clear part.
        let view = [1170, 2532];
        let part = [0.0, 0.0, 1170.0, 1832.0];
        let away_m = 1.0 / (2.0 * 0.5f64.tan() * 1170.0 / 2532.0);
        let upright = lay_in(view, clear(view, Some(part)), away_m, 0.0).unwrap();
        assert_eq!(upright.rect, [25, 846, 1120, 840]);
        // The same said nothing of: seven tenths of the width, five a texel.
        let silent = lay(view, away_m, 0.0).unwrap();
        assert_eq!(silent.rect[2..], [800, 600]);
    }

    #[test]
    fn a_clear_part_said_is_kept_inside_the_view_and_one_too_small_is_not_heard() {
        let view = [800, 600];
        assert_eq!(
            clear(view, Some([-5.0, 10.0, 900.0, 500.5])),
            [0.0, 10.0, 800.0, 500.5]
        );
        let silent = clear(view, None);
        assert_eq!(clear(view, Some([10.0, 10.0, 10.5, 500.0])), silent);
        assert_eq!(clear(view, Some([f64::NAN, 0.0, 800.0, 600.0])), silent);
        assert_eq!(clear(view, Some([700.0, 0.0, 100.0, 600.0])), silent);
        // Silent, seven tenths of the view round a middle raised 0.06 of it.
        assert_eq!(
            silent.map(|n| (n * 1e6).round() / 1e6),
            [120.0, 54.0, 680.0, 474.0]
        );
    }
}
