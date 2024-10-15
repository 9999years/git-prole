use command_error::CommandExt;
use expect_test::expect;
use miette::IntoDiagnostic;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_default_branch_checked_out() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main")
                .branch("main")
                .no_upstream()
                .file(
                    "README.md",
                    expect![[r#"
                        puppy doggy
                    "#]],
                ),
        ])
        .assert();

    Ok(())
}
