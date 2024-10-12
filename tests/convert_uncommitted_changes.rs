use std::str::FromStr;

use command_error::CommandExt;
use expect_test::expect;
use git_prole::StatusEntry;
use miette::IntoDiagnostic;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn convert_uncommitted_changes() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole.sh("
        cd my-repo
        git switch -c puppy
        echo 'softie cutie' > README.md
        git add .
        ")?;

    assert_eq!(
        prole.git("my-repo").status().get()?.entries,
        vec![StatusEntry::from_str("M  README.md\0")?]
    );

    prole
        .cd_cmd("my-repo")
        .arg("convert")
        .status_checked()
        .into_diagnostic()?;

    prole.assert_contents(&[
        (
            "my-repo/main/README.md",
            expect![[r#"
                puppy doggy
            "#]],
        ),
        (
            "my-repo/puppy/README.md",
            expect![[r#"
                softie cutie
            "#]],
        ),
    ]);

    // /!\ /!\ /!\ /!\ /!\ /!\
    // TODO: This is a bug!!
    // We run a `git reset`, so we lose the staged changes!
    // Fix: Bring back the `git stash` if anything is staged?
    // /!\ /!\ /!\ /!\ /!\ /!\
    assert_eq!(
        prole.git("my-repo/puppy").status().get()?.entries,
        vec![StatusEntry::from_str(" M README.md\0")?]
    );

    // Different contents, same commits!
    assert_eq!(
        prole.git("my-repo/main").refs().get_head()?.abbrev(),
        "4023d080"
    );
    assert_eq!(
        prole.git("my-repo/puppy").refs().get_head()?.abbrev(),
        "4023d080"
    );

    Ok(())
}
