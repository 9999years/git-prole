use command_error::CommandExt;
use miette::IntoDiagnostic;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_common_prefix() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-prefix/my-repo")?;

    prole.sh(r#"
        cd my-prefix/my-repo
        git worktree add ../puppy
        git worktree add ../doggy
        git worktree add silly
        git worktree add silly/cutie
        "#)?;

    prole
        .cd_cmd("my-prefix/my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    prole
        .repo_state("my-prefix/my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy"),
            WorktreeState::new("doggy").branch("doggy"),
            WorktreeState::new("silly").branch("silly"),
            WorktreeState::new("cutie").branch("cutie"),
        ])
        .assert();

    Ok(())
}
