use std::fmt::Display;

use derive_more::{AsRef, Constructor, Deref, DerefMut, From, Into};

/// A Git commit hash.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Into, From, AsRef, Deref, DerefMut, Constructor,
)]
pub struct CommitHash(String);

impl CommitHash {
    /// Get an abbreviated 8-character Git hash.
    pub fn abbrev(&self) -> &str {
        &self.0[..8]
    }
}

impl Display for CommitHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            Display::fmt(&self.0, f)
        } else {
            Display::fmt(self.abbrev(), f)
        }
    }
}
