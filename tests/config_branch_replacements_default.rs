use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_branch_replacements_default() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "doggy/puppy"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy")
                .branch("doggy/puppy")
                .upstream("main"),
        ])
        .assert();

    Ok(())
}
