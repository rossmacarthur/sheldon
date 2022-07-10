//! Match profiles according to configuration

use crate::config::{ExternalPlugin, InlinePlugin};
use crate::Context;

pub trait MatchesProfile {
    fn profiles(&self) -> Option<&[String]>;

    fn matches_profile(&self, ctx: &Context) -> bool {
        match self.profiles() {
            None => true,
            Some(profiles) => match &ctx.profile {
                None => false,
                Some(profile) => profiles.contains(profile),
            },
        }
    }
}

impl MatchesProfile for &ExternalPlugin {
    fn profiles(&self) -> Option<&[String]> {
        self.profiles.as_deref()
    }
}

impl MatchesProfile for &InlinePlugin {
    fn profiles(&self) -> Option<&[String]> {
        self.profiles.as_deref()
    }
}
