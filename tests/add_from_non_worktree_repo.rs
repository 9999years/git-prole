use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_from_non_worktree_repo() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole.write_config(
        r#"
        branch_names = []
        "#,
    )?;

    prole.sh(r#"
        cd my-repo || exit
        git switch -c puppy
        git branch -D main
        echo puppyyyy > puppy-file
    "#)?;

    // We can add a worktree from the container directory (outside of any working tree but
    // "within" the repo as far as Git is concerned).
    prole
        .cd_cmd("my-repo")
        .args(["add", "doggy", "@"])
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new("").branch("puppy"),
            // Copied from first non-bare worktree even if no default is found.
            WorktreeState::new("../doggy").branch("doggy").file(
                "puppy-file",
                expect![[r#"
                    puppyyyy
                "#]],
            ),
        ])
        .assert();

    Ok(())
}
