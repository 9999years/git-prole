use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_add_branch_replacements_count() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;
    prole.write_config(
        r#"
        [[add.branch_replacements]]
        find = '''puppy'''
        replace = '''doggy'''
        count = 1
        "#,
    )?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "puppypuppypuppy"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("doggypuppypuppy")
                .branch("puppypuppypuppy")
                .upstream("main"),
        ])
        .assert();

    Ok(())
}
