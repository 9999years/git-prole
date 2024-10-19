use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_default_branches_default() -> miette::Result<()> {
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
        git remote rename origin puppy
        ")?;

    prole.cd_cmd("my-repo").arg("convert").status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            // We can't find a default remote, so we look for a default branch. We pull up `master`
            // because that's listed after `main`.
            //
            // Note: We can find a `master` branch on a remote even if it doesn't exist locally!
            WorktreeState::new("master")
                .branch("master")
                .upstream("puppy/trunk"),
            // We also get a checkout for the default HEAD on the remote when we clone, so that
            // sticks around.
            WorktreeState::new("puppy")
                .branch("puppy")
                .upstream("puppy/puppy"),
        ])
        .assert();

    Ok(())
}
