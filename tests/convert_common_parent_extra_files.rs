use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_common_parent_extra_files() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-prefix/my-repo")?;

    prole.sh(r#"
        cd my-prefix/my-repo
        git worktree add ../puppy
        git worktree add ../doggy

        # This non-worktree path will prevent `my-prefix` from being used
        # as the destination.
        touch ../something-else
        "#)?;

    prole
        .cd_cmd("my-prefix/my-repo")
        .arg("convert")
        .status_checked()?;

    prole
        .repo_state("my-prefix/my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy"),
            WorktreeState::new("doggy").branch("doggy"),
        ])
        .assert();

    Ok(())
}
