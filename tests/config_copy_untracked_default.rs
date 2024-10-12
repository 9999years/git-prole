use command_error::CommandExt;
use expect_test::expect;
use miette::IntoDiagnostic;
use test_harness::GitProle;

#[test]
fn config_copy_untracked_default() -> miette::Result<()> {
    let prole = GitProle::new()?;

    prole.setup_worktree_repo("my-repo")?;

    prole.sh("
        cd my-repo/main || exit
        echo 'puppy doggy' > animal-facts.txt
        ")?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "puppy"])
        .status_checked()
        .into_diagnostic()?;

    // The untracked file is copied to the new worktree.
    prole.assert_contents(&[
        (
            "my-repo/main/animal-facts.txt",
            expect![[r#"
                puppy doggy
            "#]],
        ),
        (
            "my-repo/puppy/animal-facts.txt",
            expect![[r#"
                puppy doggy
            "#]],
        ),
    ]);

    Ok(())
}
