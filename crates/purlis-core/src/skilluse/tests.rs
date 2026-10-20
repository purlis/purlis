// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_skill_use_is_logged_with_the_persona_or_null() {
    let dir = tempfile::tempdir().unwrap();
    let when = chrono::DateTime::parse_from_rfc3339("2026-05-04T11:32:17+00:00")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let p = record(dir.path(), "charter:persona", Some("ops"), when, "box").unwrap();
    record(dir.path(), "x", None, when, "box").unwrap();
    assert_eq!(record(dir.path(), " ", None, when, "box"), None);
    assert_eq!(
        std::fs::read_to_string(p).unwrap(),
        "{\"persona\": \"ops\", \"skill\": \"charter:persona\", \"ts\": \
         \"2026-05-04T11:32:17+00:00\"}\n{\"persona\": null, \"skill\": \"x\", \"ts\": \
         \"2026-05-04T11:32:17+00:00\"}\n"
    );
}

#[test]
fn no_skill_use_is_logged_in_a_project_this_purlis_may_not_write() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(crate::plane::MANIFEST),
        "schema = 2\nrequires = [{ feature = \"from-the-future\", since = \"9.0.0\" }]\n",
    )
    .unwrap();
    let when = chrono::Utc::now();
    assert_eq!(
        record(dir.path(), "charter:persona", Some("ops"), when, "box"),
        None
    );
    assert!(!dir.path().join("personas").exists());
}
