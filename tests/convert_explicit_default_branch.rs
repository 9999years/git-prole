use command_error::CommandExt;
use test_harness::setup_repo_multiple_remotes;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_explicit_default_branch() -> miette::Result<()> {
    let prole = GitProle::new()?;
    setup_repo_multiple_remotes(&prole, "my-remotes/my-repo", "my-repo")?;

    prole.sh(r#"
        cd my-repo || exit
        git fetch a
    "#)?;

    prole
        .cd_cmd("my-repo")
        .args(["convert", "--default-branch", "a/a"])
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main")
                .branch("main")
                .upstream("origin/main"),
            WorktreeState::new("a").branch("a").upstream("a/a"),
        ])
        .assert();

    Ok(())
}
