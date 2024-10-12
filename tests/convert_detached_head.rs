use command_error::CommandExt;
use expect_test::expect;
use git_prole::HeadKind;
use miette::IntoDiagnostic;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn convert_detached_head() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole.sh("
        cd my-repo
        git switch --detach
        ")?;

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    prole.assert_contents(&[
        (
            "my-repo/main/README.md",
            expect![[r#"
                puppy doggy
            "#]],
        ),
        (
            "my-repo/work/README.md",
            expect![[r#"
                puppy doggy
            "#]],
        ),
    ]);

    assert_eq!(
        prole.git("my-repo/main").refs().head_kind()?,
        HeadKind::Branch("main".into())
    );
    assert_eq!(
        prole.git("my-repo/work").refs().head_kind()?,
        HeadKind::Detached("4023d08019c45f462a9469778e78c3a1faad5013".into())
    );

    Ok(())
}
