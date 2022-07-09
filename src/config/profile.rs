//! Match profiles according to configuration

use crate::config::{ExternalPlugin, InlinePlugin};
use crate::Context;

pub trait MatchesProfile {
    fn profiles(&self) -> &Option<Vec<String>>;

    fn matches_profile(&self, ctx: &Context) -> bool {
        match self.profiles() {
            None => true,
            Some(ref profiles) => match ctx.profile {
                None => false,
                Some(ref profile) => profiles.contains(profile),
            },
        }
    }
}

impl MatchesProfile for &ExternalPlugin {
    fn profiles(&self) -> &Option<Vec<String>> {
        &self.profiles
    }
}

impl MatchesProfile for &InlinePlugin {
    fn profiles(&self) -> &Option<Vec<String>> {
        &self.profiles
    }
}
