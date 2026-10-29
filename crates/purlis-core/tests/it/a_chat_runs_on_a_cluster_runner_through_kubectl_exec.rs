// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat on a cluster runner, for real (#2056): a Claim on a Pool of purlis's cluster operator,
//! made as this kubeconfig's user; the operator's statement of the runner's link key, checked
//! against the key the Pool names; a pairing over the desktop's own `kubectl exec` that must
//! agree with it; then a link through `kubectl exec -i` and the runner's bridge, and a chat
//! started, drawn and stopped on it, as on any host. A runner answering another key than the one
//! vouched for, as one reached in another runner's place is, is refused. A kept claim's runner
//! whose pod is lost is made again on its own volume and resumed with the pairing it had, its
//! home as it was left (#2057). Everything it made is removed, whatever the outcome.
//!
//! It needs a cluster with purlis's cluster operator installed and the runner image of this
//! build named by digest in `CLUSTER_POOL_TEST_IMAGE`, with the kubeconfig context in
//! `CLUSTER_POOL_TEST_CONTEXT` (and `kubectl` on `PATH`): `operator/e2e/kind.sh` sets both on
//! a kind cluster and runs it with `--ignored`. It is ignored anywhere else, and fails when
//! either is missing.

#![cfg(unix)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_core::clusterpool::{
    self, Claimed, Keeping, Kubectl, OperatorKey, Paired, Refused, Vouched,
};
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
};
use purlis_session_protocol::link::Acceptor;
use purlis_session_protocol::session::{Client, Launch, ProgramKind, StartProgram, ViewKind};
use purlis_session_protocol::view::{Chunk, Opened, Viewer};

const PATIENCE: Duration = Duration::from_secs(60);
const PROJECT: &str = "01J9PRJCT00000000000000000";
const SAID: &str = "a chat on a cluster runner";

/// Removes the namespace and pool the test made, however it ends.
struct Removes {
    kubectl: Kubectl,
    namespace: String,
    pool: String,
}

impl Drop for Removes {
    fn drop(&mut self) {
        for args in [
            vec!["delete", "namespace", &self.namespace, "--wait=false"],
            vec![
                "delete",
                "pools.runners.purlis.dev",
                &self.pool,
                "--wait=false",
            ],
        ] {
            let _ = self.kubectl.run(&self.kubectl.args(&args), None, 64 * 1024);
        }
    }
}

fn eventually<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(
            start.elapsed() < Duration::from_secs(300),
            "{what}: not within 300s"
        );
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn claimed(
    kubectl: &Kubectl,
    namespace: &str,
    name: &str,
    pool: &str,
    keeping: Keeping,
) -> Claimed {
    let requester = kubectl.whoami().expect("who this kubeconfig is");
    let uid = kubectl
        .claim(namespace, name, pool, &requester, keeping)
        .expect("the claim is made");
    Claimed {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        uid,
        requester,
    }
}

fn vouched(kubectl: &Kubectl, operator: &OperatorKey, claimed: &Claimed) -> Vouched {
    eventually("the operator vouches for the runner's key", || {
        clusterpool::vouched_for(kubectl, operator, claimed).expect("the statement checks")
    })
}

async fn linked(
    kubectl: &Kubectl,
    operator: &OperatorKey,
    paired: &Paired,
    ours: Identity,
) -> (Client, Acceptor) {
    let connector = CommandConnector::new(
        kubectl.program(),
        kubectl.connect_args(&paired.vouched.runner_namespace, &paired.vouched.pod),
        {
            let (kubectl, operator, paired) = (kubectl.clone(), operator.clone(), paired.clone());
            Arc::new(move || {
                clusterpool::still_vouched(&kubectl, &operator, &paired)
                    .map_err(|why| why.to_string())
            })
        },
    );
    let mut links = hostlink::keep_linked(
        connector,
        Handshake::THIS_BUILD,
        ours,
        paired.pinned.clone(),
        Liveness::default(),
    );
    match tokio::time::timeout(PATIENCE, links.next()).await {
        Ok(Some(Linked::Up(link))) => {
            let (client, views, _events) = Client::new(*link);
            tokio::spawn(async move { while links.next().await.is_some() {} });
            (client, views)
        }
        Ok(Some(Linked::Down(down))) => panic!("not linked: {}", down.why),
        Ok(None) => panic!("the kept link ended"),
        Err(_) => panic!("no link within the test's patience"),
    }
}

#[test]
#[ignore = "needs a cluster with purlis's cluster operator: operator/e2e/kind.sh"]
fn a_chat_runs_on_a_cluster_runner_paired_with_the_key_the_operator_vouched_for() {
    purlis_core::unsteered!();
    // Run only by kind.sh, which names both: without them it fails, never passes unrun.
    let image = std::env::var("CLUSTER_POOL_TEST_IMAGE")
        .expect("CLUSTER_POOL_TEST_IMAGE names the runner image by digest");
    let context = std::env::var("CLUSTER_POOL_TEST_CONTEXT")
        .expect("CLUSTER_POOL_TEST_CONTEXT names the cluster's kubeconfig context");
    let kubectl = Kubectl::at("kubectl").with_context(&context);
    let suffix = ulid::Ulid::generate().to_string().to_lowercase();
    let namespace = format!("e2e-chat-{}", &suffix[..10]);
    let pool = format!("e2e-chat-{}", &suffix[..10]);
    let _removes = Removes {
        kubectl: kubectl.clone(),
        namespace: namespace.clone(),
        pool: pool.clone(),
    };
    kubectl
        .run(
            &kubectl.args(&["create", "namespace", &namespace]),
            None,
            64 * 1024,
        )
        .expect("a namespace of the test's own");
    let definition = serde_json::json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Pool",
        "metadata": { "name": pool },
        "spec": { "image": image },
    });
    kubectl
        .run(
            &kubectl.args(&["apply", "--filename", "-"]),
            Some(&definition.to_string()),
            64 * 1024,
        )
        .expect("the pool");

    // Adding the pool pins the operator's key its status names.
    let operator = eventually("the pool names the operator's key", || {
        kubectl.operator_key(&pool).ok()
    });

    let first = claimed(&kubectl, &namespace, "ada-1", &pool, Keeping::Fresh);
    let second = claimed(&kubectl, &namespace, "ada-2", &pool, Keeping::Fresh);
    let first_vouched = vouched(&kubectl, &operator, &first);
    let second_vouched = vouched(&kubectl, &operator, &second);
    assert_ne!(
        first_vouched.key, second_vouched.key,
        "each runner made a key of its own"
    );

    let ours = Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap(),
    };
    let desktop = Pinned {
        device: ours.device.clone(),
        key: ours.key.public(),
    };

    // The desktop's exec reaching another runner than the one the operator vouched for answers
    // another key, and is refused.
    let substituted = Vouched {
        pod: second_vouched.pod.clone(),
        ..first_vouched.clone()
    };
    let refused = clusterpool::pair(&kubectl, &first, &substituted, &desktop).unwrap_err();
    assert!(matches!(refused, Refused::KeyChanged { .. }), "{refused:?}");

    // The runner the operator vouched for answers that key, and is paired.
    let paired = clusterpool::pair(&kubectl, &first, &first_vouched, &desktop)
        .expect("the vouched runner is paired");
    assert_eq!(paired.pinned.key, first_vouched.key);
    // A runner is paired once.
    assert!(clusterpool::pair(&kubectl, &first, &first_vouched, &desktop).is_err());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (client, mut views) = linked(&kubectl, &operator, &paired, ours).await;
        assert_eq!(client.list().await.expect("listed").len(), 0);
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the runner holds the project where it keeps it");
        let chat = client
            .start_program(StartProgram {
                launch: Some(Launch {
                    program: "/bin/sh".into(),
                    args: vec!["-c".into(), format!("echo '{SAID}'; exec sleep 120")],
                    cwd: None,
                    set: Vec::new(),
                    strip: Vec::new(),
                    pass: Vec::new(),
                    declared: Vec::new(),
                    harness: None,
                    git_hooks: None,
                    reporting: None,
                    confinement: None,
                }),
                ..StartProgram::of(PROJECT, ProgramKind::Shell)
            })
            .await
            .expect("the chat starts on the runner");
        let listed = client.list().await.expect("listed");
        assert!(listed.iter().any(|one| one.chat == chat), "{listed:?}");

        client
            .attach_view(&chat, ViewKind::Terminal)
            .await
            .expect("the runner opens its view");
        let stream = tokio::time::timeout(PATIENCE, views.accept())
            .await
            .expect("a view within the test's patience")
            .expect("a view");
        let Opened::View(mut reader) = Viewer::default().accept_any(stream).await.unwrap() else {
            panic!("a code stream, where a view was meant");
        };
        let mut drawn = Vec::new();
        while !String::from_utf8_lossy(&drawn).contains(SAID) {
            let chunk = tokio::time::timeout(PATIENCE, reader.next())
                .await
                .expect("the chat draws")
                .expect("the view is open")
                .unwrap();
            let bytes = match chunk {
                Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
            };
            drawn.extend_from_slice(&bytes);
            reader.ack(bytes.len()).await.expect("acknowledged");
        }

        client.stop(&chat).await.expect("the chat stops");
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    let _ = kubectl.delete_claim(&namespace, "ada-1");
    let _ = kubectl.delete_claim(&namespace, "ada-2");
}

/// Starts `/bin/sh -c command` on the runner `client` is linked to, and answers what it drew
/// once that holds `until`.
async fn ran(client: &Client, views: &mut Acceptor, command: &str, until: &str) -> String {
    let chat = client
        .start_program(StartProgram {
            launch: Some(Launch {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), command.into()],
                cwd: None,
                set: Vec::new(),
                strip: Vec::new(),
                pass: Vec::new(),
                declared: Vec::new(),
                harness: None,
                git_hooks: None,
                reporting: None,
                confinement: None,
            }),
            ..StartProgram::of(PROJECT, ProgramKind::Shell)
        })
        .await
        .expect("the chat starts on the runner");
    client
        .attach_view(&chat, ViewKind::Terminal)
        .await
        .expect("the runner opens its view");
    let stream = tokio::time::timeout(PATIENCE, views.accept())
        .await
        .expect("a view within the test's patience")
        .expect("a view");
    let Opened::View(mut reader) = Viewer::default().accept_any(stream).await.unwrap() else {
        panic!("a code stream, where a view was meant");
    };
    let mut drawn = Vec::new();
    while !String::from_utf8_lossy(&drawn).contains(until) {
        let chunk = tokio::time::timeout(PATIENCE, reader.next())
            .await
            .expect("the chat draws")
            .expect("the view is open")
            .unwrap();
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        drawn.extend_from_slice(&bytes);
        reader.ack(bytes.len()).await.expect("acknowledged");
    }
    client.stop(&chat).await.expect("the chat stops");
    String::from_utf8_lossy(&drawn).into_owned()
}

/// What kubectl prints for `jsonpath` of the object `kind`/`name` in `namespace`, if it is there.
fn field(
    kubectl: &Kubectl,
    namespace: &str,
    kind: &str,
    name: &str,
    jsonpath: &str,
) -> Option<String> {
    kubectl
        .run(
            &kubectl.args(&[
                "get",
                kind,
                name,
                "--namespace",
                namespace,
                "--output",
                &format!("jsonpath={jsonpath}"),
            ]),
            None,
            64 * 1024,
        )
        .ok()
        .map(|printed| printed.trim().to_owned())
        .filter(|printed| !printed.is_empty())
}

#[test]
#[ignore = "needs a cluster with purlis's cluster operator: operator/e2e/kind.sh"]
fn a_kept_claims_runner_keeps_its_pairing_and_its_home_when_its_pod_is_made_again() {
    purlis_core::unsteered!();
    let image = std::env::var("CLUSTER_POOL_TEST_IMAGE")
        .expect("CLUSTER_POOL_TEST_IMAGE names the runner image by digest");
    let context = std::env::var("CLUSTER_POOL_TEST_CONTEXT")
        .expect("CLUSTER_POOL_TEST_CONTEXT names the cluster's kubeconfig context");
    let kubectl = Kubectl::at("kubectl").with_context(&context);
    let suffix = ulid::Ulid::generate().to_string().to_lowercase();
    let namespace = format!("e2e-kept-{}", &suffix[..10]);
    let pool = format!("e2e-kept-{}", &suffix[..10]);
    let _removes = Removes {
        kubectl: kubectl.clone(),
        namespace: namespace.clone(),
        pool: pool.clone(),
    };
    kubectl
        .run(
            &kubectl.args(&["create", "namespace", &namespace]),
            None,
            64 * 1024,
        )
        .expect("a namespace of the test's own");
    let definition = serde_json::json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Pool",
        "metadata": { "name": pool },
        "spec": { "image": image, "keptVolume": { "size": "1Gi" } },
    });
    kubectl
        .run(
            &kubectl.args(&["apply", "--filename", "-"]),
            Some(&definition.to_string()),
            64 * 1024,
        )
        .expect("the pool");
    let operator = eventually("the pool names the operator's key", || {
        kubectl.operator_key(&pool).ok()
    });

    let kept = claimed(&kubectl, &namespace, "ada-kept", &pool, Keeping::Kept);
    let vouched_first = vouched(&kubectl, &operator, &kept);
    assert_eq!(
        vouched_first.pod, "ada-kept",
        "a kept claim's runner is its own, cold"
    );
    // Its runner, and its volume, are in its requester's own runner namespace (#2055).
    let runner = clusterpool::runner_namespace(&kept.requester);
    assert_eq!(vouched_first.runner_namespace, runner);
    let volume = eventually("the claim's own volume", || {
        field(
            &kubectl,
            &runner,
            "persistentvolumeclaim",
            "purlis-kept-ada-kept",
            "{.metadata.uid}",
        )
    });

    let ours = Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap(),
    };
    let desktop = Pinned {
        device: ours.device.clone(),
        key: ours.key.public(),
    };
    let paired = clusterpool::pair(&kubectl, &kept, &vouched_first, &desktop)
        .expect("the vouched runner is paired");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let identity = || Identity {
        device: ours.device.clone(),
        key: ours.key.clone(),
    };
    runtime.block_on(async {
        let (client, mut views) = linked(&kubectl, &operator, &paired, identity()).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the runner holds the project where it keeps it");
        ran(
            &client,
            &mut views,
            "echo 'the conversation so far' > \"$HOME/conversation\" && echo written; exec sleep 120",
            "written",
        )
        .await;
    });

    // The pod is lost: upstream makes it again, under the same name, on the same volume.
    let first_pod =
        field(&kubectl, &runner, "pod", "ada-kept", "{.metadata.uid}").expect("the runner's pod");
    kubectl
        .run(
            &kubectl.args(&[
                "delete",
                "pod",
                "ada-kept",
                "--namespace",
                &runner,
                "--wait=true",
            ]),
            None,
            64 * 1024,
        )
        .expect("the runner's pod deleted");
    eventually("the runner's pod made again and ready", || {
        let uid = field(&kubectl, &runner, "pod", "ada-kept", "{.metadata.uid}")?;
        let ready = field(
            &kubectl,
            &runner,
            "pod",
            "ada-kept",
            "{.status.conditions[?(@.type==\"Ready\")].status}",
        )?;
        (uid != first_pod && ready == "True").then_some(())
    });
    assert_eq!(
        field(
            &kubectl,
            &runner,
            "persistentvolumeclaim",
            "purlis-kept-ada-kept",
            "{.metadata.uid}"
        ),
        Some(volume),
        "the same volume"
    );
    // The operator vouches for the same runner: the key it was paired with.
    let again = eventually("the operator vouches for the runner again", || {
        let now = clusterpool::vouched_for(&kubectl, &operator, &kept).ok()??;
        (clusterpool::held_to(&paired, &now).is_ok()).then_some(now)
    });
    assert_eq!(again.key, paired.pinned.key);

    // Resumed with the pairing it had, never paired again: its home is as it was left.
    runtime.block_on(async {
        let (client, mut views) = linked(&kubectl, &operator, &paired, identity()).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the runner holds the project where it keeps it");
        let drawn = ran(
            &client,
            &mut views,
            "cat \"$HOME/conversation\"; echo done; exec sleep 120",
            "done",
        )
        .await;
        assert!(drawn.contains("the conversation so far"), "{drawn}");
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    let _ = kubectl.delete_claim(&namespace, "ada-kept");
}
