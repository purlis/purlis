// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

use purlis_session_protocol::hostlink::LinkKey;
use ring::signature::{Ed25519KeyPair, KeyPair};

/// The worked example the operator signs in its own tests (`fixtures/runner-link-key/`).
fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/runner-link-key")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
}

fn claimed() -> Claimed {
    Claimed {
        namespace: "ada".into(),
        name: "ada-1".into(),
        uid: "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60".into(),
        requester: "ada@example.com".into(),
    }
}

fn operator_key() -> OperatorKey {
    OperatorKey::from_spelled(fixture("operator-key.txt").trim()).unwrap()
}

/// The worked example's cluster: the uid of its `kube-system` namespace (#3305).
fn cluster() -> String {
    fixture("cluster.txt").trim().to_owned()
}

/// A pin on `key` in the worked example's cluster.
fn pin(key: OperatorKey) -> PoolPin {
    PoolPin {
        key,
        cluster: cluster(),
    }
}

/// `text` signed as `signature`, outside a roll.
fn signed(text: &str, signature: &str) -> Signed {
    Signed {
        text: text.to_owned(),
        signature: signature.to_owned(),
        next_signature: None,
    }
}

/// An operator of the test's own: its key, and a signer.
pub(crate) struct Operator(Ed25519KeyPair);

impl Operator {
    pub(crate) fn new() -> Operator {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        Operator(Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap())
    }

    pub(crate) fn key(&self) -> OperatorKey {
        OperatorKey::from_spelled(&format!("ed25519:{}", hex_of(self.0.public_key().as_ref())))
            .unwrap()
    }

    pub(crate) fn sign(&self, text: &str) -> String {
        hex_of(self.0.sign(text.as_bytes()).as_ref())
    }
}

/// Ada's runner namespace: `purlis-runners-` and the first 16 hex digits of the SHA-256 of
/// `ada@example.com`, as the operator works it out (#2055).
const RUNNER_NAMESPACE: &str = "purlis-runners-b5fc85e55755f9e0";

#[test]
fn a_requesters_runner_namespace_is_worked_out_as_the_operator_does() {
    assert_eq!(runner_namespace("ada@example.com"), RUNNER_NAMESPACE);
    assert_eq!(
        runner_namespace("kubernetes-admin"),
        "purlis-runners-b2f220d99d68735b"
    );
}

fn vouched(key: PublicKey) -> Vouched {
    Vouched {
        cluster: cluster(),
        namespace: "ada".into(),
        claim: "ada-1".into(),
        claim_uid: claimed().uid,
        runner_namespace: RUNNER_NAMESPACE.into(),
        pod: "ada-1".into(),
        key,
        requester: "ada@example.com".into(),
        until: now() + 600,
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn the_operators_worked_example_is_verified_and_read() {
    let read = operator_key()
        .verified(&fixture("statement.txt"), fixture("signature.txt").trim())
        .expect("the operator's own example verifies");
    assert_eq!(read.cluster, cluster());
    assert_eq!(read.namespace, "ada");
    assert_eq!(read.claim, "ada-1");
    assert_eq!(read.runner_namespace, RUNNER_NAMESPACE);
    assert_eq!(read.pod, "ada-1");
    assert_eq!(read.requester, "ada@example.com");
    assert_eq!(read.until, 1_798_761_600);
    assert_eq!(
        hex_of(read.key.as_bytes()),
        "2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f70819"
    );
    assert_eq!(
        read.text(),
        fixture("statement.txt"),
        "written back byte for byte"
    );
}

#[test]
fn a_statement_signed_by_another_key_than_the_pinned_one_is_refused() {
    let stranger = Operator::new();
    let text = fixture("statement.txt");
    let refused = operator_key()
        .verified(&text, &stranger.sign(&text))
        .unwrap_err();
    assert!(matches!(refused, Refused::NotVouched(_)), "{refused:?}");
}

#[test]
fn a_statement_changed_after_it_was_signed_is_refused() {
    let text = fixture("statement.txt");
    let substituted = text.replace(
        "key: 2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f70819",
        "key: 1111111111111111111111111111111111111111111111111111111111111111",
    );
    assert_ne!(substituted, text);
    let refused = operator_key()
        .verified(&substituted, fixture("signature.txt").trim())
        .unwrap_err();
    assert!(matches!(refused, Refused::NotVouched(_)), "{refused:?}");
}

#[test]
fn a_statement_purlis_does_not_read_is_refused_even_when_signed() {
    let operator = Operator::new();
    let good = vouched(LinkKey::generate().unwrap().public()).text();
    let unread = [
        good.replace(STATEMENT_HEADER, "purlis runner link key v1"),
        good.replace(STATEMENT_HEADER, "purlis runner link key v2"),
        good.replace(STATEMENT_HEADER, "purlis runner link key v4"),
        // A v2 statement, which names no cluster, is one purlis no longer reads (#3305).
        good.replace(STATEMENT_HEADER, "purlis runner link key v2")
            .replace(&format!("cluster: {}\n", cluster()), ""),
        good.replace(&format!("cluster: {}\n", cluster()), ""),
        good.replace(&format!("cluster: {}", cluster()), "cluster: kube-system"),
        // A v1 statement's pod uid line is one purlis no longer reads.
        good.replace(
            "pod: ada-1\n",
            "pod: ada-1\npod uid: 0a1b2c3d-4e5f-4061-8728-394a5b6c7d8e\n",
        ),
        good.replace("pod: ada-1\n", "pod: ada-1\nextra: line\n"),
        good.replace("requester: ada@example.com", "requester: ada@example.com\r"),
        good.replace("requester: ada@example.com", "requester:  ada@example.com"),
        good.replace("claim uid: ", "claim uid:"),
        good.trim_end().to_owned(),
        good.replace("key: ", "key: 0"),
        good.replace("pod: ada-1", "pod: --server=https://elsewhere"),
        good.replace("pod: ada-1", "pod: Ada-1"),
        good.replace("namespace: ada", "namespace: a.b"),
        good.replace(
            &format!("runner namespace: {RUNNER_NAMESPACE}"),
            "runner namespace: --all-namespaces",
        ),
        // A statement without its runner namespace is one purlis no longer reads (#2055).
        good.replace(&format!("runner namespace: {RUNNER_NAMESPACE}\n"), ""),
        good.replace("claim uid: 6f1c2b8e", "claim uid: 6F1C2B8E"),
        good.replace(
            "claim uid: 6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60",
            "claim uid: a-claim",
        ),
    ];
    for text in unread {
        let refused = operator.key().verified(&text, &operator.sign(&text));
        assert!(
            matches!(refused, Err(Refused::NotVouched(_))),
            "{text:?}: {refused:?}"
        );
    }
    assert!(
        operator
            .key()
            .verified(&good, &operator.sign(&good))
            .is_ok()
    );
}

#[test]
fn a_statement_about_another_claim_or_requester_is_refused() {
    let operator = Operator::new();
    let key = LinkKey::generate().unwrap().public();
    let others = [
        Vouched {
            claim: "ada-2".into(),
            ..vouched(key)
        },
        Vouched {
            claim_uid: "99999999-9999-4999-8999-999999999999".into(),
            ..vouched(key)
        },
        Vouched {
            namespace: "grace".into(),
            ..vouched(key)
        },
        Vouched {
            requester: "grace@example.com".into(),
            ..vouched(key)
        },
        // Its runner in any namespace but its requester's own (#2055).
        Vouched {
            runner_namespace: "ada".into(),
            ..vouched(key)
        },
        Vouched {
            runner_namespace: runner_namespace("grace@example.com"),
            ..vouched(key)
        },
    ];
    for other in others {
        let text = other.text();
        let refused = checked(
            &pin(operator.key()),
            &claimed(),
            &signed(&text, &operator.sign(&text)),
        );
        assert!(matches!(refused, Err(Refused::NotVouched(_))), "{other:?}");
    }
    let text = vouched(key).text();
    assert!(
        checked(
            &pin(operator.key()),
            &claimed(),
            &signed(&text, &operator.sign(&text))
        )
        .is_ok()
    );
}

#[test]
fn an_operator_key_is_read_only_in_its_one_spelling() {
    let spelled = fixture("operator-key.txt");
    let key = OperatorKey::from_spelled(spelled.trim()).unwrap();
    assert_eq!(key.spelled(), spelled.trim());
    for bad in [
        "79b5562e8fe654f94078b112e8a98ba7901f853ae695bed7e0e3910bad049664",
        "ed25519:79B5562E8FE654F94078B112E8A98BA7901F853AE695BED7E0E3910BAD049664",
        "ed25519:79b5562e",
        "rsa:79b5562e8fe654f94078b112e8a98ba7901f853ae695bed7e0e3910bad049664",
    ] {
        assert!(OperatorKey::from_spelled(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_runner_answering_another_key_than_the_vouched_one_is_refused_as_a_changed_key() {
    let ours = LinkKey::generate().unwrap().public();
    let answered = Pinned {
        device: "01JB0RUNNER000000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    };
    assert_eq!(
        agreed(&claimed(), &vouched(ours), answered.clone()),
        Err(Refused::KeyChanged {
            claim: "ada/ada-1".into()
        })
    );
    let same = Pinned {
        key: ours,
        ..answered
    };
    assert_eq!(agreed(&claimed(), &vouched(ours), same.clone()), Ok(same));
}

#[test]
fn a_statement_naming_another_pod_or_key_than_the_paired_ones_refuses_the_link() {
    let key = LinkKey::generate().unwrap().public();
    let paired = Paired {
        claimed: claimed(),
        vouched: vouched(key),
        pinned: Pinned {
            device: "01JB0RUNNER000000000000000".into(),
            key,
        },
    };
    assert_eq!(held_to(&paired, &vouched(key)), Ok(()));
    let changed = [
        Vouched {
            pod: "ada-2".into(),
            ..vouched(key)
        },
        Vouched {
            runner_namespace: "purlis-runners-0000000000000000".into(),
            ..vouched(key)
        },
        vouched(LinkKey::generate().unwrap().public()),
    ];
    for now in changed {
        assert!(matches!(
            held_to(&paired, &now),
            Err(Refused::KeyChanged { .. })
        ));
    }
}

/// A stand-in for kubectl: a script that prints `answer` and records its arguments.
#[cfg(unix)]
fn kubectl_answering(dir: &std::path::Path, answer: &str) -> Kubectl {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("kubectl");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\ncat > /dev/null\nprintf '%s\\n' '{answer}'\n",
            dir.join("args").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Kubectl::at(script).with_context("kind-test")
}

/// **#4281**: what kubectl prints past what purlis reads: a limit that says it is fixed.
#[cfg(unix)]
#[test]
fn kubectl_printing_past_what_purlis_reads_says_the_limit_is_fixed() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("kubectl");
    std::fs::write(
        &script,
        "#!/bin/sh\ncat > /dev/null\nhead -c 2097152 /dev/zero | tr '\\000' x\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let refused = Kubectl::at(script)
        .with_context("kind-test")
        .pool("default")
        .unwrap_err()
        .to_string();
    assert!(refused.contains("printed more"), "{refused}");
    let fixed = purlis_session_protocol::fixedlimits::RUNNER_OUTPUT.said();
    assert!(refused.contains(&fixed), "{refused}");
}

#[cfg(unix)]
#[test]
fn pairing_execs_into_the_vouched_pod_and_refuses_a_runner_answering_another_key() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    };
    let theirs = Pinned {
        device: "01JB0RUNNER000000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    };
    let kubectl = kubectl_answering(dir.path(), &crate::runnerlink::to_line(&theirs));

    let refused = pair(
        &kubectl,
        &claimed(),
        &vouched(LinkKey::generate().unwrap().public()),
        &desktop,
    )
    .unwrap_err();
    assert!(matches!(refused, Refused::KeyChanged { .. }), "{refused:?}");

    let paired = pair(&kubectl, &claimed(), &vouched(theirs.key), &desktop).unwrap();
    assert_eq!(paired.pinned, theirs);
    let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        [
            "--context",
            "kind-test",
            "exec",
            "-i",
            "--namespace",
            RUNNER_NAMESPACE,
            "--container",
            "runner",
            "pod/ada-1",
            "--",
            "/usr/local/bin/purlis",
            "bridge",
            "--pair",
            "--key-made-at-start",
            "/run/purlis-link/link-key",
        ]
    );
}

#[test]
fn the_link_key_path_is_the_one_the_operators_template_names() {
    assert_eq!(KEY_MADE_AT_START, fixture("link-key-path.txt").trim());
}

#[test]
fn a_kept_claims_pod_made_again_with_the_key_it_was_paired_with_is_the_same_runner() {
    // The pod's uid is not in the statement: a kept claim's pairing is the claim's (#2057).
    let key = LinkKey::generate().unwrap().public();
    let paired = Paired {
        claimed: claimed(),
        vouched: vouched(key),
        pinned: Pinned {
            device: "01JB0RUNNER000000000000000".into(),
            key,
        },
    };
    let remade = Vouched::from_text(&vouched(key).text()).unwrap();
    assert_eq!(held_to(&paired, &remade), Ok(()));
    assert!(!vouched(key).text().contains("pod uid"));
}

#[test]
fn purlis_keeps_a_chats_runner_unless_it_is_a_short_dispatched_task() {
    use crate::harness_declaration::SignInKind;
    let chat = |dispatched, sign_in, keep_work| ChatOnARunner {
        dispatched,
        sign_in,
        keep_work,
    };
    for (asked, kept) in [
        (chat(false, None, None), Keeping::Kept),
        (chat(false, Some(SignInKind::ApiKey), None), Keeping::Kept),
        (chat(true, None, None), Keeping::Fresh),
        (chat(true, Some(SignInKind::ApiKey), None), Keeping::Fresh),
        (chat(true, Some(SignInKind::Cloud), None), Keeping::Fresh),
        // A subscription's sign-in lives on the kept volume.
        (
            chat(true, Some(SignInKind::Subscription), None),
            Keeping::Kept,
        ),
        // The person's switch decides, either way.
        (chat(true, None, Some(true)), Keeping::Kept),
        (chat(false, None, Some(false)), Keeping::Fresh),
        (
            chat(true, Some(SignInKind::Subscription), Some(false)),
            Keeping::Fresh,
        ),
    ] {
        assert_eq!(kept_for(&asked), kept, "{asked:?}");
    }
    assert_eq!(kept_for(&ChatOnARunner::default()), Keeping::Kept);
}

#[test]
fn a_kept_claim_says_so_and_a_fresh_one_reads_as_every_claim_did() {
    let kept = claim_definition("ada", "ada-1", "team", "ada@example.com", Keeping::Kept);
    assert_eq!(
        kept["spec"],
        serde_json::json!({ "pool": "team", "requester": "ada@example.com", "kept": true })
    );
    let fresh = claim_definition("ada", "ada-1", "team", "ada@example.com", Keeping::Fresh);
    assert_eq!(
        fresh["spec"],
        serde_json::json!({ "pool": "team", "requester": "ada@example.com" })
    );
    assert_eq!(fresh["apiVersion"], "runners.purlis.dev/v1alpha1");
    assert_eq!(fresh["kind"], "Claim");
    assert_eq!(
        fresh["metadata"],
        serde_json::json!({ "name": "ada-1", "namespace": "ada" })
    );
}

#[test]
fn a_statement_holds_only_until_the_time_it_names_and_never_past_fifteen_minutes() {
    let operator = Operator::new();
    let key = LinkKey::generate().unwrap().public();
    let text = vouched(key).text();
    let signature = operator.sign(&text);
    let until = vouched(key).until;
    let at = |now| {
        checked_at(
            &pin(operator.key()),
            &claimed(),
            &signed(&text, &signature),
            now,
        )
    };
    assert!(at(until - 1).is_ok());
    for now in [until, until + 1] {
        let refused = at(now).unwrap_err();
        assert!(
            matches!(&refused, Refused::NotVouched(why) if why.contains("no longer holds")),
            "{refused:?}"
        );
    }
    assert!(
        at(until - MOST_AHEAD).is_ok(),
        "ten minutes as signed, and five of clock"
    );
    assert_eq!(MOST_AHEAD, 900);
    let refused = at(until - MOST_AHEAD - 1).unwrap_err();
    assert!(
        matches!(&refused, Refused::NotVouched(why) if why.contains("more than fifteen minutes")),
        "{refused:?}"
    );
    // The operator's worked example, long past.
    let fixture_at = |now| {
        checked_at(
            &pin(operator_key()),
            &claimed(),
            &signed(&fixture("statement.txt"), fixture("signature.txt").trim()),
            now,
        )
    };
    assert!(fixture_at(1_798_761_000).is_ok());
    assert!(fixture_at(1_798_761_600).is_err());
}

#[test]
fn a_statement_of_another_version_says_both_are_to_be_of_one_release() {
    let operator = Operator::new();
    let text = vouched(LinkKey::generate().unwrap().public())
        .text()
        .replace(STATEMENT_HEADER, "purlis runner link key v1");
    let refused = operator
        .key()
        .verified(&text, &operator.sign(&text))
        .unwrap_err();
    assert!(
        matches!(&refused, Refused::NotVouched(why) if why.contains("version v1") && why.contains("one release")),
        "{refused:?}"
    );
}

fn next_operator_key() -> OperatorKey {
    OperatorKey::from_spelled(fixture("next-operator-key.txt").trim()).unwrap()
}

/// Now, as a desktop reads the worked example's roll: within its seven days.
const DURING_THE_ROLL: u64 = 1_798_761_600 - 3 * 24 * 3600;

/// A Pool's status in the worked example's roll (`fixtures/runner-link-key/`).
fn rolling() -> PoolSeen {
    PoolSeen {
        signing_key: Some(fixture("operator-key.txt").trim().to_owned()),
        cluster: Some(cluster()),
        next_signing_key: Some(fixture("next-operator-key.txt").trim().to_owned()),
        next_signing_key_vouch: Some((
            fixture("key-roll.txt"),
            fixture("key-roll-signature.txt").trim().to_owned(),
        )),
        mints_in_cluster: false,
    }
}

#[test]
fn a_statement_made_in_another_cluster_than_the_pinned_one_is_refused() {
    let operator = Operator::new();
    let key = LinkKey::generate().unwrap().public();
    let elsewhere = Vouched {
        cluster: "99999999-9999-4999-8999-999999999999".into(),
        ..vouched(key)
    };
    let text = elsewhere.text();
    let refused = checked(
        &pin(operator.key()),
        &claimed(),
        &signed(&text, &operator.sign(&text)),
    )
    .unwrap_err();
    assert!(
        matches!(&refused, Refused::NotVouched(why) if why.contains("another cluster")),
        "{refused:?}"
    );
}

#[test]
fn a_pools_pin_is_its_operators_key_and_its_cluster() {
    let seen = PoolSeen::from_status(&serde_json::json!({
        "signingKey": fixture("operator-key.txt").trim(),
        "cluster": cluster(),
    }));
    assert_eq!(
        PoolPin::first("team", &seen),
        Ok(pin(operator_key())),
        "{seen:?}"
    );
    // An operator that names no cluster is of an older release.
    let older = PoolSeen {
        cluster: None,
        ..seen.clone()
    };
    let refused = PoolPin::first("team", &older).unwrap_err();
    assert!(
        matches!(&refused, Refused::NotVouched(why) if why.contains("one release")),
        "{refused:?}"
    );
    assert!(
        PoolPin::first(
            "team",
            &PoolSeen {
                signing_key: None,
                ..seen
            }
        )
        .is_err()
    );
    assert_eq!(
        PoolSeen::from_status(&serde_json::json!({
            "signingKey": "ed25519:00",
            "cluster": "c",
            "nextSigningKey": "ed25519:11",
            "nextSigningKeyVouch": { "statement": "s", "signature": "g" },
        })),
        PoolSeen {
            signing_key: Some("ed25519:00".into()),
            cluster: Some("c".into()),
            next_signing_key: Some("ed25519:11".into()),
            next_signing_key_vouch: Some(("s".into(), "g".into())),
            mints_in_cluster: false,
        }
    );
}

#[test]
fn the_operators_worked_example_of_a_roll_is_read() {
    let roll = KeyRoll::from_text(&fixture("key-roll.txt")).expect("read");
    assert_eq!(roll.cluster, cluster());
    assert_eq!(roll.from, operator_key());
    assert_eq!(roll.to, next_operator_key());
    assert_eq!(roll.until, 1_798_761_600);
    assert_eq!(roll.text(), fixture("key-roll.txt"));
    let good = fixture("key-roll.txt");
    for bad in [
        good.replace(ROLL_HEADER, "purlis cluster operator key roll v2"),
        good.replace("until: ", "until:"),
        good.replace("from: ed25519:", "from: "),
        good.replace(&format!("cluster: {}", cluster()), "cluster: kube-system"),
        good.trim_end().to_owned(),
        format!("{good}extra: line\n"),
    ] {
        assert!(KeyRoll::from_text(&bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_desktop_pinned_to_the_current_key_moves_its_pin_once_that_key_vouches_for_the_next() {
    let mut pinned = pin(operator_key());
    assert_eq!(
        pinned.followed(&rolling(), DURING_THE_ROLL),
        Ok(Following::Moves(pin(next_operator_key())))
    );
    assert_eq!(pinned.follow(&rolling(), DURING_THE_ROLL), Ok(true));
    assert_eq!(pinned, pin(next_operator_key()));
    // Moved, it stays through the rest of the roll and after it.
    assert_eq!(pinned.follow(&rolling(), DURING_THE_ROLL), Ok(false));
    let promoted = PoolSeen {
        signing_key: Some(fixture("next-operator-key.txt").trim().to_owned()),
        next_signing_key: None,
        next_signing_key_vouch: None,
        ..rolling()
    };
    assert_eq!(
        pinned.followed(&promoted, DURING_THE_ROLL),
        Ok(Following::Stays)
    );
    // No roll: it stays.
    let quiet = PoolSeen {
        next_signing_key: None,
        next_signing_key_vouch: None,
        ..rolling()
    };
    assert_eq!(
        pin(operator_key()).followed(&quiet, DURING_THE_ROLL),
        Ok(Following::Stays)
    );
}

#[test]
fn a_desktop_that_missed_the_roll_asks_again() {
    let promoted = PoolSeen {
        signing_key: Some(fixture("next-operator-key.txt").trim().to_owned()),
        next_signing_key: None,
        next_signing_key_vouch: None,
        ..rolling()
    };
    let mut pinned = pin(operator_key());
    let refused = pinned.follow(&promoted, DURING_THE_ROLL).unwrap_err();
    assert!(
        matches!(&refused, Refused::NotVouched(why) if why.contains("add the pool again")),
        "{refused:?}"
    );
    assert_eq!(pinned, pin(operator_key()), "a refusal leaves the pin");
}

#[test]
fn a_roll_the_pinned_key_does_not_vouch_for_is_not_followed() {
    let stranger = Operator::new();
    let pinned = pin(operator_key());
    let not_followed = |seen: &PoolSeen, now| {
        let followed = pinned.followed(seen, now);
        assert!(
            matches!(followed, Ok(Following::NotFollowed(_))),
            "{seen:?} at {now}: {followed:?}"
        );
    };
    // Past its until, or holding more than seven days and an hour from now.
    not_followed(&rolling(), 1_798_761_600);
    not_followed(&rolling(), 1_798_761_600 - ROLL_MOST_AHEAD - 1);
    // Signed by another key than the pinned one.
    let text = fixture("key-roll.txt");
    not_followed(
        &PoolSeen {
            next_signing_key_vouch: Some((text.clone(), stranger.sign(&text))),
            ..rolling()
        },
        DURING_THE_ROLL,
    );
    // No word at all.
    not_followed(
        &PoolSeen {
            next_signing_key_vouch: None,
            ..rolling()
        },
        DURING_THE_ROLL,
    );
    // A word for another next key than the one the Pool names.
    not_followed(
        &PoolSeen {
            next_signing_key: Some(stranger.key().spelled()),
            ..rolling()
        },
        DURING_THE_ROLL,
    );
    // A word the pinned key signed for another cluster or from another key.
    let operator = Operator::new();
    for word in [
        KeyRoll {
            cluster: "99999999-9999-4999-8999-999999999999".into(),
            from: operator.key(),
            to: next_operator_key(),
            until: 1_798_761_600,
        },
        KeyRoll {
            cluster: cluster(),
            from: next_operator_key(),
            to: next_operator_key(),
            until: 1_798_761_600,
        },
    ] {
        let text = word.text();
        let seen = PoolSeen {
            signing_key: Some(operator.key().spelled()),
            next_signing_key_vouch: Some((text.clone(), operator.sign(&text))),
            ..rolling()
        };
        let followed = pin(operator.key()).followed(&seen, DURING_THE_ROLL);
        assert!(
            matches!(followed, Ok(Following::NotFollowed(_))),
            "{word:?}: {followed:?}"
        );
    }
}

#[test]
fn a_pool_in_another_cluster_than_the_pinned_one_is_refused() {
    let elsewhere = PoolSeen {
        cluster: Some("99999999-9999-4999-8999-999999999999".into()),
        ..rolling()
    };
    let mut pinned = pin(operator_key());
    let refused = pinned.follow(&elsewhere, DURING_THE_ROLL).unwrap_err();
    assert!(
        matches!(&refused, Refused::NotVouched(why) if why.contains("another cluster")),
        "{refused:?}"
    );
    assert_eq!(pinned, pin(operator_key()));
}

#[test]
fn during_a_roll_a_moved_pin_reads_the_next_keys_signature() {
    let during = Signed {
        text: fixture("statement.txt"),
        signature: fixture("signature.txt").trim().to_owned(),
        next_signature: Some(fixture("next-signature.txt").trim().to_owned()),
    };
    // Pinned to either key, the statement is read.
    assert!(pin(operator_key()).verified(&during).is_ok());
    assert!(pin(next_operator_key()).verified(&during).is_ok());
    // Moved, a statement the next key did not sign is refused.
    let unsigned_by_next = Signed {
        next_signature: None,
        ..during.clone()
    };
    assert!(
        pin(next_operator_key())
            .verified(&unsigned_by_next)
            .is_err()
    );
    let wrong_next = Signed {
        next_signature: Some(fixture("signature.txt").trim().to_owned()),
        ..during
    };
    assert!(pin(next_operator_key()).verified(&wrong_next).is_err());
}

/// **A cluster of the test's own, behind a stand-in for kubectl** (#3292): it answers who the
/// kubeconfig is, a Pool naming its operator's key, a Claim made, the Claim's statement as its
/// operator signed it, a runner's pairing, and the kubeconfig's contexts, each from a file the
/// test can change, and records every run's words and environment.
#[cfg(unix)]
pub(crate) struct FakeCluster {
    pub(crate) dir: tempfile::TempDir,
    pub(crate) operator: Operator,
    pub(crate) runner: Pinned,
}

/// The context the stand-in's kubeconfig gives whole (`config view --minify --raw --flatten`).
pub(crate) const CONTEXT_KEPT: &str = r#"{"apiVersion": "v1", "kind": "Config", "current-context": "team", "users": [{"name": "ada", "user": {}}]}"#;

/// The uid the stand-in gives every claim it makes.
pub(crate) const CLAIM_UID: &str = "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60";

/// The stand-in's cluster: the uid of its `kube-system` namespace.
pub(crate) const CLUSTER: &str = "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b";

/// The device id of the desktop the stand-in's claims are marked as.
pub(crate) const OURS_DEVICE: &str = "01JA0DESKT0P00000000000000";

/// Who the stand-in's kubeconfig signs in as.
pub(crate) const REQUESTER: &str = "ada@example.com";

#[cfg(unix)]
impl FakeCluster {
    pub(crate) fn new() -> FakeCluster {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let operator = Operator::new();
        let runner = Pinned {
            device: "01JB0RUNNER000000000000000".into(),
            key: LinkKey::generate().unwrap().public(),
        };
        let d = dir.path().display().to_string();
        let script = format!(
            "#!/bin/sh\nd='{d}'\nprintf '%s ' \"$@\" >> \"$d/calls\"\nprintf '\\n' >> \"$d/calls\"\n\
             printf '%s|%s\\n' \"${{KUBECONFIG-unset}}\" \"$PATH\" >> \"$d/env\"\ncat > /dev/null\n\
             case \"$*\" in\n\
             *'auth whoami'*) cat \"$d/whoami.json\";;\n\
             *'get pools.runners.purlis.dev'*) cat \"$d/pool.json\";;\n\
             *'create --filename'*) cat \"$d/created.json\";;\n\
             *'get claims.runners.purlis.dev'*) cat \"$d/claim.json\";;\n\
             *'get pods'*) cat \"$d/pod.json\";;\n\
             *'exec -i'*) cat \"$d/paired.txt\";;\n\
             *'delete claims'*) echo deleted;;\n\
             *'--raw'*) cat \"$d/context.yaml\";;\n\
             *'--minify'*) cat \"$d/minify.json\";;\n\
             *'config view'*) cat \"$d/view.json\";;\n\
             *) exit 1;;\nesac\n"
        );
        let at = dir.path().join("bin");
        std::fs::create_dir(&at).unwrap();
        std::fs::write(at.join("kubectl"), script).unwrap();
        std::fs::set_permissions(at.join("kubectl"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let cluster = FakeCluster {
            dir,
            operator,
            runner,
        };
        cluster.write(
            "whoami.json",
            &serde_json::json!({"status": {"userInfo": {"username": REQUESTER}}}).to_string(),
        );
        cluster.names_key(&cluster.operator.key());
        cluster.write(
            "created.json",
            &serde_json::json!({"metadata": {"uid": CLAIM_UID}}).to_string(),
        );
        cluster.write("claim.json", "{}");
        cluster.write("pod.json", r#"{"metadata": {"uid": "pod-uid-1"}}"#);
        cluster.write("context.yaml", CONTEXT_KEPT);
        cluster.write("paired.txt", &crate::runnerlink::to_line(&cluster.runner));
        cluster.write(
            "minify.json",
            &serde_json::json!({"contexts": [{"name": "team", "context": {"namespace": "ada"}}]})
                .to_string(),
        );
        cluster.write(
            "view.json",
            &serde_json::json!({
                "current-context": "team",
                "contexts": [{"name": "team"}, {"name": "kind-test"}],
            })
            .to_string(),
        );
        cluster
    }

    pub(crate) fn write(&self, name: &str, text: &str) {
        std::fs::write(self.dir.path().join(name), text).unwrap();
    }

    /// The Pool's status names `key` as its operator's, in the stand-in's cluster.
    pub(crate) fn names_key(&self, key: &OperatorKey) {
        self.write(
            "pool.json",
            &serde_json::json!({"status": {"signingKey": key.spelled(), "cluster": CLUSTER}})
                .to_string(),
        );
    }

    /// The Pool's spec says `forge`, its status as the pin says (#4204).
    pub(crate) fn pool_says_forge(&self, forge: serde_json::Value) {
        self.write(
            "pool.json",
            &serde_json::json!({
                "spec": {"forge": forge},
                "status": {"signingKey": self.operator.key().spelled(), "cluster": CLUSTER},
            })
            .to_string(),
        );
    }

    /// The Pool's status rolls its operator's key to `next`'s, the stand-in's operator vouching
    /// for it until `until` (#3305).
    pub(crate) fn rolls_to(&self, next: &Operator, until: u64) {
        let roll = KeyRoll {
            cluster: CLUSTER.into(),
            from: self.operator.key(),
            to: next.key(),
            until,
        }
        .text();
        self.write(
            "pool.json",
            &serde_json::json!({"status": {
                "signingKey": self.operator.key().spelled(),
                "cluster": CLUSTER,
                "nextSigningKey": next.key().spelled(),
                "nextSigningKeyVouch": {"statement": roll, "signature": self.operator.sign(&roll)},
            }})
            .to_string(),
        );
    }

    /// The pin a desktop adding the stand-in's pool takes.
    pub(crate) fn pin(&self) -> PoolPin {
        PoolPin {
            key: self.operator.key(),
            cluster: CLUSTER.into(),
        }
    }

    /// The operator vouches for the runner of claim `namespace`/`claim` in pod `pod` with `key`,
    /// signed by its key and, during a roll, by `next` too.
    pub(crate) fn vouch_signed(
        &self,
        namespace: &str,
        claim: &str,
        pod: &str,
        key: PublicKey,
        next: Option<&Operator>,
    ) {
        let text = Vouched {
            cluster: CLUSTER.into(),
            namespace: namespace.into(),
            claim: claim.into(),
            claim_uid: CLAIM_UID.into(),
            runner_namespace: runner_namespace(REQUESTER),
            pod: pod.into(),
            key,
            requester: REQUESTER.into(),
            until: now() + 600,
        }
        .text();
        let mut link_key = serde_json::json!({
            "statement": text,
            "signature": self.operator.sign(&text),
        });
        if let Some(next) = next {
            link_key["nextSignature"] = next.sign(&text).into();
        }
        self.write(
            "claim.json",
            &serde_json::json!({
                "metadata": {
                    "uid": CLAIM_UID,
                    "annotations": {DESKTOP_ANNOTATION: desktop_label(OURS_DEVICE)},
                },
                "spec": {"requester": REQUESTER},
                "status": {
                    "linkKey": link_key,
                    "conditions": [{"type": "KeyPublished", "status": "True", "reason": "Vouched"}],
                },
            })
            .to_string(),
        );
    }

    /// [`Self::vouch_signed`], outside a roll.
    pub(crate) fn vouch(&self, namespace: &str, claim: &str, pod: &str, key: PublicKey) {
        self.vouch_signed(namespace, claim, pod, key, None);
    }

    /// kubectl, as this machine would find it here.
    pub(crate) fn kubectl(&self) -> Kubectl {
        Kubectl::found_in(
            &[],
            Some(&self.dir.path().join("bin").display().to_string()),
            None,
        )
        .unwrap()
    }

    /// Every run's words, a line each.
    pub(crate) fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("calls")).unwrap_or_default()
    }

    /// Every run's `KUBECONFIG` (`unset` where none) and `PATH`, a line each.
    pub(crate) fn environments(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("env")).unwrap_or_default()
    }
}

#[cfg(unix)]
#[test]
fn kubectl_is_found_by_an_absolute_path_and_never_by_a_relative_one() {
    let cluster = FakeCluster::new();
    let bin = cluster.dir.path().join("bin");
    let found = Kubectl::found_in(&[], Some(&format!("bin:{}", bin.display())), None).unwrap();
    assert_eq!(found.program(), bin.join("kubectl"));
    let installed = bin.join("kubectl").display().to_string();
    let first = Kubectl::found_in(&[&installed], Some("/nowhere"), None).unwrap();
    assert_eq!(first.program(), bin.join("kubectl"));
    let none = Kubectl::found_in(&[], Some("bin:relative/bin"), None).unwrap_err();
    assert!(none.to_string().contains("no kubectl"), "{none}");
}

#[cfg(unix)]
#[test]
fn a_kubectl_run_never_inherits_a_kubeconfig_and_finds_plugins_beside_it_or_system_wide() {
    let cluster = FakeCluster::new();
    let kubectl = cluster
        .kubectl()
        .pointed_at(std::path::Path::new("/home/ada/.kube/config"), "team");
    kubectl.whoami().unwrap();
    let env = cluster.environments();
    let (kubeconfig, path) = env.trim().split_once('|').unwrap();
    assert_eq!(kubeconfig, "unset");
    let bin = cluster.dir.path().join("bin").display().to_string();
    assert_eq!(path.split(':').next(), Some(bin.as_str()));
    assert!(path.split(':').any(|dir| dir == "/usr/bin"), "{path}");
    // Never a folder of the home, where a program could be put to be run as a plugin.
    assert!(!path.contains(".local/bin"), "{path}");
    assert!(
        cluster
            .calls()
            .starts_with("--kubeconfig /home/ada/.kube/config --context team auth whoami"),
        "{}",
        cluster.calls()
    );
}

#[cfg(unix)]
#[test]
fn a_kubeconfigs_contexts_and_its_contexts_namespace_are_read() {
    let cluster = FakeCluster::new();
    let kubectl = cluster.kubectl();
    let (contexts, current) = kubectl.contexts().unwrap();
    assert_eq!(contexts, ["team", "kind-test"]);
    assert_eq!(current.as_deref(), Some("team"));
    assert_eq!(kubectl.namespace().unwrap(), "ada");
    cluster.write(
        "minify.json",
        r#"{"contexts": [{"name": "team", "context": {}}]}"#,
    );
    assert_eq!(kubectl.namespace().unwrap(), "default");
}

#[cfg(unix)]
#[test]
fn a_link_is_refused_once_the_pool_names_another_operator_key_than_the_pinned_one() {
    let cluster = FakeCluster::new();
    let kubectl = cluster.kubectl();
    let mut pin = cluster.pin();
    cluster.vouch("ada", "ada-1", "ada-1", cluster.runner.key);
    let claimed = Claimed {
        uid: CLAIM_UID.into(),
        ..claimed()
    };
    let check = |pin: &mut PoolPin, pod: &str| {
        before_link(
            &kubectl,
            pin,
            "default",
            &claimed,
            pod,
            &cluster.runner,
            now(),
        )
    };
    assert_eq!(check(&mut pin, "ada-1"), Ok(()));
    // Another pod than the one paired with: a changed runner.
    assert!(matches!(
        check(&mut pin, "ada-2"),
        Err(Refused::KeyChanged { .. })
    ));
    cluster.names_key(&Operator::new().key());
    assert_eq!(
        check(&mut pin, "ada-1"),
        Err(Refused::OperatorKeyChanged {
            pool: "default".into()
        })
    );
    assert_eq!(pin, cluster.pin(), "a refusal leaves the pin");
}

#[cfg(unix)]
#[test]
fn a_pin_follows_a_roll_and_a_claim_not_yet_signed_again_refuses_that_link_alone() {
    let cluster = FakeCluster::new();
    let kubectl = cluster.kubectl();
    let next = Operator::new();
    let mut pin = cluster.pin();
    let claimed = Claimed {
        uid: CLAIM_UID.into(),
        ..claimed()
    };
    cluster.rolls_to(&next, now() + 24 * 3600);
    // The Claim has not been reconciled since: signed by the old key alone.
    cluster.vouch("ada", "ada-1", "ada-1", cluster.runner.key);
    let first = before_link(
        &kubectl,
        &mut pin,
        "default",
        &claimed,
        "ada-1",
        &cluster.runner,
        now(),
    );
    assert_eq!(pin.key, next.key(), "the pin moved, for the caller to keep");
    // Not vouched for this once: the link is refused, never as a changed key, so nothing is
    // unpaired and the next try goes ahead.
    assert!(matches!(first, Err(Refused::NotVouched(_))), "{first:?}");
    // Reconciled: the next key's signature beside the old one's, and the link goes ahead.
    cluster.vouch_signed("ada", "ada-1", "ada-1", cluster.runner.key, Some(&next));
    assert_eq!(
        before_link(
            &kubectl,
            &mut pin,
            "default",
            &claimed,
            "ada-1",
            &cluster.runner,
            now()
        ),
        Ok(())
    );
    assert_eq!(pin.key, next.key());
}

#[cfg(unix)]
#[test]
fn a_claim_never_vouched_for_is_deleted_and_says_what_the_claim_says() {
    let cluster = FakeCluster::new();
    cluster.write(
        "claim.json",
        r#"{"status": {"conditions": [{"type": "Ready", "status": "False", "reason": "OverQuota", "message": "5 claims"}]}}"#,
    );
    let desktop = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    };
    let why = claim_and_pair(
        &cluster.kubectl(),
        &cluster.pin(),
        &ClaimTo {
            pool: "default",
            namespace: "ada",
            name: "team-1",
            desktop: "0123456789abcdef",
        },
        &desktop,
        std::time::Duration::ZERO,
        std::time::Duration::ZERO,
    )
    .unwrap_err();
    assert!(why.to_string().contains("OverQuota: 5 claims"), "{why}");
    assert!(
        cluster
            .calls()
            .lines()
            .any(|call| call.contains("delete claims.runners.purlis.dev team-1")),
        "{}",
        cluster.calls()
    );
}

#[test]
fn a_desktops_label_is_its_device_ids_in_any_spelling() {
    let label = desktop_label("01JA0DESKT0P00000000000000");
    assert_eq!(label.len(), 16);
    assert!(
        label
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert_eq!(label, desktop_label("01ja0deskt0p00000000000000"));
    assert_ne!(label, desktop_label("01JA0DESKT0P00000000000001"));
}

#[test]
fn a_kubeconfigs_plugins_are_each_users_exec_command() {
    let kept = r#"{"users": [
        {"name": "a", "user": {"exec": {"command": "gke-gcloud-auth-plugin"}}},
        {"name": "b", "user": {"token": "t"}},
        {"name": "c", "user": {"exec": {"command": "/usr/local/bin/aws", "args": ["eks"]}}}
    ]}"#;
    assert_eq!(
        plugins_named(kept).unwrap(),
        ["gke-gcloud-auth-plugin", "/usr/local/bin/aws"]
    );
    assert!(plugins_named("apiVersion: v1").is_err());
}

#[test]
fn a_context_is_named_as_its_kubeconfig_names_it_but_never_as_a_flag() {
    for named in [
        "team",
        "arn:aws:eks:eu-west-1:1:cluster/team",
        "gke_p_z_team",
        "Kind-Team",
    ] {
        assert!(is_context(named), "{named}");
    }
    for refused in ["", "--server=https://elsewhere", "-n", "a\nb"] {
        assert!(!is_context(refused), "{refused:?}");
    }
}

#[test]
fn only_a_pool_whose_spec_says_so_whole_mints_in_the_cluster() {
    use serde_json::json;
    let key = json!({"name": "org-app"});
    let opted =
        json!({"forge": {"appKeySecretRef": key, "repos": ["acme/*"], "mintInCluster": true}});
    assert!(mints_in_cluster(&opted));
    for not in [
        json!({}),
        json!({"forge": null}),
        json!({"forge": {"appKeySecretRef": key, "repos": ["acme/*"]}}),
        json!({"forge": {"appKeySecretRef": key, "repos": ["acme/*"], "mintInCluster": false}}),
        // Spelled any other way is no opt-in.
        json!({"forge": {"appKeySecretRef": key, "repos": ["acme/*"], "mintInCluster": "true"}}),
        json!({"forge": {"repos": ["acme/*"], "mintInCluster": true}}),
        json!({"forge": {"appKeySecretRef": key, "mintInCluster": true}}),
        json!({"forge": {"appKeySecretRef": key, "repos": [], "mintInCluster": true}}),
    ] {
        assert!(!mints_in_cluster(&not), "{not}");
    }
}

/// A cluster that says the same of a Pool and a Claim (#4207): the Pool opting in, or not, and
/// the Claim's status saying where to ask with `key`.
#[cfg(unix)]
fn says_where_to_ask(dir: &std::path::Path, opted_in: bool, key: &str) -> Kubectl {
    let answer = serde_json::json!({
        "spec": {"forge": {
            "appKeySecretRef": {"name": "org-app"},
            "repos": ["acme/*"],
            "mintInCluster": opted_in,
        }},
        "status": {
            "signingKey": operator_key().spelled(),
            "cluster": "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b",
            "mint": {"address": "10.96.0.5:8443", "operatorKey": key},
        },
    });
    kubectl_answering(dir, &answer.to_string())
}

#[cfg(unix)]
fn a_paired_runner() -> Paired {
    let key = LinkKey::generate().unwrap().public();
    Paired {
        claimed: claimed(),
        vouched: vouched(key),
        pinned: Pinned {
            device: "01JB0RUNNER000000000000000".into(),
            key,
        },
    }
}

fn the_pin() -> PoolPin {
    PoolPin {
        key: operator_key(),
        cluster: "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b".into(),
    }
}

#[cfg(unix)]
#[test]
fn a_runner_is_told_of_the_port_only_on_a_pool_that_opts_in() {
    let last_run = |dir: &std::path::Path| std::fs::read_to_string(dir.join("args")).unwrap();
    let every = std::time::Duration::from_millis(1);
    let pinned = operator_key().spelled();

    let dir = tempfile::tempdir().unwrap();
    let kubectl = says_where_to_ask(dir.path(), true, &pinned);
    tell_the_mint_port(&kubectl, &the_pin(), "pool", &a_paired_runner(), every).unwrap();
    assert!(last_run(dir.path()).contains("--mint-port"));

    let dir = tempfile::tempdir().unwrap();
    let kubectl = says_where_to_ask(dir.path(), false, &pinned);
    tell_the_mint_port(&kubectl, &the_pin(), "pool", &a_paired_runner(), every).unwrap();
    assert!(!last_run(dir.path()).contains("--mint-port"));
}

#[cfg(unix)]
#[test]
fn at_a_link_a_runner_is_told_again_only_on_a_pool_that_opts_in_and_by_the_pinned_key() {
    let dir = tempfile::tempdir().unwrap();
    let kubectl = says_where_to_ask(dir.path(), true, "ed25519:a-status-says-anything");
    let paired = a_paired_runner();
    let seen = kubectl.pool("pool").unwrap();
    let told = to_tell_at_a_link(&seen, &kubectl, &the_pin(), &paired.claimed)
        .unwrap()
        .expect("the status says where");
    assert_eq!(told.keys, [operator_key().spelled()]);
    tell_again(&kubectl, "ns", "pod", &told).unwrap();
    let ran = std::fs::read_to_string(dir.path().join("args")).unwrap();
    assert!(ran.contains("--mint-port\n--again"), "{ran}");

    let dir = tempfile::tempdir().unwrap();
    let kubectl = says_where_to_ask(dir.path(), false, &operator_key().spelled());
    let seen = kubectl.pool("pool").unwrap();
    assert_eq!(
        to_tell_at_a_link(&seen, &kubectl, &the_pin(), &paired.claimed).unwrap(),
        None
    );
}

#[test]
fn a_runner_is_told_the_pinned_key_alone_whatever_a_status_says() {
    let told = told_of_the_port(&claimed(), &the_pin(), "10.96.0.5:8443".to_owned());
    assert_eq!(told.keys, [operator_key().spelled()]);
    assert_eq!(told.address, "10.96.0.5:8443");
}

#[test]
fn a_claims_repos_only_grow_each_once_ignoring_case() {
    let named = vec!["acme/app".to_owned()];
    assert_eq!(repos_with(&named, &["ACME/App".to_owned()]), None);
    assert_eq!(repos_with(&named, &[]), None);
    assert_eq!(
        repos_with(&named, &["acme/web".to_owned(), "acme/web".to_owned()]).unwrap(),
        ["acme/app", "acme/web"]
    );
}

#[cfg(unix)]
#[test]
fn repos_are_written_over_the_claim_as_it_was_read_and_only_where_they_add_something() {
    let dir = tempfile::tempdir().unwrap();
    let answer = serde_json::json!({
        "metadata": {"resourceVersion": "41"},
        "spec": {"forge": {"repos": ["acme/app"]}},
    });
    let kubectl = kubectl_answering(dir.path(), &answer.to_string());
    let last_run = || std::fs::read_to_string(dir.path().join("args")).unwrap();
    kubectl
        .add_claim_repos("team", "ada-1", &["acme/app".to_owned()])
        .unwrap();
    assert!(!last_run().contains("patch"), "nothing new is written");
    kubectl
        .add_claim_repos("team", "ada-1", &["acme/web".to_owned()])
        .unwrap();
    let run = last_run();
    assert!(run.contains("patch"), "{run}");
    assert!(run.contains("\"resourceVersion\":\"41\""), "{run}");
    assert!(
        run.contains("acme/app") && run.contains("acme/web"),
        "{run}"
    );
}

#[test]
fn a_claims_deletion_notice_is_read_only_where_it_is_true() {
    let with = |status: &str| {
        serde_json::json!({ "status": { "conditions": [
            { "type": "Ready", "status": "False", "message": "not this" },
            { "type": "DeletionNotice", "status": status, "message": "deleted at noon" },
        ] } })
    };
    assert_eq!(
        deletion_notice_of(&with("True")).as_deref(),
        Some("deleted at noon")
    );
    assert_eq!(deletion_notice_of(&with("False")), None);
    assert_eq!(deletion_notice_of(&serde_json::json!({})), None);
}

/// **What a start waits for on a Claim** (#4403): a runner suspended for being idle, and nothing
/// else its Claim may say.
#[test]
fn a_start_waits_only_for_a_runner_suspended_for_being_idle() {
    use serde_json::json;
    let claim = |conditions: serde_json::Value| json!({ "status": { "conditions": conditions } });
    let idle = |status: &str| json!({ "type": "Idle", "status": status, "lastTransitionTime": "2026-11-01T08:00:00Z" });
    let ready = |reason: &str| json!({ "type": "Ready", "status": "False", "reason": reason });

    assert!(suspended_for_being_idle(&claim(json!([
        idle("True"),
        ready("Idle")
    ]))));
    for other in ["UpstreamObjectInTheWay", "PoolDeleted", "PoolNotFound"] {
        assert!(
            !suspended_for_being_idle(&claim(json!([idle("True"), ready(other)]))),
            "{other} is not acted on"
        );
    }
    assert!(
        !suspended_for_being_idle(&claim(json!([idle("Unknown"), ready("Stopped")]))),
        "stopped"
    );
    assert!(
        !suspended_for_being_idle(&claim(json!([ready("Starting")]))),
        "a restart"
    );
}

/// **The runs of one check share one time**: past it, a run is ended at once, whatever each is
/// given.
#[cfg(unix)]
#[test]
fn a_check_of_several_runs_answers_within_one_time() {
    let kubectl = Kubectl::at("/bin/sleep")
        .within(std::time::Duration::from_secs(10))
        .all_within(std::time::Duration::from_millis(200));
    let started = std::time::Instant::now();
    for _ in 0..3 {
        assert!(kubectl.run(&["30".to_owned()], None, 1024).is_err());
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

/// **kubectl's own preferences file is off** for a kubectl found on this machine.
#[test]
fn a_found_kubectl_runs_with_its_preferences_file_off() {
    let kubectl = Kubectl {
        path: Some("/bin".into()),
        ..Kubectl::at("/bin/kubectl")
    };
    let (env, _) = kubectl.environment();
    assert!(env.contains(&("KUBERC".into(), "off".into())), "{env:?}");
}
