use std::fmt::Debug;
use std::ops::Deref;
use std::ops::DerefMut;

use camino::Utf8Path;

use crate::config::Config;
use crate::git::Git;
use crate::git::GitLike;

/// A [`Git`] with borrowed [`Config`].
#[derive(Clone)]
pub struct AppGit<'a, C> {
    pub git: Git<C>,
    pub config: &'a Config,
}

impl<C> Debug for AppGit<'_, C>
where
    C: AsRef<Utf8Path>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AppGit")
            .field(&self.git.get_current_dir().as_ref())
            .finish()
    }
}

impl<C> Deref for AppGit<'_, C> {
    type Target = Git<C>;

    fn deref(&self) -> &Self::Target {
        &self.git
    }
}

impl<C> DerefMut for AppGit<'_, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.git
    }
}

impl<C> AsRef<Git<C>> for AppGit<'_, C> {
    fn as_ref(&self) -> &Git<C> {
        &self.git
    }
}

impl<C> AsRef<Config> for AppGit<'_, C> {
    fn as_ref(&self) -> &Config {
        self.config
    }
}

impl<C> AsRef<Utf8Path> for AppGit<'_, C>
where
    C: AsRef<Utf8Path>,
{
    fn as_ref(&self) -> &Utf8Path {
        self.git.as_ref()
    }
}

impl<C> GitLike for AppGit<'_, C>
where
    C: AsRef<Utf8Path>,
{
    type CurrentDir = C;

    fn as_git(&self) -> &Git<Self::CurrentDir> {
        &self.git
    }
}

impl<'a, C> AppGit<'a, C>
where
    C: AsRef<Utf8Path>,
{
    pub fn with_current_dir<C2>(&self, path: C2) -> AppGit<'a, C2> {
        AppGit {
            git: self.git.with_current_dir(path),
            config: self.config,
        }
    }
}
