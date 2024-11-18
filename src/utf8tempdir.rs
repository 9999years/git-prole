use std::ops::Deref;
use std::path::Path;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use miette::IntoDiagnostic;
use tempfile::TempDir;

#[derive(Debug)]
pub struct Utf8TempDir {
    #[allow(dead_code)]
    inner: Option<TempDir>,
    path: Utf8PathBuf,
}

impl Utf8TempDir {
    pub fn new(parent_dir: &Utf8PathBuf) -> miette::Result<Self> {
        let inner = tempfile::tempdir_in(parent_dir).into_diagnostic()?;
        let path = inner.path().to_owned().try_into().into_diagnostic()?;
        Ok(Self {
            inner: Some(inner),
            path,
        })
    }

    /// Keep this directory when it goes out of scope, without changing its type.
    pub fn persist(&mut self) {
        let inner = self.inner.take();
        if let Some(tempdir) = inner {
            let _ = tempdir.into_path();
        }
    }

    /// Keep this directory when it goes out of scope.
    pub fn into_path(mut self) -> Utf8PathBuf {
        self.persist();
        self.path
    }

    #[expect(dead_code)]
    pub(crate) fn as_path(&self) -> &Utf8Path {
        &self.path
    }
}

impl Deref for Utf8TempDir {
    type Target = Utf8Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl AsRef<Path> for Utf8TempDir {
    fn as_ref(&self) -> &Path {
        self.as_std_path()
    }
}
