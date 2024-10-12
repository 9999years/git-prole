use command_error::CommandExt;
use expect_test::expect;
use miette::IntoDiagnostic;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn convert_default_branch_checked_out() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    prole.assert_contents(&[(
        "my-repo/main/README.md",
        expect![[r#"
            puppy doggy
        "#]],
    )]);

    assert_eq!(
        prole
            .git("my-repo/.git")
            .config()
            .get("core.bare")?
            .unwrap(),
        "true"
    );

    Ok(())
}
