use command_error::CommandExt;
use indoc::indoc;
use miette::IntoDiagnostic;
use test_harness::GitProle;

#[test]
fn config_remotes() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-remotes/my-repo")?;

    prole.sh(indoc!(
        r#"
        mkdir -p .config/git-prole
        cat << EOF > .config/git-prole/config.toml
        remotes = [
            "fork"
        ]
        EOF

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
        "#
    ))?;

    // Okay, this leaves us with remotes `fork`, `upstream`, and `puppy` with default branches
    // `a`, `b`, and `c` respectively.

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    assert_eq!(prole.current_branch_in("my-repo/main")?, "main");
    assert_eq!(prole.current_branch_in("my-repo/a")?, "a");
    assert_eq!(prole.upstream_for_branch_in("my-repo/a", "a")?, "fork/a");

    Ok(())
}
