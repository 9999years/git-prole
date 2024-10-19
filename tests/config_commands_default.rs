use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_commands_default() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;

    prole
        .cd_cmd("my-repo")
        .args(["add", "puppy"])
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            // Wow girl give us nothing!
            WorktreeState::new("puppy").branch("puppy").status([]),
        ])
        .assert();

    Ok(())
}
