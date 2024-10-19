use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_common_parent_extra_dotfiles() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-prefix/my-repo")?;

    prole.sh(r#"
        cd my-prefix/my-repo
        git worktree add ../puppy
        git worktree add ../doggy

        # This non-worktree path will NOT prevent `my-prefix` from being used
        # as the destination, because it's a dotfile.
        echo 'puppy = "cute"' > ../.my-config-file.toml
        "#)?;

    prole
        .cd_cmd("my-prefix/my-repo")
        .arg("convert")
        .status_checked()?;

    prole
        .repo_state("my-prefix")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy"),
            WorktreeState::new("doggy").branch("doggy"),
        ])
        .assert();

    // The config file we write should be preserved!
    prole.assert_contents(&[(
        "my-prefix/.my-config-file.toml",
        expect![[r#"
            puppy = "cute"
        "#]],
    )]);

    Ok(())
}
