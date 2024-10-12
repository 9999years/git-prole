use command_error::CommandExt;
use miette::IntoDiagnostic;
use test_harness::GitProle;

#[test]
fn convert_multiple_worktrees() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole.sh("
        cd my-repo || exit
        git worktree add ../puppy
        git worktree add ../doggy
        ")?;

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()
        // Not implemented yet!
        .unwrap_err();

    Ok(())
}
