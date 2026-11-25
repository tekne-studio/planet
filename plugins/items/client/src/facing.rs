//! Where a landmark faces: an arrow lying on the ground from its star's
//! point, the way a body arriving at the star faces (`Host::facing`, what
//! the place at it carries), shown while the star is picked and turning
//! with it as it turns. A mark over everything, as the gizmo's handles are,
//! in white light edged in black so it reads on grass, snow and night.
//! Only a builder picks, so only a builder sees it.

use client::{Eye, Host, Mark, PartId};
use glam::{DAffine3, DMat3, DQuat, DVec3};
use voxel::{Edge, Finish, Paint};

/// How long the arrow is, of the star's reach along it, and the least it
/// is, metres: a star a metre across shows one that reaches past a body
/// standing on it, seen from behind.
const LENGTH: f64 = 1.5;
const LEAST_M: f64 = 1.5;
/// How wide its bars are, of its length.
const WIDE: f64 = 0.12;
/// The least they are, of the eye's view, so a far star's arrow is still
/// a line on a small screen: about four pixels on a phone held upright.
const WIDE_OF_VIEW: f64 = 0.004;
/// Each side of its head: how long, of its length, and how far it opens
/// from the shaft.
const HEAD: f64 = 0.45;
const HEAD_OPEN_DEG: f64 = 40.0;
/// How thick it is, of its width: flat on the ground.
const FLAT: f64 = 0.35;
/// White light, edged in black.
const PAINT: u8 = Paint {
    color: 0,
    finish: Finish::Light,
    edge: Edge::Black,
}
.byte();

/// The arrow over the part of that name, when it faces somewhere: a shaft
/// from its point along the way one arriving faces, and a head.
pub(crate) fn arrow(host: &Host<'_>, id: PartId, eye: &Eye) -> Vec<Mark> {
    let (Some(facing), Some(world)) = (host.facing(id), host.part_transform(id)) else {
        return Vec::new();
    };
    let (along, up) = (facing.along, facing.up);
    let centre = world.transform_point3(DVec3::new(0.0, 0.0, 0.5));
    let flat = |axis: DVec3| axis - up * up.dot(axis);
    let reach = flat(world.y_axis).length() / 2.0;
    let length = (reach * LENGTH).max(LEAST_M);
    let wide = (length * WIDE).max((centre - eye.position).length() * eye.fov_y * WIDE_OF_VIEW);
    let start = centre + along * reach;
    let tip = start + along * length;
    let back = |deg: f64| DQuat::from_axis_angle(up, deg.to_radians()) * -along;
    vec![
        bar(start, along, up, length, wide),
        bar(tip, back(HEAD_OPEN_DEG), up, length * HEAD, wide),
        bar(tip, back(-HEAD_OPEN_DEG), up, length * HEAD, wide),
    ]
}

/// A flat bar on the ground from a point along a way, unit, as a mark: a
/// mark's cube runs along its own `z` from its foot.
fn bar(from: DVec3, way: DVec3, up: DVec3, long: f64, wide: f64) -> Mark {
    let across = up.cross(way);
    let frame = DMat3::from_cols(across * wide, up * wide * FLAT, way * long);
    Mark {
        paint: PAINT,
        transform: DAffine3::from_mat3_translation(frame, from),
    }
}

#[cfg(test)]
mod tests {
    use client::{Client, Eye, Input, PartId, Shape};
    use glam::{DQuat, DVec3};
    use scene::ModelPass;

    use super::{LEAST_M, WIDE_OF_VIEW, arrow};
    use crate::tests::{client, update};

    /// The marks drawn over the world this frame.
    fn over(client: &mut Client) -> usize {
        let frame = client.update(1.0 / 60.0, &mut Input::default());
        frame
            .models
            .iter()
            .filter(|model| model.pass == ModelPass::Over)
            .count()
    }

    fn eye(at: DVec3) -> Eye {
        Eye {
            position: at,
            rotation: DQuat::IDENTITY,
            fov_y: 1.0,
            aspect: 1.0,
            clear: [0.0, 0.0, 1.0, 1.0],
            pointer: [0.5, 0.5],
        }
    }

    fn landmark(client: &Client) -> PartId {
        client
            .parts()
            .all()
            .find(|(_, _, part)| matches!(part.shape, Shape::Landmark(_)))
            .map(|(id, ..)| id)
            .unwrap()
    }

    #[test]
    fn a_star_picked_shows_where_one_arriving_faces_and_it_turns_with_it() {
        let mut client = client();
        client.command_json(r#"{"type":"items.take","item":"cube","ahead":true}"#);
        let cube_marks = over(&mut client);
        client.command_json(r#"{"type":"items.take","item":"landmark","ahead":true}"#);
        update(&mut client, &mut Input::default());
        assert_eq!(
            over(&mut client),
            cube_marks + 3,
            "the handles and an arrow"
        );
        let id = landmark(&client);
        client.command_json(r#"{"type":"items.set","rotation":[0,0,0]}"#);

        let host = client.host("items");
        let facing = host.facing(id).unwrap();
        let world = host.part_transform(id).unwrap();
        let centre = world.transform_point3(DVec3::new(0.0, 0.0, 0.5));
        let near = arrow(&host, id, &eye(centre + facing.up * 3.0));
        let [shaft, ..] = near.as_slice() else {
            panic!("{near:?}")
        };
        // The shaft runs from past the star's point the way one arriving
        // faces, flat, no shorter than the least.
        let way = shaft.transform.matrix3.z_axis;
        assert!(way.normalize().dot(facing.along) > 1.0 - 1e-9);
        assert!(way.length() >= LEAST_M);
        let from = shaft.transform.translation - centre;
        assert!(from.normalize().dot(facing.along) > 1.0 - 1e-9);
        assert!(from.length() > 0.0);
        // Far off it keeps a width the eye can see.
        let far = arrow(&host, id, &eye(centre + facing.up * 1000.0));
        let wide = far[0].transform.matrix3.x_axis.length();
        assert!(wide >= 1000.0 * WIDE_OF_VIEW * 0.99, "{wide}");

        // Turned a quarter, the arrow turns a quarter with it, the way the
        // star turns.
        client.command_json(r#"{"type":"items.set","rotation":[0,0,90]}"#);
        let host = client.host("items");
        let turned = host.facing(id).unwrap();
        assert!(turned.along.dot(facing.along).abs() < 1e-6);
        assert!(facing.along.cross(turned.along).dot(facing.up) > 0.99);
        let shaft = arrow(&host, id, &eye(centre + facing.up * 3.0))[0];
        assert!(shaft.transform.matrix3.z_axis.normalize().dot(turned.along) > 1.0 - 1e-9);
    }

    #[test]
    fn a_star_laid_on_its_back_points_nowhere_and_shows_no_arrow() {
        let mut client = client();
        client.command_json(r#"{"type":"items.take","item":"landmark","ahead":true}"#);
        client.command_json(r#"{"type":"items.set","rotation":[90,0,0]}"#);
        let id = landmark(&client);
        let host = client.host("items");
        assert_eq!(host.facing(id), None);
        assert!(arrow(&host, id, &eye(DVec3::ZERO)).is_empty());
    }

    #[test]
    fn a_star_placed_ahead_points_the_way_its_builder_looks() {
        let mut client = client();
        let looks = client.here().bearing_deg.unwrap();
        client.command_json(r#"{"type":"items.take","item":"landmark","ahead":true}"#);
        let id = landmark(&client);
        let points = client
            .host("items")
            .facing(id)
            .unwrap()
            .bearing_deg()
            .unwrap();
        let off = (points - looks + 540.0).rem_euclid(360.0) - 180.0;
        assert!(off.abs() < 1.0, "looks {looks}, points {points}");
    }
}
