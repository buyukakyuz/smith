use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

fn smith_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_smith"))
}

#[test]
fn test_cli_help() {
    smith_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("AI developer tool"))
        .stdout(predicate::str::contains("--model"));
}

#[test]
fn test_cli_version() {
    smith_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("smith"));
}

#[test]
fn test_config_where() {
    smith_cmd().args(["config", "where"]).assert().success();
}

#[test]
fn test_invalid_subcommand() {
    smith_cmd().arg("invalid-command").assert().failure();
}
