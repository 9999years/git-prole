use command_error::CommandExt;
use test_harness::GitProle;

#[test]
fn add_destination_exists() -> miette::Result<()> {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    prole.sh(r#"
        cd my-repo || exit
        mkdir puppy
    "#)?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "puppy"])
        .status_checked()
        .unwrap_err();

    Ok(())
}
