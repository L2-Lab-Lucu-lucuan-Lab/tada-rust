use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn doctor_reports_environment() {
    let dir = tempdir().expect("temp dir");
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("tada-rust"));
    cmd.env("TADA_HOME", dir.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("tada-rust doctor"))
        .stdout(contains("quran source"));
}

#[test]
#[ignore = "requires external API network access"]
fn read_single_ayah_works() {
    let dir = tempdir().expect("temp dir");
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("tada-rust"));
    cmd.env("TADA_HOME", dir.path())
        .args(["read", "--surah", "1", "--ayah", "1"])
        .assert()
        .success()
        .stdout(contains("1:1"))
        .stdout(contains("Dengan nama Allah"));
}

#[test]
fn bookmark_add_then_list() {
    let dir = tempdir().expect("temp dir");

    let mut add = Command::new(assert_cmd::cargo::cargo_bin!("tada-rust"));
    add.env("TADA_HOME", dir.path())
        .args([
            "bookmark",
            "add",
            "--surah",
            "1",
            "--ayah",
            "2",
            "--note",
            "test-note",
        ])
        .assert()
        .success()
        .stdout(contains("Bookmark #"));

    let mut list = Command::new(assert_cmd::cargo::cargo_bin!("tada-rust"));
    list.env("TADA_HOME", dir.path())
        .args(["bookmark", "list"])
        .assert()
        .success()
        .stdout(contains("test-note"));
}
