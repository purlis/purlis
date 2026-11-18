// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn view(command: &str) -> Value {
    serde_json::json!({"users": [{"name": "gke", "user": {"exec": {
        "command": command,
        "env": [{"name": "USE_GKE_GCLOUD_AUTH_PLUGIN", "value": "True"}]
    }}}]})
}

#[test]
fn the_plugin_is_read_from_the_context_s_exec_with_the_names_of_its_env_only() {
    let wanted = wanted(&view("gke-gcloud-auth-plugin")).unwrap();
    assert_eq!(wanted.command, "gke-gcloud-auth-plugin");
    assert_eq!(wanted.env, ["USE_GKE_GCLOUD_AUTH_PLUGIN"]);
}

#[test]
fn a_context_signing_in_by_a_token_names_no_plugin() {
    let token = serde_json::json!({"users": [{"name": "u", "user": {"token": "REDACTED"}}]});
    assert_eq!(wanted(&token), None);
}

#[cfg(unix)]
fn plugin_in(folder: &Path, name: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(folder).unwrap();
    let file = folder.join(name);
    std::fs::write(&file, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn a_plugin_in_a_known_folder_under_the_home_is_found_there() {
    let home = tempfile::tempdir().unwrap();
    plugin_in(&home.path().join(".krew/bin"), "kubelogin");
    assert_eq!(
        find("kubelogin", None, Some(home.path())),
        Where::In(home.path().join(".krew/bin"))
    );
}

#[cfg(unix)]
#[test]
fn a_plugin_in_no_known_folder_is_missing_until_its_folder_is_named() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    plugin_in(elsewhere.path(), "gke-gcloud-auth-plugin-not-installed-anywhere");
    let command = "gke-gcloud-auth-plugin-not-installed-anywhere";
    assert_eq!(find(command, None, Some(home.path())), Where::Missing);
    assert_eq!(
        find(command, Some(elsewhere.path()), Some(home.path())),
        Where::In(elsewhere.path().to_path_buf())
    );
}

#[test]
fn the_folder_is_never_searched_with_a_relative_one() {
    assert_eq!(find("x-plugin-xyz", Some(Path::new("relative/bin")), None), Where::Missing);
}

#[test]
fn a_missing_plugin_is_named_with_what_to_do() {
    assert_eq!(
        needs("gke-gcloud-auth-plugin"),
        "Needs gke-gcloud-auth-plugin, not found. Your kubeconfig signs in with it, and purlis \
         looks in the usual install folders only. Name the folder it is in."
    );
}
