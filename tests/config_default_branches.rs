use command_error::CommandExt;
use miette::IntoDiagnostic;
use test_harness::GitProle;

#[test]
fn config_default_branches() -> miette::Result<()> {
    let prole = GitProle::new()?;

    prole.setup_repo("my-remotes/my-repo")?;

    prole.sh("
        pushd my-remotes/my-repo || exit
        git switch -c master
        git switch -c trunk
        git branch -D main
        git switch -c puppy
        popd

        git clone my-remotes/my-repo
        cd my-repo || exit
        git remote rename origin elephant
        ")?;

    prole.write_config(
        r#"
        default_branches = [
            "doggy",
            "trunk",
        ]
        "#,
    )?;

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    // We can't find a default remote, so we look for a default branch.
    assert_eq!(prole.current_branch_in("my-repo/trunk")?, "trunk");
    assert_eq!(
        prole.upstream_for_branch_in("my-repo/trunk", "trunk")?,
        "elephant/trunk"
    );
    // We also get a checkout for the default HEAD on the remote when we clone, so that
    // sticks around.
    assert_eq!(prole.current_branch_in("my-repo/puppy")?, "puppy");
    assert_eq!(
        prole.upstream_for_branch_in("my-repo/puppy", "puppy")?,
        "elephant/puppy"
    );

    Ok(())
}
