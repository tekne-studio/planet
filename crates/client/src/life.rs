//! Whether the world this client is in has life: a system of the core a
//! world's admin switches, said as the line `gaia` of the world's
//! statement. What lives here asks this one place, and life is on only at
//! the version of `gaia` this client carries: a client of another version
//! draws no life rather than a different one.

use crate::seam::PluginOn;

/// Whether life is on in the world this client is in.
#[derive(Default)]
pub(crate) struct Life {
    on: bool,
}

impl Life {
    /// Takes the world's statement: life is on when it says `gaia` at this
    /// client's version. A line for the log when it speaks another.
    pub(crate) fn hear(&mut self, spoken: &[protocol::Plugin]) -> Option<String> {
        let said = spoken.iter().find(|said| said.name == gaia::NAME);
        self.on = said.is_some_and(|said| said.version == gaia::VERSION);
        said.filter(|_| !self.on).map(|said| {
            format!(
                "the world speaks {} {}, this client {}",
                said.name,
                said.version,
                gaia::VERSION
            )
        })
    }

    /// Stands in for a world's word: life is on.
    pub(crate) fn rehearse(&mut self) {
        self.on = true;
    }

    /// No world has spoken: life is off.
    pub(crate) fn hush(&mut self) {
        self.on = false;
    }

    pub(crate) fn on(&self) -> bool {
        self.on
    }

    /// Its line of what is on, while it is: what a front end shows beside
    /// the plugins.
    pub(crate) fn line(&self) -> Option<PluginOn> {
        self.on.then(|| PluginOn {
            name: gaia::NAME.to_owned(),
            version: gaia::VERSION,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(version: u32) -> Vec<protocol::Plugin> {
        vec![protocol::Plugin {
            name: gaia::NAME.to_owned(),
            version,
        }]
    }

    #[test]
    fn life_is_on_only_at_this_version() {
        let mut life = Life::default();
        assert!(!life.on(), "no world has spoken");
        assert_eq!(life.hear(&said(gaia::VERSION)), None);
        assert!(life.on() && life.line().is_some());
        let lifeless = life.hear(&said(gaia::VERSION + 1));
        assert!(!life.on() && life.line().is_none());
        assert!(lifeless.is_some_and(|line| line.contains("gaia")));
        life.rehearse();
        assert!(life.on());
        assert_eq!(
            life.hear(&[]),
            None,
            "a world without life says nothing of it"
        );
        assert!(!life.on());
    }
}
