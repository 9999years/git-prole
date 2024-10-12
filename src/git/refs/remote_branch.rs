use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;

use miette::miette;

use super::LocalBranchRef;
use super::Ref;

/// A Git reference to a remote branch.
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct RemoteBranchRef(Ref);

impl Debug for RemoteBranchRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl PartialEq<Ref> for RemoteBranchRef {
    fn eq(&self, other: &Ref) -> bool {
        self.0.eq(other)
    }
}

impl RemoteBranchRef {
    pub fn new(remote: &str, name: &str) -> Self {
        Self(Ref::new(
            Ref::REMOTES.to_owned(),
            format!("{remote}/{name}"),
        ))
    }

    /// Get the qualified name of this branch, including the remote name.
    pub fn qualified_branch_name(&self) -> &str {
        self.name()
    }

    /// Get the name of this remote and branch.
    pub fn remote_and_branch(&self) -> (&str, &str) {
        self.0
            .name()
            .split_once('/')
            .expect("A remote branch always has a remote and a branch")
    }

    /// Get the name of this remote.
    pub fn remote(&self) -> &str {
        self.remote_and_branch().0
    }

    /// Get the name of this branch.
    pub fn branch_name(&self) -> &str {
        self.remote_and_branch().1
    }

    /// Get a local branch with the same name.
    pub fn as_local(&self) -> LocalBranchRef {
        LocalBranchRef::new(self.branch_name().to_owned())
    }
}

impl Deref for RemoteBranchRef {
    type Target = Ref;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Ref> for RemoteBranchRef {
    type Error = miette::Report;

    fn try_from(value: Ref) -> Result<Self, Self::Error> {
        if value.is_remote_branch() {
            Ok(Self(value))
        } else {
            Err(miette!("Ref is not a remote branch: {value}"))
        }
    }
}

impl Display for RemoteBranchRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn remote_branch_ref_try_from() {
        let branch =
            RemoteBranchRef::try_from(Ref::from_str("refs/remotes/puppy/doggy").unwrap()).unwrap();

        assert_eq!(branch.remote(), "puppy");
        assert_eq!(branch.branch_name(), "doggy");
    }

    #[test]
    fn test_remote_branch_new() {
        assert_eq!(
            RemoteBranchRef::new("origin", "puppy"),
            Ref::from_str("refs/remotes/origin/puppy").unwrap(),
        );
    }

    #[test]
    fn test_remote_branch_qualified_branch_name() {
        assert_eq!(
            RemoteBranchRef::new("origin", "puppy").qualified_branch_name(),
            "origin/puppy",
        );
    }

    #[test]
    fn test_remote_branch_remote_and_branch() {
        assert_eq!(
            RemoteBranchRef::new("origin", "puppy/doggy").remote_and_branch(),
            ("origin", "puppy/doggy"),
        );
    }

    #[test]
    fn test_remote_branch_branch_name() {
        assert_eq!(
            RemoteBranchRef::new("origin", "puppy").branch_name(),
            "puppy",
        );
    }

    #[test]
    fn test_remote_branch_as_local() {
        assert_eq!(
            RemoteBranchRef::new("origin", "puppy").as_local(),
            Ref::from_str("refs/heads/puppy").unwrap(),
        );
    }

    #[test]
    fn test_remote_branch_display() {
        let branch = RemoteBranchRef::new("origin", "puppy");
        assert_eq!(format!("{branch}"), "origin/puppy");
        assert_eq!(format!("{branch:#}"), "refs/remotes/origin/puppy");
    }
}
