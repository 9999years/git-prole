use command_error::CommandExt;
use miette::IntoDiagnostic;
use test_harness::setup_repo_multiple_remotes;
use test_harness::GitProle;

#[test]
fn convert_multiple_remotes() -> miette::Result<()> {
    let prole = GitProle::new()?;
    setup_repo_multiple_remotes(&prole, "my-remotes/my-repo", "my-repo")?;

    prole.sh("
        cd my-repo || exit
        git remote rename a upstream
        ")?;

    // Okay, this leaves us with remotes `origin`, `upstream`, `b`, and `c`.
    //
    // The default config says `upstream` is more important than `origin`, so we use that!

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    assert_eq!(prole.current_branch_in("my-repo/a")?, "a");
    assert_eq!(
        prole.upstream_for_branch_in("my-repo/a", "a")?,
        "upstream/a"
    );

    Ok(())
}
