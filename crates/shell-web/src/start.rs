//! How the page asks the engine to start, and what the engine says of each
//! step of its start: plain data, so a machine that stops at one step can
//! be read from a photo of its screen.

/// The page's start, read once, before a device exists. A field left out
/// takes its default: the fastest adapter, WebGPU where the browser has
/// it, no paint between steps, the high picture.
#[derive(Debug, Default, PartialEq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Start {
    pub power: Power,
    pub backend: Backend,
    /// Whether the page is given a paint after each step said, so a start
    /// that stops shows the step it stopped at; a frame's wait a step.
    pub paint: bool,
    /// What the first frame is drawn with, before any `set_effects`.
    pub effects: Option<client::Effects>,
}

impl Start {
    /// Empty is every default.
    pub fn read(json: &str) -> Result<Start, String> {
        if json.trim().is_empty() {
            return Ok(Start::default());
        }
        serde_json::from_str(json).map_err(|error| format!("start: {error}"))
    }
}

/// Which adapter the browser is asked for, on a machine with two.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Power {
    #[default]
    High,
    Low,
}

/// The road to the GPU: asked of the engine, and said of the adapter given.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// WebGPU where the browser has it, WebGL2 otherwise.
    #[default]
    WebGpu,
    /// WebGL2, even where WebGPU is offered.
    WebGl2,
}

/// A step of the start, once reached.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(tag = "step", rename_all = "lowercase")]
pub enum Step {
    /// The adapter the browser gave: its name as the browser says it
    /// (empty where it hides it), the road and the limits that matter.
    Adapter {
        name: String,
        backend: Backend,
        /// The largest side of a 2D texture, texels.
        texture_px: u32,
        /// The largest buffer, MiB.
        buffer_mib: u64,
    },
    /// The device opened, and what the browser says of the adapter once
    /// one is open: vendor, architecture, device and description, where
    /// WebGPU tells them.
    Device { about: Vec<String> },
    /// Every pipeline of the renderer made.
    Pipelines,
    /// The first frame handed to the GPU, at the canvas's size, pixels.
    Frame { width: u32, height: u32, ratio: f64 },
    /// The GPU done with the first frame.
    Drawn,
}

impl Step {
    /// The `start` event, `ms` on the page's clock.
    pub fn said(&self, ms: f64) -> String {
        let mut value = serde_json::to_value(self).expect("a step is plain data");
        let object = value.as_object_mut().expect("a step is an object");
        object.insert("type".into(), "start".into());
        object.insert("ms".into(), (ms.max(0.0).round()).into());
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_start_is_every_default() {
        assert_eq!(Start::read("").unwrap(), Start::default());
        assert_eq!(Start::read("{}").unwrap(), Start::default());
        let start = Start::default();
        assert_eq!((start.power, start.backend), (Power::High, Backend::WebGpu));
        assert!(!start.paint && start.effects.is_none());
    }

    #[test]
    fn a_start_reads_the_names_the_page_writes() {
        let start = Start::read(
            r#"{"power":"low","backend":"webgl2","paint":true,"effects":{"trees":false}}"#,
        )
        .unwrap();
        assert_eq!((start.power, start.backend), (Power::Low, Backend::WebGl2));
        assert!(start.paint);
        let effects = start.effects.unwrap();
        assert!(!effects.trees);
        // What the page left out is the high picture's.
        assert!(effects.animals && effects.shadows);
    }

    /// The page writes these names from its own lists
    /// (`apps/web/src/lib/engine/start.ts`): this holds the two equal.
    #[test]
    fn the_page_names_the_powers_roads_and_steps_the_engine_does() {
        const PAGE: &str = include_str!("../../../apps/web/src/lib/engine/start.ts");
        let listed = |name: &str| -> Vec<String> {
            let from = PAGE
                .find(&format!("export const {name} = ["))
                .unwrap_or_else(|| panic!("start.ts lists no `{name}`"));
            let list = &PAGE[from..];
            let list = &list[list.find('[').unwrap() + 1..list.find(']').unwrap()];
            list.split(',')
                .map(|item| item.trim().trim_matches('\'').to_owned())
                .filter(|item| !item.is_empty())
                .collect()
        };
        let said = |value: serde_json::Value| value.as_str().unwrap().to_owned();
        // Each list by a match the compiler holds whole: a new variant fails
        // here until it is named, and then until the page names it too.
        let powers = [Power::High, Power::Low].map(|power| match power {
            Power::High | Power::Low => said(serde_json::to_value(power).unwrap()),
        });
        let roads = [Backend::WebGpu, Backend::WebGl2].map(|road| match road {
            Backend::WebGpu | Backend::WebGl2 => said(serde_json::to_value(road).unwrap()),
        });
        let steps = [
            Step::Adapter {
                name: String::new(),
                backend: Backend::WebGpu,
                texture_px: 0,
                buffer_mib: 0,
            },
            Step::Device { about: Vec::new() },
            Step::Pipelines,
            Step::Frame {
                width: 0,
                height: 0,
                ratio: 1.0,
            },
            Step::Drawn,
        ]
        .map(|step| match step {
            Step::Adapter { .. }
            | Step::Device { .. }
            | Step::Pipelines
            | Step::Frame { .. }
            | Step::Drawn => said(serde_json::to_value(&step).unwrap()["step"].clone()),
        });
        assert_eq!(listed("powers"), powers);
        assert_eq!(listed("roads"), roads);
        // The page's steps hold the engine's between its own two: the module
        // loaded before, the world shown after.
        let page = listed("steps");
        let [first, engine @ .., last] = page.as_slice() else {
            panic!("start.ts lists fewer than two steps");
        };
        assert_eq!((first.as_str(), last.as_str()), ("module", "world"));
        assert_eq!(engine, steps);
    }

    #[test]
    fn a_start_the_engine_cannot_read_is_refused() {
        assert!(Start::read(r#"{"power":"lowest"}"#).is_err());
        assert!(Start::read(r#"{"backend":"webgl"}"#).is_err());
        assert!(Start::read(r#"{"safe":true}"#).is_err());
        assert!(Start::read("{").is_err());
    }

    #[test]
    fn a_step_says_its_name_and_time() {
        let said: serde_json::Value = serde_json::from_str(
            &Step::Adapter {
                name: "a GPU".into(),
                backend: Backend::WebGl2,
                texture_px: 8192,
                buffer_mib: 256,
            }
            .said(12.4),
        )
        .unwrap();
        assert_eq!(said["type"], "start");
        assert_eq!(said["step"], "adapter");
        assert_eq!(said["ms"], 12.0);
        assert_eq!(said["texture_px"], 8192);
        assert_eq!(said["backend"], "webgl2");
        let said: serde_json::Value = serde_json::from_str(&Step::Drawn.said(40.0)).unwrap();
        assert_eq!(
            (&said["type"], &said["step"]),
            (&"start".into(), &"drawn".into())
        );
    }
}
