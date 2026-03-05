use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn root_help_includes_interactive_and_logging_flags() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("tada-rust"));
    cmd.args(["--help"])
        .assert()
        .success()
        .stdout(contains("interactive"))
        .stdout(contains("--debug"))
        .stdout(contains("--no-color"))
        .stdout(contains("--yes"));
}

#[test]
fn read_help_includes_examples() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("tada-rust"));
    cmd.args(["read", "--help"])
        .assert()
        .success()
        .stdout(contains("Contoh:"))
        .stdout(contains("read --surah 1 --ayah 1"));
}

#[test]
fn bookmark_remove_help_mentions_yes_flag() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("tada-rust"));
    cmd.args(["bookmark", "remove", "--help"])
        .assert()
        .success()
        .stdout(contains("--yes"))
        .stdout(contains("bookmark remove 4 --yes"));
}
