//! Black-box tests of the binary's surface. None reaches the Keychain: the
//! service is an empty test one, the config is a temp dir, and every case
//! renders help or info, works on an empty state file, or is refused by
//! argument checks before any credential read.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};

/// A fresh scratch dir per call. The counter, not the clock, makes it
/// unique: parallel tests read the same instant often enough on macOS.
fn tmp() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "kaiser-cli-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn kp(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kp").unwrap();
    cmd.env("KP_KEYCHAIN_SERVICE", "piekstra.kp.hermetic-tests")
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("HOME", home)
        .env_remove("KP_CONFIG")
        .env_remove("NO_COLOR");
    cmd
}

fn json_of(out: &[u8]) -> Value {
    serde_json::from_slice(out).unwrap_or_else(|e| {
        panic!(
            "not one JSON document ({e}): {}",
            String::from_utf8_lossy(out)
        )
    })
}

fn stdout_json(cmd: &mut Command) -> Value {
    let out = cmd.assert().success();
    json_of(&out.get_output().stdout)
}

#[test]
fn every_command_renders_help() {
    let home = tmp();
    for args in [
        vec!["--help"],
        vec!["orgs", "--help"],
        vec!["orgs", "list", "--help"],
        vec!["orgs", "search", "--help"],
        vec!["auth", "--help"],
        vec!["auth", "set-credential", "--help"],
        vec!["auth", "login", "--help"],
        vec!["auth", "status", "--help"],
        vec!["auth", "logout", "--help"],
        vec!["patients", "--help"],
        vec!["patients", "list", "--help"],
        vec!["records", "--help"],
        vec!["records", "export", "--help"],
        vec!["api", "--help"],
        vec!["config", "--help"],
        vec!["config", "path", "--help"],
        vec!["config", "show", "--help"],
        vec!["config", "set", "--help"],
        vec!["config", "unset", "--help"],
        vec!["self-update", "--help"],
        vec!["completions", "--help"],
        vec!["info", "--help"],
    ] {
        kp(&home)
            .args(&args)
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage"));
    }
}

#[test]
fn version_and_completions() {
    let home = tmp();
    kp(&home)
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("kp "));
    for shell in ["bash", "zsh", "fish"] {
        kp(&home)
            .args(["completions", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains("kp"));
    }
}

#[test]
fn info_is_cli_info() {
    let home = tmp();
    let v = stdout_json(kp(&home).arg("info"));
    assert_eq!(v["schema"], "cli-info/v1");
    assert_eq!(v["name"], "kp");
    assert_eq!(v["auth"]["method"], "browser-session");
    assert_eq!(v["profiles"], json!(["health-records/v1"]));
    assert!(v.to_string().contains("piekstra/kaiser-cli"));
}

#[test]
fn orgs_list_and_search_emit_org_list() {
    let home = tmp();
    let v = stdout_json(kp(&home).args(["orgs", "list", "--json"]));
    assert_eq!(v["schema"], "org-list/v1");
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "kaiser-health-plan");
    assert_eq!(items[1]["id"], "kaiser-health-plan-sandbox");
    assert_eq!(items[0]["vendor"], "kaiser");
    assert_eq!(
        items[0]["fhir_base"],
        "https://kpx-service-bus.kp.org/service/cdo/siae/healthplankpxv1rc/FHIR/api"
    );

    let v = stdout_json(kp(&home).args(["orgs", "search", "kaiser northwest", "--json"]));
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    let v = stdout_json(kp(&home).args(["orgs", "search", "sandbox", "--json"]));
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["items"][0]["id"], "kaiser-health-plan-sandbox");
    let v = stdout_json(kp(&home).args(["orgs", "search", "somewhere else", "--json"]));
    assert_eq!(v["items"], json!([]));

    kp(&home)
        .args(["orgs", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Kaiser Permanente health plan"));
}

#[test]
fn nothing_stored_reads_as_such_without_the_keychain() {
    let home = tmp();
    let v = stdout_json(kp(&home).args(["auth", "status", "--json"]));
    assert_eq!(v["schema"], "auth-status/v1");
    assert_eq!(v["required"], true);
    assert_eq!(v["authenticated"], false);
    assert_eq!(v["method"], "browser-session");
    assert_eq!(v["credential_in_keychain"], false);
    assert_eq!(v["clients"].as_array().unwrap().len(), 2);
    assert_eq!(v["patients"], json!([]));
    kp(&home)
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not stored"));
    kp(&home)
        .args(["auth", "logout", "--json"])
        .assert()
        .success();
    let v = stdout_json(kp(&home).args(["patients", "list", "--json"]));
    assert_eq!(v["schema"], "patient-list/v1");
    assert_eq!(v["items"], json!([]));
    // With nothing signed in, a read is an auth error before any request.
    for args in [
        vec!["records", "export", "--ndjson"],
        vec!["api", "GET", "ExplanationOfBenefit"],
    ] {
        kp(&home).args(&args).assert().code(3);
    }
}

#[test]
fn bad_arguments_are_refused_before_anything_else() {
    let home = tmp();
    for (args, code) in [
        (vec!["nope"], 2),
        (vec!["orgs", "search", "  "], 2),
        (
            vec!["auth", "set-credential", "--stdin", "--from-env", "X"],
            2,
        ),
        (vec!["auth", "login", "--org", "somewhere"], 4),
        (vec!["auth", "login", "--label", "has space"], 2),
        (vec!["auth", "login", "--timeout", "0"], 2),
        (vec!["records", "export"], 2),
        (vec!["records", "export", "--ndjson", "--json"], 2),
        (vec!["records", "export", "--ndjson", "-o", "x"], 2),
        (
            vec!["records", "export", "--ndjson", "--since", "2024-13-01"],
            2,
        ),
        (vec!["api", "POST", "Patient"], 2),
        (vec!["config", "set", "nope", "x"], 2),
        (vec!["config", "set", "environment", "sandbox"], 2),
        (vec!["config", "set", "redirect_port", "80"], 2),
        (
            vec!["config", "set", "fhir_base", "http://example.com/fhir"],
            2,
        ),
        (
            vec!["config", "set", "fhir_base", "https://example.com/fhir"],
            2,
        ),
        (vec!["patients", "list", "--org", "nope"], 4),
    ] {
        kp(&home).args(&args).assert().code(code);
    }
}

#[test]
fn a_malformed_pair_is_refused_without_echoing_it() {
    let home = tmp();
    let out = kp(&home)
        .args(["auth", "set-credential", "--stdin", "--json"])
        .write_stdin("has space-secret-value-1")
        .assert()
        .code(2);
    let err = String::from_utf8_lossy(&out.get_output().stderr).to_string()
        + &String::from_utf8_lossy(&out.get_output().stdout);
    assert!(!err.contains("secret-value-1"), "{err}");
    kp(&home)
        .args(["auth", "set-credential", "--from-env", "KP_TEST_UNSET_VAR"])
        .env_remove("KP_TEST_UNSET_VAR")
        .assert()
        .code(2);
}

#[test]
fn errors_are_one_json_document() {
    let home = tmp();
    let out = kp(&home)
        .args(["config", "set", "nope", "x", "--json"])
        .assert()
        .code(2);
    let v = json_of(&out.get_output().stdout);
    assert_eq!(v["error"]["code"], "usage");
}

#[test]
fn config_round_trips() {
    let home = tmp();
    kp(&home)
        .args(["config", "set", "redirect_port", "8123"])
        .assert()
        .success();
    let v = stdout_json(kp(&home).args(["config", "show", "--json"]));
    assert_eq!(v["redirect_uri"], "http://127.0.0.1:8123/callback");
    let v = stdout_json(kp(&home).args(["config", "path", "--json"]));
    let path = v["path"].as_str().unwrap().to_string();
    assert!(path.ends_with("kp/config.json"), "{path}");
    kp(&home)
        .args(["config", "unset", "redirect_port"])
        .assert()
        .success();
    let v = stdout_json(kp(&home).args(["config", "show", "--json"]));
    assert_eq!(v["redirect_uri"], "http://127.0.0.1:7469/callback");
}

#[test]
fn a_config_with_the_retired_environment_key_is_refused_until_unset() {
    let home = tmp();
    let dir = home.join("config/kp");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.json"), r#"{"environment":"sandbox"}"#).unwrap();
    kp(&home)
        .args(["auth", "login", "--org", "sandbox"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("kaiser-health-plan-sandbox"))
        .stderr(predicate::str::contains("kp config unset environment"));
    kp(&home)
        .args(["config", "unset", "environment"])
        .assert()
        .success();
    kp(&home).args(["config", "show"]).assert().success();
}

#[test]
fn no_color_is_accepted() {
    let home = tmp();
    kp(&home)
        .env("NO_COLOR", "1")
        .args(["orgs", "list"])
        .assert()
        .success();
}
