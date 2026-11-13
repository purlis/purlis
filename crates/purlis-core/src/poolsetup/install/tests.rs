// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const IMAGE: &str = "ghcr.io/purlis/purlis-cluster-operator@sha256:6a06111c3b0a8600feadb270aec771dc239bf407b25625f93f44ab00e11f4d3c";

fn nothing() -> Installed {
    Installed {
        definitions: false,
        key: false,
        operator: Operator::Absent,
        claims: Some((0, 0)),
    }
}

fn next_ids(parts: &[Part]) -> Vec<Option<ActionId>> {
    parts
        .iter()
        .map(|part| part.next.as_ref().map(|next| next.id))
        .collect()
}

#[test]
fn on_a_cluster_with_nothing_only_the_first_part_has_a_button() {
    let parts = parts(false, &nothing(), Some(IMAGE), "");
    assert_eq!(
        next_ids(&parts),
        [Some(ActionId::InstallUpstream), None, None, None]
    );
    assert!(parts.iter().all(|part| !part.done));
}

#[test]
fn each_part_is_offered_once_those_before_it_are_done() {
    let installed = Installed {
        definitions: true,
        ..nothing()
    };
    let parts = parts(true, &installed, Some(IMAGE), "");
    assert_eq!(
        next_ids(&parts),
        [None, None, Some(ActionId::MakeSigningKey), None]
    );
}

#[test]
fn a_signing_key_that_is_there_is_done_and_offers_no_button() {
    let installed = Installed {
        definitions: true,
        key: true,
        ..nothing()
    };
    let parts = parts(true, &installed, Some(IMAGE), "");
    let key = &parts[2];
    assert!(key.done);
    assert!(key.next.is_none());
    assert!(key.line.contains("never replaces"), "{}", key.line);
    assert_eq!(
        parts[3].next.as_ref().unwrap().id,
        ActionId::InstallOperator
    );
}

#[test]
fn the_operator_waits_for_its_image_by_digest() {
    let installed = Installed {
        definitions: true,
        key: true,
        ..nothing()
    };
    for image in [
        None,
        Some("ghcr.io/purlis/purlis-cluster-operator:main"),
        Some("ghcr.io/purlis/purlis-cluster-operator@sha256:abc"),
    ] {
        let parts = parts(true, &installed, image, "");
        assert_eq!(
            parts[3].next.as_ref().unwrap().id,
            ActionId::ChooseOperatorImage,
            "{image:?}"
        );
    }
}

#[test]
fn each_button_says_what_it_runs_from_the_same_source_as_see_what_this_does() {
    let installed = Installed {
        definitions: true,
        key: true,
        ..nothing()
    };
    let on = " --context kind-team";
    let parts = parts(true, &installed, Some(IMAGE), on);
    let command = parts[3].next.as_ref().unwrap().command.clone().unwrap();
    assert_eq!(command, by_hand(PartId::Operator, Some(IMAGE), on));
    assert!(command.contains(&format!("operator={IMAGE}")), "{command}");
    assert!(all_by_hand(Some(IMAGE), on).contains(&command));
}

#[test]
fn uninstalling_is_commands_that_say_how_many_people_and_claims_they_end() {
    let installed = Installed {
        claims: Some((3, 2)),
        ..nothing()
    };
    let undo = uninstall(&installed, "");
    assert!(
        undo.note.contains("3 claims by 2 people would end"),
        "{}",
        undo.note
    );
    assert!(undo.note.contains("never uninstalls"), "{}", undo.note);
    assert!(
        undo.text
            .starts_with("kubectl delete claims.runners.purlis.dev")
    );
    let none = uninstall(&nothing(), "");
    assert!(none.note.contains("end no one's chats"), "{}", none.note);
}

#[test]
fn a_new_signing_key_is_an_ed25519_key_in_pkcs8_pem() {
    let key = signing_key().unwrap();
    let body: String = key
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    assert!(key.starts_with(&format!("{}\n", pem_line("BEGIN"))));
    assert!(key.ends_with(&format!("{}\n", pem_line("END"))));
    let der = base64::engine::general_purpose::STANDARD
        .decode(body)
        .unwrap();
    // As the operator reads it (vouch.rs).
    ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&der).unwrap();
    assert_ne!(signing_key().unwrap(), key, "each key is new");
}

/// A stand-in kubectl recording each run's words and what it was handed, which says a Secret is
/// there where `secret_there` is.
#[cfg(unix)]
fn recording(dir: &std::path::Path, secret_there: bool) -> Kubectl {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("kubectl");
    let secret = if secret_there {
        "purlis-cluster-operator-signing-key   Opaque   1     3d"
    } else {
        ""
    };
    std::fs::write(
        &script,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> '{ran}'
case "$*" in
  *"get secrets"*) printf '%s' '{secret}' ;;
  *"create secret"*) cat > '{handed}' ;;
  *"-f -"*) cat > /dev/null ;;
  *"get namespace"*) ;;
  *) ;;
esac
"#,
            ran = dir.join("ran").display(),
            handed = dir.join("handed").display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Kubectl::at(script).with_context("kind-team")
}

#[cfg(unix)]
#[test]
fn a_signing_key_that_is_there_is_kept_and_no_key_is_made() {
    let dir = tempfile::tempdir().unwrap();
    apply(&recording(dir.path(), true), PartId::SigningKey, None).unwrap();
    let ran = std::fs::read_to_string(dir.path().join("ran")).unwrap();
    assert!(!ran.contains("create secret"), "{ran}");
    assert!(!dir.path().join("handed").exists());
}

#[cfg(unix)]
#[test]
fn a_new_signing_key_goes_to_the_cluster_on_stdin_and_nowhere_else() {
    let dir = tempfile::tempdir().unwrap();
    apply(&recording(dir.path(), false), PartId::SigningKey, None).unwrap();
    let ran = std::fs::read_to_string(dir.path().join("ran")).unwrap();
    let create = ran
        .lines()
        .find(|line| line.contains("create secret"))
        .expect("a secret made");
    assert!(
        create.ends_with(
            "--namespace purlis-system create secret generic \
             purlis-cluster-operator-signing-key --from-file=key.pem=/dev/stdin"
        ),
        "{create}"
    );
    // The key's words are on no command line, only what kubectl was handed.
    let handed = std::fs::read_to_string(dir.path().join("handed")).unwrap();
    assert!(handed.starts_with(&pem_line("BEGIN")));
    assert!(!ran.contains(PEM_LABEL));
    // Every run was on the chosen context.
    assert!(
        ran.lines()
            .all(|line| line.starts_with("--context kind-team ")),
        "{ran}"
    );
}

#[cfg(unix)]
#[test]
fn the_operator_is_refused_without_its_image_and_nothing_is_applied() {
    let dir = tempfile::tempdir().unwrap();
    let refused = apply(&recording(dir.path(), false), PartId::Operator, None).unwrap_err();
    assert!(
        refused.to_string().contains("nothing was applied"),
        "{refused}"
    );
    assert!(!dir.path().join("ran").exists());
}

#[cfg(unix)]
#[test]
fn upstream_is_applied_from_the_release_this_build_carries_and_waited_on() {
    let dir = tempfile::tempdir().unwrap();
    apply(&recording(dir.path(), false), PartId::Upstream, None).unwrap();
    let ran = std::fs::read_to_string(dir.path().join("ran")).unwrap();
    let lines: Vec<&str> = ran.lines().collect();
    assert!(
        lines[0].starts_with("--context kind-team apply --server-side -k /"),
        "{ran}"
    );
    assert!(
        lines[1].contains("rollout status deployment/agent-sandbox-controller"),
        "{ran}"
    );
    // The kustomization names the controller by the digest doctor checks for.
    assert!(RELEASE_KUSTOMIZATION.contains(purlis_cluster_doctor::CONTROLLER_DIGEST));
}
