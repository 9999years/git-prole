//! Like [`fs_err`], but the functions are instrumented with [`macro@tracing::instrument`] and return
//! [`miette::Result`] instead of [`std::io::Result`].

use std::fmt::Debug;
use std::path::Path;

use miette::IntoDiagnostic;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn rename<P, Q>(from: P, to: Q) -> miette::Result<()>
where
    P: AsRef<Path> + Debug,
    Q: AsRef<Path> + Debug,
{
    #[expect(clippy::disallowed_methods)]
    fs_err::rename(from, to).into_diagnostic()
}

#[instrument(level = "trace")]
pub fn create_dir<P>(path: P) -> miette::Result<()>
where
    P: AsRef<Path> + Debug,
{
    #[expect(clippy::disallowed_methods)]
    fs_err::create_dir(path).into_diagnostic()
}

#[instrument(level = "trace")]
pub fn create_dir_all<P>(path: P) -> miette::Result<()>
where
    P: AsRef<Path> + Debug,
{
    #[expect(clippy::disallowed_methods)]
    fs_err::create_dir_all(path).into_diagnostic()
}

#[instrument(level = "trace")]
pub fn remove_dir<P>(path: P) -> miette::Result<()>
where
    P: AsRef<Path> + Debug,
{
    #[expect(clippy::disallowed_methods)]
    fs_err::remove_dir(path).into_diagnostic()
}

#[instrument(level = "trace")]
pub fn read_to_string<P>(path: P) -> miette::Result<String>
where
    P: AsRef<Path> + Debug,
{
    #[expect(clippy::disallowed_methods)]
    fs_err::read_to_string(path).into_diagnostic()
}

#[instrument(level = "trace")]
pub fn copy<P, Q>(from: P, to: Q) -> miette::Result<u64>
where
    P: AsRef<Path> + Debug,
    Q: AsRef<Path> + Debug,
{
    #[expect(clippy::disallowed_methods)]
    fs_err::copy(from, to).into_diagnostic()
}

#[instrument(level = "trace")]
pub fn write<P, C>(path: P, contents: C) -> miette::Result<()>
where
    P: AsRef<Path> + Debug,
    C: AsRef<[u8]> + Debug,
{
    #[expect(clippy::disallowed_methods)]
    fs_err::write(path, contents).into_diagnostic()
}
