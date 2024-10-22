use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_add_branch_replacements_path_separator() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;
    prole.write_config(
        r#"
        [[add.branch_replacements]]
        find = "doggy"
        replace = "silly"
        "#,
    )?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "puppy/doggy"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            // Last component of the result of the replacements is used:
            WorktreeState::new("silly")
                .branch("puppy/doggy")
                .upstream("main"),
        ])
        .assert();

    Ok(())
}
