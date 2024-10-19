use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_multiple_worktrees() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole.sh("
        # Another path here keeps `git-prole` from using the tempdir as the root.
        mkdir my-other-repo
        cd my-repo || exit
        git worktree add ../puppy
        git worktree add ../doggy
        ")?;

    prole.cd_cmd("my-repo").arg("convert").status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy"),
            WorktreeState::new("doggy").branch("doggy"),
        ])
        .assert();

    Ok(())
}
