use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_commands() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;

    prole.write_config(
        r#"
        commands = [
            "sh -c 'echo Puppy wuz here > puppy-log'",
            { sh = '''
                echo 2wice the Pupyluv >> puppy-log
              ''' },
        ]
        "#,
    )?;

    prole
        .cd_cmd("my-repo")
        .args(["add", "puppy"])
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy").file(
                "puppy-log",
                expect![[r#"
                    Puppy wuz here
                    2wice the Pupyluv
                "#]],
            ),
        ])
        .assert();

    Ok(())
}
