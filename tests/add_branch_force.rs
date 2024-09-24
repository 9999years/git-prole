use command_error::CommandExt;
use expect_test::expect;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn add_branch_force() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    prole
        .sh("
        cd my-repo/main || exit
        git switch -c puppy
        echo 'softy pup' > README.md
        git commit -am 'cooler readme'
        git switch main
        ")
        .unwrap();

    // `-b` fails; the branch already exists.
    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "puppy"])
        .status_checked()
        .unwrap_err();

    // -B works though!
    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-B", "puppy"])
        .status_checked()
        .unwrap();

    // Branch is reset, so we don't see the updated readme.
    prole.assert_contents(&[(
        "my-repo/puppy/README.md",
        expect![[r#"
            puppy doggy
        "#]],
    )]);

    assert_eq!(prole.current_branch_in("my-repo/puppy").unwrap(), "puppy");
}
