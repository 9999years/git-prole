use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_bare_dot_git() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.sh(r#"
        mkdir -p my-repo/.git
        cd my-repo/.git || exit
        git init --bare

        git worktree add ../main
        cd ../main || exit
        echo "puppy doggy" > README.md 
        git add .
        git commit -m "Initial commit"

        git worktree add ../puppy
        git worktree add --detach ../doggy
        "#)?;

    prole
        .cd_cmd("my-repo/main")
        .arg("convert")
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").branch("puppy"),
            WorktreeState::new("doggy").detached("4023d080"),
        ])
        .assert();

    Ok(())
}
