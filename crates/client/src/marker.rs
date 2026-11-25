//! Markers: what a plugin or an owner of the core points out somewhere in
//! the world, shown wherever it is.
//!
//! A giver gives its markers as data each turn, as a plugin offers: what
//! each marks, a label, a glyph, and what pressing it offers, each a command
//! of the seam in the giver's words. The core finds each thing every frame
//! and says where it is for the eye: on the screen at a point, or off it at
//! a point of the screen's edge with the angle it lies at, and how far it
//! is along the ground. A front end draws every marker in one manner; an
//! agent reads them and sends their commands itself.

use glam::DVec3;
use serde::{Serialize, Serializer};
use serde_json::Value;

/// How many markers are shown at once, the nearest: and how many one
/// giver gives, its first.
pub const SHOWN: usize = 6;

/// How long a label is at most, bytes: a landmark's name's limit, cut at
/// a character.
pub const LABEL_BYTES: usize = 64;

/// What a giver points out this turn. `thing` and the verbs of its offers
/// are keys of the giver's words, as an offer's are; `glyph` names an icon
/// a front end draws, a pin where it has none of that name.
#[derive(Clone, PartialEq, Debug)]
pub struct Marker {
    /// The giver's own number for it. Under 2^53, so it is whole wherever
    /// JSON is read.
    pub id: u64,
    pub on: Marked,
    /// What sort of marker it is, the word a person hides them by: `pet`,
    /// `home`, `favourite`.
    pub kind: &'static str,
    /// What it is: `horse`, said where the label is empty.
    pub thing: &'static str,
    /// What it is called: a person's text, shown as text, at most
    /// [`LABEL_BYTES`]; empty, the thing is said.
    pub label: String,
    /// The icon it is drawn with: `paw-print`.
    pub glyph: &'static str,
    /// A key of the giver's words said beside the label, how it stands
    /// now: `on_its_way`.
    pub note: Option<&'static str>,
    /// What pressing it offers, in order: a choice whose command is
    /// another plugin's is left out.
    pub choices: Vec<Choice>,
}

/// What a marker marks: found by the core each frame, wherever it is.
#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Marked {
    /// An animal by its number: where it is drawn near, else where the
    /// world keeps it as a pet. One that is neither is not shown.
    Animal(u64),
    /// A point of the world that stays where it is, as an offer's
    /// `On::Point`: where the mark hangs, over what it points out, where
    /// that thing's label hangs when it has one. Said `[x, y, z]`, metres.
    Point(#[serde(serialize_with = "metres")] DVec3),
}

fn metres<S: Serializer>(point: &DVec3, serializer: S) -> Result<S::Ok, S::Error> {
    point.to_array().serialize(serializer)
}

/// One thing pressing a marker offers: the verb, a key of the giver's
/// words, and the seam command it sends, `{"type":"sound","voice":"whistle"}`.
/// One with a `confirm` cannot be undone and is asked first: `confirm` is
/// the key of the giver's words that says plainly what it does, the label
/// in it as `{name}`.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Choice {
    pub verb: &'static str,
    pub command: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm: Option<&'static str>,
}

/// A marker as the seam says it, by who gave it: a plugin's name, or an
/// owner of the core's, `parts`.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Said {
    pub giver: &'static str,
    pub id: u64,
    pub on: Marked,
    pub kind: &'static str,
    pub thing: &'static str,
    pub label: String,
    pub glyph: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<&'static str>,
    pub choices: Vec<Choice>,
}

impl Said {
    pub(crate) fn of(giver: &'static str, marker: &Marker) -> Said {
        Said {
            giver,
            id: marker.id,
            on: marker.on,
            kind: marker.kind,
            thing: marker.thing,
            label: marker.label.clone(),
            glyph: marker.glyph,
            note: marker.note,
            choices: marker.choices.clone(),
        }
    }
}

/// Where a marker is for the eye this frame: `x` and `y` are fractions of
/// the view from its top left, on the screen or on its edge. Off the
/// screen, `edge_deg` is the angle the thing lies at from the middle,
/// clockwise from the screen's up. `tagged` is a thing in view whose
/// nametag already hangs over it: a front end draws nothing more there.
#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
pub struct At {
    pub giver: &'static str,
    pub id: u64,
    pub x: f32,
    pub y: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_deg: Option<f32>,
    pub distance_m: f32,
    pub tagged: bool,
}

impl At {
    /// Whether it moved from `told` past a pixel of a view `width_px` by
    /// `height_px`, came on or off the screen, or changed by a metre.
    fn moved(&self, told: &At, width_px: f32, height_px: f32) -> bool {
        (self.giver, self.id, self.tagged, self.edge_deg.is_some())
            != (told.giver, told.id, told.tagged, told.edge_deg.is_some())
            || ((self.x - told.x) * width_px).abs() >= 1.0
            || ((self.y - told.y) * height_px).abs() >= 1.0
            || (self.distance_m - told.distance_m).abs() >= 1.0
            || self
                .edge_deg
                .zip(told.edge_deg)
                .is_some_and(|(a, b)| (a - b).abs() >= 1.0)
    }
}

/// A point as the eye sees it: across and up the view, each scaled so the
/// view's edge is at `depth`, and `depth` itself, how far ahead of the eye
/// it is. A point behind the eye has a depth below zero.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Seen {
    pub across: f64,
    pub up: f64,
    pub depth: f64,
}

/// Where a point seen so lands: on the screen while it is in view and
/// ahead of `near`, else on the edge of the view along the line from the
/// middle toward it, with the angle of that line on a view `aspect` wide
/// for its height. Behind the eye, how far behind counts as below: what
/// is more behind than above rides the bottom edge, and one turned toward
/// slides along it. A front end lays an edge point in from the edge by its
/// own measure.
pub(crate) fn place(seen: Seen, near: f64, aspect: f64) -> ([f32; 2], Option<f32>) {
    let Seen { across, up, depth } = seen;
    let ahead = depth > near;
    if ahead && across.abs() <= depth && up.abs() <= depth {
        let (x, y) = (across / depth, up / depth);
        return ([((x + 1.0) / 2.0) as f32, ((1.0 - y) / 2.0) as f32], None);
    }
    // The line toward it in the view's plane, never divided by depth, which
    // would turn it over behind the eye; there it leans down by how far
    // behind it is, the same line at the eye's own plane.
    let (mut x, mut y) = (across, up - (-depth).max(0.0));
    if x.hypot(y) < 1e-9 {
        (x, y) = (0.0, -1.0);
    }
    let reach = x.abs().max(y.abs());
    let (x, y) = (x / reach, y / reach);
    // On the screen a unit across is `aspect` units up.
    let angle = libm::atan2(x * aspect, y).to_degrees();
    (
        [((x + 1.0) / 2.0) as f32, ((1.0 - y) / 2.0) as f32],
        Some(angle as f32),
    )
}

/// Whether a command is the giver's to offer: one of the core's, or, with
/// a dot, the giver's own. A plugin never offers another's command, which
/// would be an edge between plugins nobody drew.
fn owns(giver: &str, command: &Value) -> bool {
    let kind = command["type"].as_str().unwrap_or_default();
    kind.split_once('.')
        .is_none_or(|(plugin, _)| plugin == giver)
}

/// A label kept to [`LABEL_BYTES`], cut at a character, its control
/// characters dropped.
pub(crate) fn label(text: &str) -> String {
    let mut kept = String::new();
    for c in text.chars().filter(|c| !c.is_control()) {
        if kept.len() + c.len_utf8() > LABEL_BYTES {
            break;
        }
        kept.push(c);
    }
    kept
}

/// Every giver's markers of its last turn, by its name, the kinds the
/// person hides, and what the seam was last told of them.
#[derive(Default)]
pub(crate) struct Markers {
    given: Vec<(&'static str, Vec<Marker>)>,
    hidden: Vec<String>,
    /// Something given or hidden changed since the seam was last told.
    changed: bool,
    told: Vec<At>,
}

impl Markers {
    /// A giver's markers, in place of those it gave last: its first
    /// [`SHOWN`] of distinct ids, each label kept to its length, each
    /// choice its own or the core's. A plugin gives through `Host::markers`;
    /// an owner of the core calls this with its own name. The same again
    /// changes nothing.
    pub(crate) fn give(&mut self, giver: &'static str, mut markers: Vec<Marker>) {
        let mut ids = Vec::with_capacity(SHOWN);
        markers.retain(|marker| {
            let first = ids.len() < SHOWN && !ids.contains(&marker.id);
            first.then(|| ids.push(marker.id));
            first
        });
        for marker in &mut markers {
            if marker.label.len() > LABEL_BYTES || marker.label.chars().any(char::is_control) {
                marker.label = label(&marker.label);
            }
            marker.choices.retain(|choice| owns(giver, &choice.command));
        }
        let at = self.given.iter().position(|(given, _)| *given == giver);
        if at.map_or(markers.is_empty(), |at| self.given[at].1 == markers) {
            return;
        }
        self.changed = true;
        match (at, markers.is_empty()) {
            (Some(at), false) => self.given[at].1 = markers,
            (Some(at), true) => {
                self.given.remove(at);
            }
            (None, false) => self.given.push((giver, markers)),
            (None, true) => {}
        }
    }

    /// The kinds the person hides, in place of the last: what a front end
    /// keeps for them and hands over, `favourite`.
    pub(crate) fn hide(&mut self, kinds: Vec<String>) {
        self.changed |= self.hidden != kinds;
        self.hidden = kinds;
    }

    /// Every marker given and not hidden, with its giver.
    pub(crate) fn every(&self) -> impl Iterator<Item = (&'static str, &Marker)> {
        self.given
            .iter()
            .flat_map(|(giver, markers)| markers.iter().map(move |marker| (*giver, marker)))
            .filter(|(_, marker)| !self.hidden.iter().any(|kind| kind == marker.kind))
    }

    /// Every marker as the seam says it, when that changed since it was
    /// last said.
    pub(crate) fn resaid(&mut self) -> Option<Vec<Said>> {
        core::mem::take(&mut self.changed).then(|| {
            self.every()
                .map(|(giver, marker)| Said::of(giver, marker))
                .collect()
        })
    }

    /// Where the markers are this frame, the [`SHOWN`] nearest, when one
    /// moved past a pixel since they were last told; nothing while there
    /// is none and none was told.
    pub(crate) fn retold(
        &mut self,
        mut at: Vec<At>,
        width_px: f32,
        height_px: f32,
    ) -> Option<Vec<At>> {
        at.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m));
        at.truncate(SHOWN);
        let moved = at.len() != self.told.len()
            || at
                .iter()
                .zip(&self.told)
                .any(|(now, told)| now.moved(told, width_px, height_px));
        moved.then(|| {
            self.told.clone_from(&at);
            at
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pet(id: u64) -> Marker {
        Marker {
            id,
            on: Marked::Animal(id),
            kind: "pet",
            thing: "horse",
            label: "Star".into(),
            glyph: "paw-print",
            note: None,
            choices: Vec::new(),
        }
    }

    fn at(id: u64, x: f32, distance_m: f32) -> At {
        At {
            giver: "ride",
            id,
            x,
            y: 0.5,
            edge_deg: None,
            distance_m,
            tagged: false,
        }
    }

    #[test]
    fn a_point_in_view_lands_on_the_screen() {
        let seen = Seen {
            across: 0.0,
            up: 0.0,
            depth: 10.0,
        };
        assert_eq!(place(seen, 0.1, 1.5), ([0.5, 0.5], None));
        let corner = Seen {
            across: -10.0,
            up: 10.0,
            depth: 10.0,
        };
        assert_eq!(place(corner, 0.1, 1.5), ([0.0, 0.0], None));
    }

    #[test]
    fn a_point_out_of_view_rides_the_edge_on_its_side() {
        // Off to the right, a little up: on the right edge, turned right.
        let right = Seen {
            across: 30.0,
            up: 3.0,
            depth: 10.0,
        };
        let ([x, y], edge) = place(right, 0.1, 1.0);
        assert_eq!(x, 1.0);
        assert!(y < 0.5 && y > 0.4, "{y}");
        let edge = edge.unwrap();
        assert!((80.0..90.0).contains(&edge), "{edge}");
        // Behind and a little to the left: the bottom, toward the left,
        // never turned over.
        let behind = Seen {
            across: -5.0,
            up: 0.0,
            depth: -20.0,
        };
        let ([x, y], edge) = place(behind, 0.1, 1.0);
        assert_eq!(y, 1.0);
        assert!(x < 0.5 && edge.unwrap() < -90.0, "{x} {edge:?}");
        // Behind and a little above the eye, as a pet thirty metres back:
        // the bottom still, whichever way a slope tips it.
        for up in [-2.0, 2.0] {
            let pet = Seen {
                across: 0.5,
                up,
                depth: -30.0,
            };
            assert_eq!(place(pet, 0.1, 1.0).0[1], 1.0, "{up}");
        }
        // Behind but far more above than behind: the top.
        let high = Seen {
            across: 0.0,
            up: 100.0,
            depth: -30.0,
        };
        assert_eq!(place(high, 0.1, 1.0).0[1], 0.0);
        // Straight behind: the bottom.
        let back = Seen {
            across: 0.0,
            up: 0.0,
            depth: -20.0,
        };
        assert_eq!(place(back, 0.1, 1.0), ([0.5, 1.0], Some(180.0)));
    }

    #[test]
    fn the_edge_angle_is_the_screens_on_a_wide_view() {
        // A diagonal of the view is steeper on the screen than 45 degrees
        // when the view is twice as wide as tall.
        let corner = Seen {
            across: 10.0,
            up: 10.0,
            depth: 1.0,
        };
        let (_, edge) = place(corner, 0.1, 2.0);
        let expected = libm::atan2(2.0, 1.0).to_degrees() as f32;
        assert!((edge.unwrap() - expected).abs() < 1e-4);
    }

    #[test]
    fn a_label_keeps_a_names_length_and_no_control() {
        assert_eq!(label("Star\n"), "Star");
        let long = "é".repeat(LABEL_BYTES);
        let kept = label(&long);
        assert_eq!(kept.len(), LABEL_BYTES);
        assert!(kept.chars().all(|c| c == 'é'));
    }

    #[test]
    fn a_plugin_gives_its_first_few_and_the_seam_hears_changes() {
        let mut markers = Markers::default();
        assert_eq!(markers.resaid(), None, "none given, none said");
        markers.give("ride", (1..=SHOWN as u64 + 3).map(pet).collect());
        assert_eq!(markers.every().count(), SHOWN);
        let said = markers.resaid().unwrap();
        assert_eq!(said[0].giver, "ride");
        assert_eq!(markers.resaid(), None, "unchanged");
        markers.give("ride", (1..=SHOWN as u64 + 3).map(pet).collect());
        assert_eq!(markers.resaid(), None, "the same again");
        markers.give("ride", Vec::new());
        assert_eq!(markers.resaid(), Some(Vec::new()));
        assert_eq!(markers.every().count(), 0);
    }

    #[test]
    fn an_id_is_given_once_and_a_choice_is_the_givers_or_the_cores() {
        let mut markers = Markers::default();
        let choice = |command: Value| Choice {
            verb: "go",
            command,
            confirm: None,
        };
        let mut one = pet(1);
        one.choices = vec![
            choice(serde_json::json!({ "type": "sound", "voice": "whistle" })),
            choice(serde_json::json!({ "type": "ride.release" })),
            choice(serde_json::json!({ "type": "items.take" })),
        ];
        markers.give("ride", vec![one, pet(1), pet(2)]);
        let said = markers.resaid().unwrap();
        assert_eq!(said.iter().map(|said| said.id).collect::<Vec<_>>(), [1, 2]);
        let kinds: Vec<&str> = said[0]
            .choices
            .iter()
            .filter_map(|choice| choice.command["type"].as_str())
            .collect();
        assert_eq!(kinds, ["sound", "ride.release"]);
    }

    #[test]
    fn a_kind_hidden_is_neither_said_nor_shown_until_it_is_shown_again() {
        let mut markers = Markers::default();
        let home = Marker {
            id: 9,
            on: Marked::Point(DVec3::new(1.0, 2.0, 3.0)),
            kind: "home",
            thing: "home",
            label: "Square".into(),
            glyph: "house",
            note: None,
            choices: Vec::new(),
        };
        markers.give("ride", vec![pet(1)]);
        markers.give("parts", vec![home]);
        assert_eq!(markers.resaid().unwrap().len(), 2);
        markers.hide(vec!["pet".into()]);
        let said = markers.resaid().unwrap();
        assert_eq!(
            said.iter()
                .map(|said| (said.giver, said.kind))
                .collect::<Vec<_>>(),
            [("parts", "home")]
        );
        let json = serde_json::to_value(&said[0]).unwrap();
        assert_eq!(json["on"], serde_json::json!({ "point": [1.0, 2.0, 3.0] }));
        markers.hide(Vec::new());
        assert_eq!(markers.resaid().unwrap().len(), 2);
    }

    #[test]
    fn where_they_are_is_told_past_a_pixel_and_the_nearest_first() {
        let mut markers = Markers::default();
        assert_eq!(
            markers.retold(Vec::new(), 1000.0, 500.0),
            None,
            "nothing while none"
        );
        let told = markers.retold(vec![at(1, 0.5, 900.0), at(2, 0.2, 40.0)], 1000.0, 500.0);
        assert_eq!(
            told.unwrap().iter().map(|at| at.id).collect::<Vec<_>>(),
            [2, 1]
        );
        let still = vec![at(1, 0.5004, 900.2), at(2, 0.2, 40.0)];
        assert_eq!(markers.retold(still, 1000.0, 500.0), None, "under a pixel");
        let moved = vec![at(1, 0.502, 900.0), at(2, 0.2, 40.0)];
        assert!(markers.retold(moved, 1000.0, 500.0).is_some());
        let many = (0..10).map(|id| at(id, 0.5, id as f32)).collect();
        assert_eq!(markers.retold(many, 1000.0, 500.0).unwrap().len(), SHOWN);
        assert_eq!(
            markers.retold(Vec::new(), 1000.0, 500.0),
            Some(Vec::new()),
            "once empty"
        );
        assert_eq!(markers.retold(Vec::new(), 1000.0, 500.0), None);
    }
}
