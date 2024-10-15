use command_error::CommandExt;
use expect_test::expect;
use miette::IntoDiagnostic;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_non_default_branch_checked_out() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole.sh("
        cd my-repo
        git switch -c puppy
        echo 'softie cutie' > README.md
        git commit -am 'cooler readme'
        ")?;

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main").file(
                "README.md",
                expect![[r#"
                    puppy doggy
                "#]],
            ),
            WorktreeState::new("puppy").branch("puppy").file(
                "README.md",
                expect![[r#"
                    softie cutie
                "#]],
            ),
        ])
        .assert();

    Ok(())
}
