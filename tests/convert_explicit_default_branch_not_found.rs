use command_error::CommandExt;
use test_harness::setup_repo_multiple_remotes;
use test_harness::GitProle;

#[test]
fn convert_explicit_default_branch_not_found() -> miette::Result<()> {
    let prole = GitProle::new()?;
    setup_repo_multiple_remotes(&prole, "my-remotes/my-repo", "my-repo")?;

    prole
        .cd_cmd("my-repo")
        .args(["convert", "--default-branch", "d/a"])
        .status_checked()
        .unwrap_err();

    Ok(())
}
