use command_error::CommandExt;
use expect_test::expect;
use git_prole::HeadKind;
use pretty_assertions::assert_eq;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_detached_head() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole.sh("
        cd my-repo
        git switch --detach
        ")?;

    prole.cd_cmd("my-repo").arg("convert").status_checked()?;

    assert_eq!(
        prole.git("my-repo/main").refs().head_kind()?,
        HeadKind::Branch("main".into())
    );
    assert_eq!(
        prole.git("my-repo/work").refs().head_kind()?,
        HeadKind::Detached("4023d08019c45f462a9469778e78c3a1faad5013".into())
    );

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
            WorktreeState::new("work").detached("4023d080").file(
                "README.md",
                expect![[r#"
                    puppy doggy
                "#]],
            ),
        ])
        .assert();

    Ok(())
}
