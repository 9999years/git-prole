use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_add_branch_replacements() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;
    prole.write_config(
        r#"
        [[add.branch_replacements]]
        find = '''\w+/\w{1,4}-\d{1,5}-(\w+(?:-\w+){0,2}).*'''
        replace = '''$1'''
        "#,
    )?;

    prole
        .cd_cmd("my-repo/main")
        .args([
            "add",
            "-b",
            "doggy/pup-1234-my-cool-feature-with-very-very-very-long-name",
        ])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("my-cool-feature")
                .branch("doggy/pup-1234-my-cool-feature-with-very-very-very-long-name")
                .upstream("main"),
        ])
        .assert();

    Ok(())
}
