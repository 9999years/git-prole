use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;

use miette::miette;

use super::Ref;
use super::RemoteBranchRef;

/// A Git reference to a local branch.
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct LocalBranchRef(Ref);

impl Debug for LocalBranchRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl PartialEq<Ref> for LocalBranchRef {
    fn eq(&self, other: &Ref) -> bool {
        self.0.eq(other)
    }
}

impl LocalBranchRef {
    pub fn new(name: String) -> Self {
        Self(Ref::new(Ref::HEADS.to_owned(), name))
    }

    /// Get the name of this local branch.
    pub fn branch_name(&self) -> &str {
        self.0.name()
    }

    pub fn on_remote(&self, remote: &str) -> RemoteBranchRef {
        RemoteBranchRef::new(remote, self.branch_name())
    }
}

impl Deref for LocalBranchRef {
    type Target = Ref;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Ref> for LocalBranchRef {
    type Error = miette::Report;

    fn try_from(value: Ref) -> Result<Self, Self::Error> {
        if value.is_local_branch() {
            Ok(Self(value))
        } else {
            Err(miette!("Ref is not a local branch: {value}"))
        }
    }
}

impl<S> From<S> for LocalBranchRef
where
    S: AsRef<str>,
{
    fn from(value: S) -> Self {
        Self::new(value.as_ref().to_owned())
    }
}

impl Display for LocalBranchRef {
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
    fn local_branch_ref_try_from() {
        let branch =
            LocalBranchRef::try_from(Ref::from_str("refs/heads/puppy/doggy").unwrap()).unwrap();

        assert_eq!(branch.branch_name(), "puppy/doggy");
    }

    #[test]
    fn local_branch_ref_from_str() {
        let branch = LocalBranchRef::from("puppy");

        assert_eq!(branch, Ref::from_str("refs/heads/puppy").unwrap());
    }

    #[test]
    fn test_local_branch_new() {
        assert_eq!(
            LocalBranchRef::new("puppy".into()),
            Ref::from_str("refs/heads/puppy").unwrap(),
        );
    }

    #[test]
    fn test_local_branch_branch_name() {
        assert_eq!(LocalBranchRef::new("puppy".into()).branch_name(), "puppy",);
    }

    #[test]
    fn test_local_branch_on_remote() {
        assert_eq!(
            LocalBranchRef::new("puppy".into()).on_remote("origin"),
            Ref::from_str("refs/remotes/origin/puppy").unwrap(),
        );
    }

    #[test]
    fn test_remote_branch_display() {
        let branch = LocalBranchRef::new("puppy".into());
        assert_eq!(format!("{branch}"), "puppy");
        assert_eq!(format!("{branch:#}"), "refs/heads/puppy");
    }
}
