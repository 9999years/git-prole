use std::fmt::Debug;
use std::fmt::Display;
use std::path::Path;
use std::path::MAIN_SEPARATOR_STR;

use camino::Utf8Path;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use path_absolutize::Absolutize;

use crate::current_dir::current_dir_utf8;

/// A way to display a path "nicely".
pub trait PathDisplay: Debug + AsRef<Path> {
    fn display_path_cwd(&self) -> String {
        current_dir_utf8()
            .map(|cwd| self.display_path_from(cwd))
            .unwrap_or_else(|error| {
                tracing::debug!(
                    %error,
                    path=?self,
                    "Failed to get current working directory for displaying path"
                );
                display_backup(self)
            })
    }

    fn display_path_from(&self, base: impl AsRef<Utf8Path>) -> String;
}

impl<P> PathDisplay for P
where
    P: AsRef<Utf8Path> + AsRef<Path> + Debug,
{
    fn display_path_from(&self, base: impl AsRef<Utf8Path>) -> String {
        try_display(self, base).unwrap_or_else(|| display_backup(self))
    }
}

fn display_backup(path: impl AsRef<Path>) -> String {
    make_colorful(path.as_ref().display())
}

fn make_colorful(path: impl Display) -> String {
    path.if_supports_color(Stream::Stdout, |text| text.cyan())
        .to_string()
}

fn try_display(path: impl AsRef<Utf8Path> + Debug, base: impl AsRef<Utf8Path>) -> Option<String> {
    try_display_inner(path.as_ref(), base).map(make_colorful)
}

fn try_display_inner(
    path: impl AsRef<Utf8Path> + Debug,
    base: impl AsRef<Utf8Path>,
) -> Option<String> {
    let base = base.as_ref();
    let normal: &Path = path.as_ref().as_ref();
    let normal = normal.absolutize_from(base).ok()?;
    let normal = Utf8Path::from_path(&normal)?;

    if let Some(home) = dirs::home_dir() {
        if let Ok(from_home) = normal.strip_prefix(&home) {
            return Some(format!("~{MAIN_SEPARATOR_STR}{from_home}"));
        }
    }

    let temp_dir = std::env::temp_dir();
    if let Ok(from_temp) = normal.strip_prefix(&temp_dir) {
        return Some(format!("$TMPDIR{MAIN_SEPARATOR_STR}{from_temp}"));
    }

    // Evil: On macOS, `/tmp` and `$TMPDIR` start with symlinks to `/private`, so you need to
    // follow symlinks to check if a path actually starts with the tempdir.
    if let Ok(canon_temp_dir) = temp_dir.canonicalize() {
        if let Ok(from_temp) = normal.strip_prefix(&canon_temp_dir) {
            return Some(format!("$TMPDIR{MAIN_SEPARATOR_STR}{from_temp}"));
        }
    }

    // TODO: Is it worth trying relative paths in some cases?
    Some(normal.to_string())
}
