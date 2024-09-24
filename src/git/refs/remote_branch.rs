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
        Self(Ref::new(Ref::HEADS.to_owned(), format!("{remote}/{name}")))
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
}
