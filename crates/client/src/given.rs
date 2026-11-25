//! What plugins give the core as data, once, as each is plugged in: ways of
//! moving, things for the hand. Each is known by its full name, the
//! plugin's and its own, `<plugin>.<name>`, which a stance carries, and only
//! while its plugin is on.

use crate::seam::PluginOn;

/// A kind of gift a plugin gives.
pub(crate) trait Gift {
    /// What a gift of the kind is called, as a refusal says it.
    const NOUN: &'static str;
    /// The most bytes its full name takes: what a stance carries, as the
    /// schema says it to the world too.
    const BYTES: usize;
    /// Its name in the plugin, the word after the dot.
    fn name(&self) -> &'static str;
    /// What is wrong with it, said after its full name; None when whole.
    fn wrong(&self) -> Option<String>;
}

/// A full name split: the plugin's, and the gift's in it.
pub(crate) fn split(full: &str) -> Option<(&str, &str)> {
    full.split_once('.')
}

/// The full name of a plugin's gift.
pub(crate) fn full(plugin: &str, name: &str) -> String {
    format!("{plugin}.{name}")
}

/// A gift, the plugin that gave it, and whether that plugin is on.
struct Entry<T> {
    plugin: &'static str,
    gift: T,
    on: bool,
}

/// Every gift of one kind the plugins plugged in gave.
pub(crate) struct Given<T> {
    entries: Vec<Entry<T>>,
}

impl<T> Default for Given<T> {
    fn default() -> Self {
        Given {
            entries: Vec::new(),
        }
    }
}

impl<T: Gift> Given<T> {
    /// Takes the gifts a plugin gives, as it is plugged in. They are off
    /// until a world says the plugin is on. One whose full name is longer
    /// than a stance carries, or that is wrong, is refused: what is wrong
    /// with each, said.
    pub(crate) fn give(&mut self, plugin: &'static str, gifts: Vec<T>) -> Vec<String> {
        let mut refused = Vec::new();
        for gift in gifts {
            let name = gift.name();
            if plugin.len() + 1 + name.len() > T::BYTES {
                refused.push(format!(
                    "the {} `{plugin}.{name}` is longer than the {} bytes a stance carries",
                    T::NOUN,
                    T::BYTES
                ));
            } else if let Some(wrong) = gift.wrong() {
                refused.push(format!("the {} `{plugin}.{name}` {wrong}", T::NOUN));
            } else {
                self.entries.push(Entry {
                    plugin,
                    gift,
                    on: false,
                });
            }
        }
        refused
    }

    /// The plugins that are on now: their gifts are, and every other is off.
    pub(crate) fn switch(&mut self, on: &[PluginOn]) {
        for entry in &mut self.entries {
            entry.on = on.iter().any(|plugin| plugin.name == entry.plugin);
        }
    }

    /// A gift that is on, by its full name: the plugin that gave it, and
    /// the gift.
    pub(crate) fn get(&self, full: &str) -> Option<(&'static str, &T)> {
        let (plugin, name) = split(full)?;
        self.entries
            .iter()
            .find(|entry| entry.on && entry.plugin == plugin && entry.gift.name() == name)
            .map(|entry| (entry.plugin, &entry.gift))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Pebble(&'static str);

    impl Gift for Pebble {
        const NOUN: &'static str = "pebble";
        const BYTES: usize = 12;
        fn name(&self) -> &'static str {
            self.0
        }
        fn wrong(&self) -> Option<String> {
            (self.0 == "sand").then(|| "is too small".into())
        }
    }

    #[test]
    fn a_gift_is_on_while_its_plugin_is_and_refused_too_long_or_wrong() {
        let mut given = Given::default();
        let refused = given.give(
            "beach",
            vec![Pebble("grey"), Pebble("enormous"), Pebble("sand")],
        );
        assert_eq!(
            refused,
            [
                "the pebble `beach.enormous` is longer than the 12 bytes a stance carries",
                "the pebble `beach.sand` is too small",
            ]
        );
        assert!(given.get("beach.grey").is_none());
        given.switch(&[PluginOn {
            name: "beach".into(),
            version: 1,
        }]);
        assert_eq!(
            given.get("beach.grey").map(|(plugin, _)| plugin),
            Some("beach")
        );
        assert!(given.get("beach.white").is_none());
        assert!(given.get("grey").is_none());
        given.switch(&[]);
        assert!(given.get("beach.grey").is_none());
    }
}
