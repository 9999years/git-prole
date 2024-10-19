use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_from_container() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;

    // We can add a worktree from the container directory (outside of any working tree but
    // "within" the repo as far as Git is concerned).
    prole
        .cd_cmd("my-repo")
        .args(["add", "puppy"])
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy").upstream("main"),
        ])
        .assert();

    Ok(())
}
