use std::borrow::Borrow;
use std::env;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::ops::Deref;
use std::path::Path;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use common_path::common_path;
use miette::miette;
use miette::IntoDiagnostic;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use owo_colors::Stream::Stdout;
use path_absolutize::Absolutize;

use crate::current_dir::current_dir_utf8;

/// A normalized [`Utf8PathBuf`] in tandem with a relative path.
///
/// Normalized paths are absolute paths with dots removed; see [`path_dedot`][path_dedot] and
/// [`path_absolutize`] for more details.
///
/// These paths are [`Display`]ed as the relative path but compared ([`Hash`], [`Eq`], [`Ord`]) as
/// the normalized path.
///
/// [path_dedot]: https://docs.rs/path-dedot/latest/path_dedot/
#[derive(Debug, Clone)]
pub struct NormalPath {
    normal: Utf8PathBuf,
    relative: Option<Utf8PathBuf>,
}

impl NormalPath {
    pub fn try_display_cwd(original: impl AsRef<Utf8Path>) -> String {
        let original = original.as_ref();
        Self::from_cwd(original)
            .map(|normal_path| normal_path.to_string())
            .unwrap_or_else(|_| {
                original
                    .if_supports_color(Stream::Stdout, |text| text.cyan())
                    .to_string()
            })
    }

    /// Creates a new normalized path relative to the given base path.
    pub fn new(original: impl AsRef<Path>, base: impl AsRef<Utf8Path>) -> miette::Result<Self> {
        let base = base.as_ref();
        let normal = original.as_ref().absolutize_from(base).into_diagnostic()?;
        let normal = normal
            .into_owned()
            .try_into()
            .map_err(|err| miette!("{err}"))?;
        let relative = if common_path(&normal, base).is_some() {
            pathdiff::diff_utf8_paths(&normal, base)
        } else {
            None
        };
        Ok(Self { normal, relative })
    }

    /// Create a new normalized path relative to the current working directory.
    pub fn from_cwd(original: impl AsRef<Path>) -> miette::Result<Self> {
        Self::new(original, current_dir_utf8()?)
    }

    /// Get a reference to the absolute (normalized) path, borrowed as a [`Utf8Path`].
    pub fn absolute(&self) -> &Utf8Path {
        self.normal.as_path()
    }

    /// Get a reference to the relative path, borrowed as a [`Utf8Path`].
    ///
    /// If no relative path is present, the absolute (normalized) path is used instead.
    pub fn relative(&self) -> &Utf8Path {
        self.relative.as_deref().unwrap_or_else(|| self.absolute())
    }

    pub fn push(&mut self, component: impl AsRef<Utf8Path>) {
        let component = component.as_ref();
        self.normal.push(component);
        if let Some(path) = self.relative.as_mut() {
            path.push(component);
        }
    }
}

// Hash, Eq, and Ord delegate to the normalized path.
impl Hash for NormalPath {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Hash::hash(&self.normal, state);
    }
}

impl PartialEq for NormalPath {
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&self.normal, &other.normal)
    }
}

impl Eq for NormalPath {}

impl PartialOrd for NormalPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NormalPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Ord::cmp(&self.normal, &other.normal)
    }
}

impl Display for NormalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = match &self.relative {
            Some(path) => path.as_path(),
            None => self.normal.as_path(),
        };
        if path.as_str().is_empty() {
            write!(
                f,
                "{}",
                "$PWD".if_supports_color(Stdout, |text| text.cyan())
            )
        } else {
            let temp_dir = Utf8PathBuf::try_from(env::temp_dir()).ok();
            write!(
                f,
                "{}",
                &match temp_dir.and_then(|temp_dir| self.normal.strip_prefix(temp_dir).ok()) {
                    Some(after_tmpdir) => {
                        format!("$TMPDIR{}{}", std::path::MAIN_SEPARATOR_STR, after_tmpdir)
                    }
                    None => path.as_str().to_owned(),
                }
                .if_supports_color(Stdout, |text| text.cyan())
            )
        }
    }
}

impl From<NormalPath> for Utf8PathBuf {
    fn from(value: NormalPath) -> Self {
        value.normal
    }
}

impl AsRef<Utf8Path> for NormalPath {
    fn as_ref(&self) -> &Utf8Path {
        &self.normal
    }
}

impl AsRef<Path> for NormalPath {
    fn as_ref(&self) -> &Path {
        self.normal.as_std_path()
    }
}

impl Borrow<Utf8PathBuf> for NormalPath {
    fn borrow(&self) -> &Utf8PathBuf {
        &self.normal
    }
}

impl Borrow<Utf8Path> for NormalPath {
    fn borrow(&self) -> &Utf8Path {
        self.normal.as_path()
    }
}

impl Deref for NormalPath {
    type Target = Utf8PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.normal
    }
}
