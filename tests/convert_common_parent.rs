use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_common_parent() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-prefix/my-repo")?;

    prole.sh(r#"
        cd my-prefix/my-repo
        git worktree add ../puppy
        git worktree add ../doggy
        "#)?;

    prole
        .cd_cmd("my-prefix/my-repo")
        .arg("convert")
        .status_checked()?;

    prole
        .repo_state("my-prefix")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy"),
            WorktreeState::new("doggy").branch("doggy"),
        ])
        .assert();

    Ok(())
}
