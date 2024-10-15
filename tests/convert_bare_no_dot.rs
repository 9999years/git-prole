use command_error::CommandExt;
use miette::IntoDiagnostic;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_bare_no_dot() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.sh(r#"
        mkdir -p my-repo
        cd my-repo || exit
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
        .cd_cmd("main")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

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
