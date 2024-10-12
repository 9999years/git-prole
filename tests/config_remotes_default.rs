use command_error::CommandExt;
use miette::IntoDiagnostic;
use test_harness::GitProle;

#[test]
fn convert_multiple_remotes() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-remotes/my-repo")?;

    prole.sh(r#"
        for repo in a b c; do
            pushd my-remotes || exit
            cp -r my-repo "$repo"
            pushd "$repo" || exit
            git switch -c "$repo"
            git branch -D main
            popd || exit
            popd || exit
        done
        cp -r my-remotes/my-repo my-remotes/a
        cp -r my-remotes/my-repo my-remotes/b
        cp -r my-remotes/my-repo my-remotes/c
        git clone my-remotes/my-repo
        cd my-repo || exit
        git remote add fork ../my-remotes/a
        git remote add upstream ../my-remotes/b
        git remote add puppy ../my-remotes/c
        "#)?;

    // Okay, this leaves us with remotes `fork`, `upstream`, and `puppy` with default branches
    // `a`, `b`, and `c` respectively.
    //
    // The default config says `upstream` is more important than `origin`, so we use that!

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    assert_eq!(prole.current_branch_in("my-repo/main")?, "main");

    Ok(())
}
