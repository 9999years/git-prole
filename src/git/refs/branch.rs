use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

use miette::miette;

use super::LocalBranchRef;
use super::Ref;
use super::RemoteBranchRef;

/// A Git reference to a remote branch.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum BranchRef {
    /// A local branch.
    Local(LocalBranchRef),
    /// A remote-tracking branch.
    Remote(RemoteBranchRef),
}

impl BranchRef {
    /// Get the qualified name of this branch.
    pub fn qualified_branch_name(&self) -> &str {
        match &self {
            BranchRef::Local(ref_name) => ref_name.branch_name(),
            BranchRef::Remote(ref_name) => ref_name.name(),
        }
    }

    /// Get the name of this branch.
    pub fn branch_name(&self) -> &str {
        match &self {
            BranchRef::Local(ref_name) => ref_name.branch_name(),
            BranchRef::Remote(ref_name) => ref_name.branch_name(),
        }
    }

    pub fn as_local(&self) -> LocalBranchRef {
        match self {
            BranchRef::Local(local) => local.clone(),
            BranchRef::Remote(remote) => remote.as_local(),
        }
    }
}

impl PartialEq<Ref> for BranchRef {
    fn eq(&self, other: &Ref) -> bool {
        self.deref().eq(other)
    }
}

impl Display for BranchRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchRef::Local(ref_name) => Display::fmt(ref_name, f),
            BranchRef::Remote(ref_name) => Display::fmt(ref_name, f),
        }
    }
}

impl Deref for BranchRef {
    type Target = Ref;

    fn deref(&self) -> &Self::Target {
        match self {
            BranchRef::Local(ref_name) => ref_name.deref(),
            BranchRef::Remote(ref_name) => ref_name.deref(),
        }
    }
}

impl TryFrom<Ref> for BranchRef {
    type Error = miette::Report;

    fn try_from(value: Ref) -> Result<Self, Self::Error> {
        match value.kind() {
            Ref::HEADS => Ok(Self::Local(LocalBranchRef::try_from(value)?)),
            Ref::REMOTES => Ok(Self::Remote(RemoteBranchRef::try_from(value)?)),
            _ => Err(miette!("Ref is not a local or remote branch: {value}")),
        }
    }
}

impl FromStr for BranchRef {
    type Err = miette::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ref::from_str(s)?.try_into()
    }
}

impl From<LocalBranchRef> for BranchRef {
    fn from(value: LocalBranchRef) -> Self {
        Self::Local(value)
    }
}

impl From<RemoteBranchRef> for BranchRef {
    fn from(value: RemoteBranchRef) -> Self {
        Self::Remote(value)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_branch_ref_try_from() {
        let branch = BranchRef::try_from(Ref::from_str("refs/heads/puppy/doggy").unwrap()).unwrap();

        assert_eq!(branch.branch_name(), "puppy/doggy",);
        assert_eq!(branch.qualified_branch_name(), "puppy/doggy",);

        let branch =
            BranchRef::try_from(Ref::from_str("refs/remotes/puppy/doggy").unwrap()).unwrap();

        assert_eq!(branch.branch_name(), "doggy",);
        assert_eq!(branch.qualified_branch_name(), "puppy/doggy",);

        assert!(BranchRef::try_from(Ref::from_str("refs/tags/v1.0.0").unwrap()).is_err());
    }

    #[test]
    fn test_branch_qualified_branch_name() {
        assert_eq!(
            BranchRef::Remote(RemoteBranchRef::new("origin", "puppy")).qualified_branch_name(),
            "origin/puppy",
        );

        assert_eq!(
            BranchRef::Local(LocalBranchRef::new("puppy".into())).qualified_branch_name(),
            "puppy",
        );
    }

    #[test]
    fn test_branch_branch_name() {
        assert_eq!(
            BranchRef::Remote(RemoteBranchRef::new("origin", "puppy")).branch_name(),
            "puppy",
        );

        assert_eq!(
            BranchRef::Local(LocalBranchRef::new("puppy".into())).branch_name(),
            "puppy",
        );
    }
}
