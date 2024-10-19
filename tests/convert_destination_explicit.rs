use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_destination_explicit() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole
        .cd_cmd("my-repo")
        .args(["convert", "../puppy"])
        .status_checked()?;

    prole.sh("ls -la && ls -la puppy")?;

    prole
        .repo_state("puppy")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
        ])
        .assert();

    Ok(())
}
