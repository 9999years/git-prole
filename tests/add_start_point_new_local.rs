use command_error::CommandExt;
use expect_test::expect;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn add_start_point_new_local() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    prole
        .sh("
        cd my-repo/main || exit
        git switch -c puppy
        echo 'soft cutie' > README.md
        git commit -am 'Cooler readme'
        ")
        .unwrap();

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "doggy", "@"])
        .status_checked()
        .unwrap();

    prole.assert_contents(&[(
        "my-repo/doggy/README.md",
        expect![[r#"
            soft cutie
        "#]],
    )]);

    assert_eq!(prole.current_branch_in("my-repo/doggy").unwrap(), "doggy");
}
