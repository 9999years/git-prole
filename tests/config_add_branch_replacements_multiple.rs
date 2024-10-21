use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_add_branch_replacements_multiple() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;
    prole.write_config(
        r#"
        [[add.branch_replacements]]
        find = '''puppy'''
        replace = '''doggy'''

        [[add.branch_replacements]]
        find = '''doggy'''
        replace = '''cutie'''
        "#,
    )?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "silly-puppy"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("silly-cutie")
                .branch("silly-puppy")
                .upstream("main"),
        ])
        .assert();

    Ok(())
}
