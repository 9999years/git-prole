use command_error::CommandExt;
use miette::IntoDiagnostic;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_no_op() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;

    prole.sh(r#"
        cd my-repo || exit
        git worktree add puppy
        git worktree add --detach doggy
    "#)?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy"),
            WorktreeState::new("doggy").detached("4023d080"),
        ])
        .assert();

    let output = prole
        .cd_cmd("my-repo")
        .arg("convert")
        .output_checked_utf8()
        .into_diagnostic()?;

    assert!(
        output.stderr.contains("is already a worktree repository"),
        "git-prole convert doesn't do anything the second time"
    );

    Ok(())
}
