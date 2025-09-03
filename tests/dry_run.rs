use anyhow::Result;
use assert_fs::prelude::*;

use crate::common::{TestContext, cmd_snapshot};

mod common;

#[test]
fn dry_run_modifies_nothing_and_succeeds() -> Result<()> {
    // 1. Setup
    let context = TestContext::new();
    context.init_project();
    context.write_pre_commit_config(indoc::indoc! {r"
        repos:
          - repo: https://github.com/pre-commit/pre-commit-hooks
            rev: v5.0.0
            hooks:
              - id: end-of-file-fixer
    "});

    // Create a file that the hook would normally fix
    let test_file = context.work_dir().child("file.txt");
    test_file.write_str("This file has no trailing newline")?;
    let original_content = context.read("file.txt");

    context.git_add(".");

    // 2. Execute with --dry-run
    cmd_snapshot!(context.filters(), context.run().arg("--dry-run"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    fix end of files........................................................Dry Run
      - .pre-commit-config.yaml
      - file.txt

    ----- stderr -----
    "#);

    // 3. Assert that the file was NOT modified
    let content_after = context.read("file.txt");
    assert_eq!(
        original_content, content_after,
        "File should not be modified in dry-run mode"
    );

    Ok(())
}

#[test]
fn normal_run_modifies_file_and_fails() -> Result<()> {
    // 1. Setup (same as above)
    let context = TestContext::new();
    context.init_project();
    context.write_pre_commit_config(indoc::indoc! {r"
        repos:
          - repo: https://github.com/pre-commit/pre-commit-hooks
            rev: v5.0.0
            hooks:
              - id: end-of-file-fixer
    "});

    let test_file = context.work_dir().child("file.txt");
    test_file.write_str("This file has no trailing newline")?;
    let original_content = context.read("file.txt");

    context.git_add(".");

    // 2. Execute a normal run (without --dry-run)
    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: false
    exit_code: 1
    ----- stdout -----
    fix end of files.........................................................Failed
    - hook id: end-of-file-fixer
    - exit code: 1
    - files were modified by this hook
      Fixing file.txt

    ----- stderr -----
    "#);

    // 3. Assert that the file WAS modified
    let content_after = context.read("file.txt");
    assert_ne!(
        original_content, content_after,
        "File should be modified in a normal run"
    );
    assert_eq!(content_after, "This file has no trailing newline\n");

    Ok(())
}
