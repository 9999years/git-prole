use command_error::CommandExt;
use miette::IntoDiagnostic;
use test_harness::GitProle;
use test_harness::WorktreeState;

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

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            // We can't find a default remote, so we look for a default branch.
            WorktreeState::new("trunk")
                .branch("trunk")
                .upstream("elephant/trunk"),
            // We also get a checkout for the default HEAD on the remote when we clone, so that
            // sticks around.
            WorktreeState::new("puppy")
                .branch("puppy")
                .upstream("elephant/puppy"),
        ])
        .assert();

    Ok(())
}
