use std::borrow::Cow;
use std::path::Path;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use path_absolutize::Absolutize;
use tap::TryConv;

/// Like [`Absolutize`] but for [`camino`] paths.
pub trait Utf8Absolutize {
    /// Get an absolute path. This works even if the path does not exist.
    fn absolutize(&self) -> std::io::Result<Cow<Utf8Path>>;

    /// Get an absolute path. This works even if the path does not exist. It gets the current
    /// working directory as the second argument.
    #[expect(dead_code)]
    fn absolutize_from(&self, cwd: impl AsRef<Path>) -> std::io::Result<Cow<Utf8Path>>;

    /// Get an absolute path. This works even if the path does not exist.
    #[expect(dead_code)]
    fn absolutize_virtually(
        &self,
        virtual_root: impl AsRef<Path>,
    ) -> std::io::Result<Cow<Utf8Path>>;
}

fn cow_path_to_cow_utf8_path(path: Cow<Path>) -> std::io::Result<Cow<Utf8Path>> {
    match path {
        Cow::Borrowed(path) => path
            .try_conv::<&Utf8Path>()
            .map_err(|err| err.into_io_error())
            .map(Into::into),
        Cow::Owned(path_buf) => path_buf
            .try_conv::<Utf8PathBuf>()
            .map_err(|err| err.into_io_error())
            .map(Into::into),
    }
}

impl Utf8Absolutize for Utf8Path {
    fn absolutize(&self) -> std::io::Result<Cow<Utf8Path>> {
        Absolutize::absolutize(self.as_std_path()).and_then(cow_path_to_cow_utf8_path)
    }

    fn absolutize_from(&self, cwd: impl AsRef<Path>) -> std::io::Result<Cow<Utf8Path>> {
        Absolutize::absolutize_from(self.as_std_path(), cwd).and_then(cow_path_to_cow_utf8_path)
    }

    fn absolutize_virtually(
        &self,
        virtual_root: impl AsRef<Path>,
    ) -> std::io::Result<Cow<Utf8Path>> {
        Absolutize::absolutize_virtually(self.as_std_path(), virtual_root)
            .and_then(cow_path_to_cow_utf8_path)
    }
}

impl Utf8Absolutize for Utf8PathBuf {
    fn absolutize(&self) -> std::io::Result<Cow<Utf8Path>> {
        Absolutize::absolutize(self.as_std_path()).and_then(cow_path_to_cow_utf8_path)
    }

    fn absolutize_from(&self, cwd: impl AsRef<Path>) -> std::io::Result<Cow<Utf8Path>> {
        Absolutize::absolutize_from(self.as_std_path(), cwd).and_then(cow_path_to_cow_utf8_path)
    }

    fn absolutize_virtually(
        &self,
        virtual_root: impl AsRef<Path>,
    ) -> std::io::Result<Cow<Utf8Path>> {
        Absolutize::absolutize_virtually(self.as_std_path(), virtual_root)
            .and_then(cow_path_to_cow_utf8_path)
    }
}
