//! A screen the eye is held on, laid over the view as drawn
//! ([`scene::Overlay`]): while the controls are lent to a game, its picture
//! is the person's screen, not a thing in the world. It is drawn after the
//! compositor, in the view's own pixels, each texel the same whole number
//! of them: the largest that fits inside where the world's screen stands
//! on the view, and inside the view, one at least.

use glam::{DAffine3, DVec3};
use scene::{Camera, Overlay, PictureId};

/// The overlay of a picture `texels` wide and tall shown on the unit quad
/// `quad` (world from quad, `-0.5..=0.5` across `x` and `y`), as `camera`
/// sees it on a view `view` pixels: centred where the quad's middle
/// lands, moved inside the view where it would cross its edge. `None` for
/// a quad behind the eye, or a picture larger than the view.
pub(crate) fn laid(
    camera: &Camera,
    view: [u32; 2],
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
    // The largest whole scale inside the quad as seen, and inside the view.
    let fits = |pixels: f64, texels: u32| (pixels / f64::from(texels)).floor().max(0.0) as u32;
    let scale = fits(high[0] - low[0], texels_wide)
        .min(fits(high[1] - low[1], texels_tall))
        .min(view_wide / texels_wide)
        .min(view_tall / texels_tall)
        .max(1);
    let (wide, tall) = (texels_wide * scale, texels_tall * scale);
    if wide > view_wide || tall > view_tall {
        return None;
    }
    let corner = |middle: f64, size: u32, room: u32| {
        (middle - f64::from(size) / 2.0)
            .round()
            .clamp(0.0, f64::from(room - size)) as u32
    };
    Some(Overlay {
        picture,
        rect: [
            corner((low[0] + high[0]) / 2.0, wide, view_wide),
            corner((low[1] + high[1]) / 2.0, tall, view_tall),
            wide,
            tall,
        ],
        opacity,
    })
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
    /// `view` pixels by an eye a radian tall.
    fn lay(view: [u32; 2], away_m: f64, up_m: f64) -> Option<Overlay> {
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
        laid(&camera, view, quad, PictureId(3), (160, 120), 1.0)
    }

    /// How far ahead the quad fills seven tenths of the view's height, as
    /// a cabinet's screen does when played.
    fn played_m() -> f64 {
        0.75 / (2.0 * 0.7 * 0.5f64.tan())
    }

    #[test]
    fn the_largest_whole_scale_inside_the_screen_as_seen() {
        // 504 pixels tall of 720: four a texel, centred.
        let wide = lay([1280, 720], played_m(), 0.0).unwrap();
        assert_eq!(wide.rect, [320, 120, 640, 480]);
        assert_eq!(wide.picture, PictureId(3));
        // A phone held sideways, 2532 by 1170 pixels: six a texel.
        let phone = lay([2532, 1170], played_m(), 0.0).unwrap();
        assert_eq!(phone.rect[2..], [960, 720]);
        // A small window, 800 by 450: two.
        let small = lay([800, 450], played_m(), 0.0).unwrap();
        assert_eq!(small.rect[2..], [320, 240]);
    }

    #[test]
    fn a_screen_smaller_than_its_picture_is_laid_at_one_pixel_a_texel() {
        let far = lay([1280, 720], 20.0, 0.0).unwrap();
        assert_eq!(far.rect, [560, 300, 160, 120]);
    }

    #[test]
    fn it_stays_inside_the_view() {
        // A screen whose middle is near the view's top edge.
        let high = lay([1280, 720], 2.0, 1.0).unwrap();
        let [x, y, wide, tall] = high.rect;
        assert!(x + wide <= 1280 && y + tall <= 720);
        assert_eq!(y, 0);
        // A view smaller than the picture lays nothing; nor does a screen
        // behind the eye.
        assert!(lay([100, 100], 2.0, 0.0).is_none());
        assert!(lay([1280, 720], -2.0, 0.0).is_none());
    }
}
