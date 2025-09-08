use crate::common::{TestContext, cmd_snapshot};
use anyhow::Result;
use assert_fs::prelude::*;
use indoc::{formatdoc, indoc};

mod common;

#[test]
fn gc_cleans_unused_repo() -> Result<()> {
    let context = TestContext::new();
    context.init_project();
    context.configure_git_author();

    // Create a hook repository
    let hook_repo = context.temp_dir().child("hook_repo");
    hook_repo.create_dir_all()?; // Create the directory
    context.init_repo_at(hook_repo.path());
    hook_repo
        .child(".pre-commit-hooks.yaml")
        .write_str(indoc! {r#"
        -   id: echo
            name: echo
            entry: echo
            language: system
    "#})?;
    context.git_add_all_at(hook_repo.path());
    context.git_commit_at(hook_repo.path(), "feat: initial hook");
    let rev1 = context.get_rev_at(hook_repo.path());

    // Use the first revision of the hook
    context.write_pre_commit_config(&formatdoc! {r#"
        repos:
          - repo: {}
            rev: {}
            hooks:
              - id: echo
    "#, hook_repo.path().display(), rev1});
    context.git_add(".");

    // Run to cache the repo
    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    echo.....................................................................Passed

    ----- stderr -----
    "#);

    // Check that one repo is cached
    assert_eq!(context.home_dir().child("repos").read_dir()?.count(), 1);

    // Update the hook to a new revision
    hook_repo.child("another_file").touch()?;
    context.git_add_all_at(hook_repo.path());
    context.git_commit_at(hook_repo.path(), "feat: new commit");
    let rev2 = context.get_rev_at(hook_repo.path());

    context.write_pre_commit_config(&formatdoc! {r#"
        repos:
          - repo: {}
            rev: {}
            hooks:
              - id: echo
    "#, hook_repo.path().display(), rev2});
    context.git_add(".");

    // Run again to cache the new version
    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    echo.....................................................................Passed

    ----- stderr -----
    "#);

    // Now two repos should be cached
    assert_eq!(context.home_dir().child("repos").read_dir()?.count(), 2);

    // Run gc
    cmd_snapshot!(context.filters(), context.command().arg("gc"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    1 repo(s) removed.

    ----- stderr -----
    "#);

    // The old, unused repo should be gone
    assert_eq!(context.home_dir().child("repos").read_dir()?.count(), 1);

    Ok(())
}

#[test]
fn gc_does_not_remove_used_repo() -> Result<()> {
    let context = TestContext::new();
    context.init_project();
    context.configure_git_author();

    let hook_repo = context.temp_dir().child("hook_repo");
    hook_repo.create_dir_all()?; // Create the directory
    context.init_repo_at(hook_repo.path());
    hook_repo
        .child(".pre-commit-hooks.yaml")
        .write_str(indoc! {r#"
        -   id: echo
            name: echo
            entry: echo
            language: system
    "#})?;
    context.git_add_all_at(hook_repo.path());
    context.git_commit_at(hook_repo.path(), "feat: initial hook");
    let rev = context.get_rev_at(hook_repo.path());

    context.write_pre_commit_config(&formatdoc! {r#"
        repos:
          - repo: {}
            rev: {}
            hooks:
              - id: echo
    "#, hook_repo.path().display(), rev});
    context.git_add(".");
    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    echo.....................................................................Passed

    ----- stderr -----
    "#);

    assert_eq!(context.home_dir().child("repos").read_dir()?.count(), 1);

    // Run gc
    cmd_snapshot!(context.filters(), context.command().arg("gc"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    0 repo(s) removed.

    ----- stderr -----
    "#);

    // The used repo should still be there
    assert_eq!(context.home_dir().child("repos").read_dir()?.count(), 1);

    Ok(())
}

#[test]
fn gc_handles_local_and_meta_repos() -> Result<()> {
    let context = TestContext::new();
    context.init_project();
    context.configure_git_author();

    context.write_pre_commit_config(indoc! {r#"
        repos:
          - repo: local
            hooks:
              - id: local-hook
                name: My Local Hook
                entry: echo "local"
                language: system
          - repo: meta
            hooks:
              - id: identity
    "#});
    context.git_add(".");
    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    My Local Hook............................................................Passed
    identity.................................................................Passed
    - hook id: identity
    - duration: [TIME]
      .pre-commit-config.yaml

    ----- stderr -----
    "#);

    // No remote repos were cached
    assert!(!context.home_dir().child("repos").exists());

    // Run gc, should not crash
    cmd_snapshot!(context.filters(), context.command().arg("gc"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    0 repo(s) removed.

    ----- stderr -----
    "#);

    Ok(())
}

#[test]
fn gc_handles_deleted_config_file() -> Result<()> {
    let context = TestContext::new();
    context.init_project();
    context.configure_git_author();

    let hook_repo = context.temp_dir().child("hook_repo");
    hook_repo.create_dir_all()?; // Create the directory
    context.init_repo_at(hook_repo.path());
    hook_repo
        .child(".pre-commit-hooks.yaml")
        .write_str(indoc! {r#"
        -   id: echo
            name: echo
            entry: echo
            language: system
    "#})?;
    context.git_add_all_at(hook_repo.path());
    context.git_commit_at(hook_repo.path(), "feat: initial hook");
    let rev = context.get_rev_at(hook_repo.path());

    let config_path = context.work_dir().child(".pre-commit-config.yaml");
    config_path.write_str(&formatdoc! {r#"
        repos:
          - repo: {}
            rev: {}
            hooks:
              - id: echo
    "#, hook_repo.path().display(), rev})?;
    context.git_add(".");

    // Run to cache the repo and mark the config as used
    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    echo.....................................................................Passed

    ----- stderr -----
    "#);
    assert_eq!(context.home_dir().child("repos").read_dir()?.count(), 1);

    // Now, delete the config file
    fs_err::remove_file(&config_path)?;

    // Run gc. It should see the config is gone and clean up the repo.
    cmd_snapshot!(context.filters(), context.command().arg("gc"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    1 repo(s) removed.

    ----- stderr -----
    "#);

    // The repo cache should now be empty
    assert!(
        !context.home_dir().child("repos").exists()
            || context.home_dir().child("repos").read_dir()?.count() == 0
    );

    Ok(())
}
