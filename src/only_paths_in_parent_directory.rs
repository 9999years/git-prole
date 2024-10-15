use std::fmt::Debug;

use camino::Utf8Path;
use miette::IntoDiagnostic;
use rustc_hash::FxHashSet;
use tracing::instrument;

/// Check if a set of paths all have the same parent directory and they are the only paths in that
/// directory (other than dotfiles).
#[instrument(level = "trace")]
pub fn only_paths_in_parent_directory<'p, I, P>(paths: I) -> Option<&'p Utf8Path>
where
    I: IntoIterator<Item = &'p P> + Debug,
    P: AsRef<Utf8Path> + 'p + ?Sized,
{
    let mut paths = paths.into_iter();
    let mut names = FxHashSet::default();
    let first = paths.next()?.as_ref();
    let parent = first.parent()?;
    names.insert(first.file_name()?);

    for path in paths {
        let path = path.as_ref();
        if path.parent()? != parent {
            return None;
        }
        names.insert(path.file_name()?);
    }

    match path_contains_only_names_and_dotfiles(parent, &names) {
        Ok(true) => Some(parent),
        Ok(false) => None,
        Err(error) => {
            tracing::debug!(
                directory=%parent,
                error=%error,
                "Error while listing directory"
            );
            None
        }
    }
}

/// Check if a path contains only files listed in the given set of names and dotfiles.
#[instrument(level = "trace")]
fn path_contains_only_names_and_dotfiles(
    path: &Utf8Path,
    names: &FxHashSet<&str>,
) -> miette::Result<bool> {
    for entry in path.read_dir_utf8().into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let name = entry.file_name();
        if !name.starts_with('.') && !names.contains(name) {
            tracing::debug!(
                directory=%path,
                entry=%name,
                "Directory entry is not a dotfile or listed in known paths"
            );
            return Ok(false);
        }
    }

    Ok(true)
}
