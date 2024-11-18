use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_copy_ignored_broken_symlink() -> miette::Result<()> {
    let prole = GitProle::new()?;

    prole.setup_worktree_repo("my-repo")?;

    prole.sh(r#"
        cd my-repo/main || exit

        echo "my-cool-symlink" >> .gitignore
        echo "symlink-to-directory" >> .gitignore
        echo "untracked-dir" >> .gitignore
        git add .gitignore
        git commit -m "Add .gitignore"

        ln -s does-not-exist my-cool-symlink
        mkdir untracked-dir
        ln -s does-not-exist untracked-dir/my-cooler-symlink
        ln -s untracked-dir symlink-to-directory
        "#)?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "puppy"])
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main").status([
                "!! my-cool-symlink",
                "!! symlink-to-directory",
                "!! untracked-dir/",
            ]),
            WorktreeState::new("puppy")
                .branch("puppy")
                .upstream("main")
                .status([
                    "!! my-cool-symlink",
                    "!! symlink-to-directory",
                    "!! untracked-dir/",
                ]),
        ])
        .assert();

    let link = prole.path("my-repo/puppy/my-cool-symlink");
    assert!(link.symlink_metadata().unwrap().is_symlink());
    assert_eq!(link.read_link_utf8().unwrap(), "does-not-exist");

    let link = prole.path("my-repo/puppy/symlink-to-directory");
    assert!(link.symlink_metadata().unwrap().is_symlink());
    assert_eq!(link.read_link_utf8().unwrap(), "untracked-dir");

    let link = prole.path("my-repo/puppy/untracked-dir/my-cooler-symlink");
    assert!(link.symlink_metadata().unwrap().is_symlink());
    assert_eq!(link.read_link_utf8().unwrap(), "does-not-exist");

    Ok(())
}
