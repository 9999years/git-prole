use crate::GitProle;
use camino::Utf8Path;
use miette::miette;

/// Set up a remote in `remote_path` with multiple other remotes as its siblings, and clone that
/// remote to `repo`.
pub fn setup_repo_multiple_remotes(
    prole: &GitProle,
    remote_path: &str,
    repo: &str,
) -> miette::Result<()> {
    prole.setup_repo(remote_path)?;

    let basename = Utf8Path::new(remote_path)
        .file_name()
        .ok_or_else(|| miette!("Remote has no basename: {remote_path}"))?;

    prole.sh(&format!(
        r#"
        for repo in a b c; do
            pushd "{remote_path}/.." || exit
            cp -r "{basename}" "$repo"
            pushd "$repo" || exit
            git switch -c "$repo"
            echo "I am on branch $repo" > README.md
            git commit -am "Update README.md"
            git branch -D main
            popd || exit
            popd || exit
        done
        git clone "{remote_path}" "{repo}"
        cd "{repo}" || exit
        git remote add a ../{remote_path}/../a
        git remote add b ../{remote_path}/../b
        git remote add c ../{remote_path}/../c
        "#
    ))?;

    Ok(())
}
