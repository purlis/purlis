// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use serde_json::json;

fn keeping() -> serde_json::Value {
    json!({
        "Os": "linux",
        "Config": {
            "User": "10001:10001",
            "Env": ["HOME=/home/purlis"],
            "Entrypoint": ["/usr/bin/tini", "--"],
            "Cmd": ["/usr/local/bin/purlis", "serve"],
        }
    })
}

#[test]
fn an_image_that_keeps_the_contract_has_no_problem() {
    assert_eq!(config_problems(&keeping()), Vec::<String>::new());
}

#[test]
fn an_image_for_another_system_says_so() {
    let mut image = keeping();
    image["Os"] = json!("windows");
    assert!(config_problems(&image)[0].contains("Linux"));
}

#[test]
fn root_a_name_or_no_user_is_refused_since_a_cluster_checks_a_number() {
    for user in ["0:0", "root", ""] {
        let mut image = keeping();
        image["Config"]["User"] = json!(user);
        let said = config_problems(&image);
        assert_eq!(said.len(), 1, "{user}: {said:?}");
        assert!(said[0].contains("number other than 0"), "{said:?}");
    }
}

#[test]
fn no_absolute_home_is_a_problem() {
    let mut image = keeping();
    image["Config"]["Env"] = json!(["HOME=relative"]);
    assert!(config_problems(&image)[0].contains("HOME"));
}

#[test]
fn its_command_has_to_end_in_purlis_serve() {
    let mut image = keeping();
    image["Config"]["Cmd"] = json!(["/usr/local/bin/purlis", "serve", "--more"]);
    assert!(config_problems(&image)[0].contains("end in purlis serve"));
}

#[test]
fn purlis_serve_as_pid_one_is_refused_since_nothing_reaps() {
    let mut image = keeping();
    image["Config"]["Entrypoint"] = json!([]);
    assert!(config_problems(&image)[0].contains("init that reaps"));
}

#[test]
fn an_image_is_looked_in_only_as_a_user_other_than_root() {
    assert_eq!(declared_user(&keeping()), Some("10001:10001".into()));
    for user in ["0", "0:0", "root", "", "10001:staff"] {
        let mut image = keeping();
        image["Config"]["User"] = json!(user);
        let said = declared_user(&image);
        assert_eq!(said, None, "{user}");
    }
}

#[test]
fn what_the_image_says_reads_as_a_sentence() {
    assert_eq!(
        sentence("there is no bash on PATH"),
        "There is no bash on PATH."
    );
    assert_eq!(sentence("Done."), "Done.");
    assert_eq!(sentence(""), "");
}

#[test]
fn a_user_with_no_group_runs_as_its_number() {
    let mut image = keeping();
    image["Config"]["User"] = json!("10001");
    assert_eq!(declared_user(&image), Some("10001".into()));
}
