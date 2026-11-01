// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat on a cluster runner, for real (#2056): a Claim on a Pool of purlis's cluster operator,
//! made as this kubeconfig's user; the operator's statement of the runner's link key, checked
//! against the key the Pool names; a pairing over the desktop's own `kubectl exec` that must
//! agree with it; then a link through `kubectl exec -i` and the runner's bridge, and a chat
//! started, drawn and stopped on it, as on any host. A runner answering another key than the one
//! vouched for, as one reached in another runner's place is, is refused. A kept claim's runner
//! whose pod is lost is made again on its own volume and resumed with the pairing it had, its
//! home as it was left (#2057). A profile kept there is asked its standing over the link and by
//! the sign-in's own `kubectl exec`, and what a sign-in leaves in its folder outlives the pod
//! (#3819); and a killed `kubectl exec -i` closes the input of what it ran in the pod, which
//! is how Cancel ends a sign-in there. `purlis profile sign-in --pool` is refused an unmarked pool
//! and a stopped claim, and on a marked, vouched one reaches the runner's own sign-in over kubectl
//! (#4164), and the runner names and forgets a profile over the link (#4183). Everything it made
//! is removed, whatever the outcome.
//!
//! It needs a cluster with purlis's cluster operator installed and the runner image of this
//! build named by digest in `CLUSTER_POOL_TEST_IMAGE`, with the kubeconfig context in
//! `CLUSTER_POOL_TEST_CONTEXT` (and `kubectl` on `PATH`): `operator/e2e/kind.sh` sets both on
//! a kind cluster and runs it with `--ignored`, with this build's binary in
//! `CLUSTER_POOL_TEST_PURLIS`. It is ignored anywhere else, and fails when any is missing.

#![cfg(unix)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_core::clusterpool::{
    self, Claimed, Keeping, Kubectl, Paired, PoolPin, Refused, Vouched,
};
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
};
use purlis_session_protocol::link::Acceptor;
use purlis_session_protocol::session::{
    Client, Launch, ProfileStanding, ProfileToKeep, ProgramKind, StartProgram, ViewKind,
};
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

fn vouched(kubectl: &Kubectl, operator: &PoolPin, claimed: &Claimed) -> Vouched {
    eventually("the operator vouches for the runner's key", || {
        clusterpool::vouched_for(kubectl, operator, claimed).expect("the statement checks")
    })
}

async fn linked(
    kubectl: &Kubectl,
    operator: &PoolPin,
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

/// Nothing runs on the desktop's own host in these tests: it only relays.
struct NothingHere;

impl purlis_host::Compose for NothingHere {
    fn compose(
        &self,
        starting: &purlis_host::ToCompose<'_>,
    ) -> Result<purlis_terminal::session::Spec, String> {
        Ok(purlis_terminal::session::Spec::new(
            "/bin/echo",
            starting.size,
        ))
    }
}

/// **The desktop's host as the window starts it** (#4403), relaying to the one runner `paired`
/// and asking for it to run again as the desktop does: what its start waits, it waits as the app's
/// `runnerlinks::runs_again` has it.
fn desktops_host(
    kubectl: &Kubectl,
    operator: &PoolPin,
    paired: &Paired,
    ours: &Identity,
    namespace: &str,
) -> purlis_host::Hosting {
    let relays: purlis_host::RelaysTo = {
        let (kubectl, operator, paired, ours) = (
            kubectl.clone(),
            operator.clone(),
            paired.clone(),
            Identity {
                device: ours.device.clone(),
                key: ours.key.clone(),
            },
        );
        Arc::new(move |name: &str| {
            let (kubectl, operator, paired, device, key) = (
                kubectl.clone(),
                operator.clone(),
                paired.clone(),
                ours.device.clone(),
                ours.key.clone(),
            );
            Ok((
                Arc::new(move || {
                    let checking = (kubectl.clone(), operator.clone(), paired.clone());
                    hostlink::keep_linked(
                        CommandConnector::new(
                            kubectl.program(),
                            kubectl.connect_args(
                                &paired.vouched.runner_namespace,
                                &paired.vouched.pod,
                            ),
                            Arc::new(move || {
                                clusterpool::still_vouched(&checking.0, &checking.1, &checking.2)
                                    .map_err(|why| why.to_string())
                            }),
                        ),
                        Handshake::THIS_BUILD,
                        Identity {
                            device: device.clone(),
                            key: key.clone(),
                        },
                        paired.pinned.clone(),
                        Liveness::default(),
                    )
                }) as purlis_host::Keep,
                format!("{name}/{name}"),
            ))
        })
    };
    let (asks, namespace) = (kubectl.clone(), namespace.to_owned());
    purlis_host::Hosting::new(NothingHere)
        .relaying_with(relays)
        .running_again_with(Arc::new(move |name: &str| {
            let claim = name.split('/').next().unwrap_or(name);
            asks.run_again_if_idle(&namespace, claim)
                .expect("the word is written")
                .then_some(Duration::from_secs(240))
        }))
}

/// A window's link to `host`, routed as the desktop's host routes one.
async fn a_window_on(host: purlis_host::Hosting) -> (Client, Acceptor) {
    use purlis_session_protocol::auth::{Credentials, Scope};
    use purlis_session_protocol::{link, session};
    let held = Credentials::mint().unwrap();
    let (a, b) = tokio::io::duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            held.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &held)
    );
    tokio::spawn(session::serve(
        served.unwrap(),
        purlis_host::Routed::new(host),
        None,
    ));
    let (client, views, events) = Client::new(client.unwrap());
    tokio::spawn(async move {
        let _kept = events;
        std::future::pending::<()>().await;
    });
    (client, views)
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

    // Adding the pool pins the operator's key and the cluster its status names (#3305).
    let operator = eventually("the pool names the operator's key", || {
        kubectl.pool_pin(&pool).ok()
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
                    prompt_key: None,
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

    // A killed `kubectl exec -i` closes the input of what it ran in the pod (#3819): what Cancel
    // ends a sign-in on a cluster runner by, since nothing else reaches into the pod.
    let (runners, pod) = (&paired.vouched.runner_namespace, &paired.vouched.pod);
    let mut command = std::process::Command::new(kubectl.program());
    command
        .args(kubectl.args(&[
            "exec",
            "-i",
            "--namespace",
            runners,
            "--container",
            clusterpool::RUNNER_CONTAINER,
            &format!("pod/{pod}"),
            "--",
            "/bin/sh",
            "-c",
            "echo started > \"$HOME/e2e-input\"; cat > /dev/null; echo ended > \"$HOME/e2e-input\"",
        ]))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut exec = purlis_core::forklock::spawn(&mut command).expect("kubectl exec runs");
    let said = || {
        in_the_pod(
            &kubectl,
            runners,
            pod,
            "cat \"$HOME/e2e-input\" 2>/dev/null; true",
        )
    };
    eventually("what kubectl exec ran has started", || {
        said().contains("started").then_some(())
    });
    exec.kill().expect("kubectl exec is ended");
    let _ = exec.wait();
    eventually("what it ran sees its input close, and ends", || {
        said().contains("ended").then_some(())
    });
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
                prompt_key: None,
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

/// The profile `work` of Claude Code, as a desktop has a runner keep it (#3819).
fn work() -> ProfileToKeep {
    ProfileToKeep {
        name: "work".into(),
        harness: "claude".into(),
        sandbox_only: false,
        pinned: None,
        settings: None,
        settings_staged: None,
        settings_changed: None,
    }
}

/// **Where the runner in `pod` of `namespace` says `work` stands**, asked by its own `purlis
/// profile sign-in --check` over the `kubectl exec` a sign-in there runs by (#3819).
fn standing_by_exec(kubectl: &Kubectl, namespace: &str, pod: &str) -> String {
    let args = kubectl.purlis_args(namespace, pod, &["profile", "sign-in", "work", "--check"]);
    let ran =
        purlis_core::forklock::output(std::process::Command::new(kubectl.program()).args(&args))
            .expect("kubectl runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    match purlis_core::dockerpool::standing_said("work", &said) {
        Ok(standing) => format!("{standing:?}"),
        Err(why) => panic!("the runner said no standing of work: {why}"),
    }
}

/// Runs `script` with `sh -c` in the runner `pod` of `namespace`, and answers what it printed.
fn in_the_pod(kubectl: &Kubectl, namespace: &str, pod: &str, script: &str) -> String {
    kubectl
        .run(
            &kubectl.args(&[
                "exec",
                "--namespace",
                namespace,
                "--container",
                clusterpool::RUNNER_CONTAINER,
                &format!("pod/{pod}"),
                "--",
                "/bin/sh",
                "-c",
                script,
            ]),
            None,
            64 * 1024,
        )
        .expect("the script ran in the runner's pod")
}

/// Leaves a mark where a sign-in of `work` would, in its folder in the runner's home, and prints
/// that folder.
const MARK_SIGN_IN: &str = "f=$(find \"$HOME\" -type d -path '*/profiles/work' | head -n 1) && \
     test -n \"$f\" && echo 'signed in before the pod went' > \"$f/e2e-sign-in\" && echo \"$f\"";

/// What the sign-in left in `work`'s folder, as the runner holds it now.
const READ_SIGN_IN: &str =
    "cat \"$(find \"$HOME\" -type d -path '*/profiles/work' | head -n 1)/e2e-sign-in\"";

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

/// `check`, until it answers or `patience` passes.
fn within<T>(patience: Duration, what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let until = std::time::Instant::now() + patience;
    loop {
        if let Some(answer) = check() {
            return answer;
        }
        assert!(
            std::time::Instant::now() < until,
            "{what}, within {patience:?}"
        );
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// **A kept runner idle for its pool's `idleAfter` is suspended, never while a chat runs on it,
/// and a word from its desktop runs it again, paired as it was** (#3980).
#[test]
#[ignore = "needs a cluster with purlis's cluster operator: operator/e2e/kind.sh"]
fn an_idle_kept_runner_is_suspended_and_its_desktop_runs_it_again() {
    purlis_core::unsteered!();
    let image = std::env::var("CLUSTER_POOL_TEST_IMAGE")
        .expect("CLUSTER_POOL_TEST_IMAGE names the runner image by digest");
    let context = std::env::var("CLUSTER_POOL_TEST_CONTEXT")
        .expect("CLUSTER_POOL_TEST_CONTEXT names the cluster's kubeconfig context");
    let kubectl = Kubectl::at("kubectl").with_context(&context);
    let suffix = ulid::Ulid::generate().to_string().to_lowercase();
    let namespace = format!("e2e-idle-{}", &suffix[..10]);
    let pool = format!("e2e-idle-{}", &suffix[..10]);
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
        "spec": { "image": image, "keptVolume": { "size": "1Gi" }, "idleAfter": "1m" },
    });
    kubectl
        .run(
            &kubectl.args(&["apply", "--filename", "-"]),
            Some(&definition.to_string()),
            64 * 1024,
        )
        .expect("the pool");
    let operator = eventually("the pool names the operator's key", || {
        kubectl.pool_pin(&pool).ok()
    });
    let kept = claimed(&kubectl, &namespace, "ada-idle", &pool, Keeping::Kept);
    let first = vouched(&kubectl, &operator, &kept);
    let runner = clusterpool::runner_namespace(&kept.requester);
    let ours = Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap(),
    };
    let desktop = Pinned {
        device: ours.device.clone(),
        key: ours.key.public(),
    };
    let paired =
        clusterpool::pair(&kubectl, &kept, &first, &desktop).expect("the vouched runner is paired");
    let idle = || {
        field(
            &kubectl,
            &namespace,
            "claims.runners.purlis.dev",
            "ada-idle",
            "{.status.conditions[?(@.type=='Idle')].status}",
        )
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    // A chat running past the pool's time keeps its runner.
    runtime.block_on(async {
        let identity = Identity {
            device: ours.device.clone(),
            key: ours.key.clone(),
        };
        let (client, mut views) = linked(&kubectl, &operator, &paired, identity).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the runner holds the project");
        ran(
            &client,
            &mut views,
            "echo 'kept' > \"$HOME/idle-kept\" && echo written; exec sleep 130",
            "written",
        )
        .await;
        tokio::time::sleep(Duration::from_secs(120)).await;
        assert_eq!(idle(), None, "a chat runs on it, so it is not idle");
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    // Its chat ends by itself and its link went with the runtime: nothing runs on it now.
    within(
        Duration::from_secs(420),
        "the claim says its runner is idle",
        || (idle().as_deref() == Some("True")).then_some(()),
    );
    within(
        Duration::from_secs(180),
        "the idle runner's pod ends",
        || {
            field(&kubectl, &runner, "pod", "ada-idle", "{.metadata.uid}")
                .is_none()
                .then_some(())
        },
    );
    field(
        &kubectl,
        &runner,
        "persistentvolumeclaim",
        "purlis-kept-ada-idle",
        "{.metadata.uid}",
    )
    .expect("its volume is kept");
    // The window's own start, through the desktop's host, runs it again, on its volume, with the
    // key it was paired with, and waits for it: nothing here writes a word or tries twice (#4403).
    let window_side = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    window_side.block_on(async {
        let host = desktops_host(&kubectl, &operator, &paired, &ours, &namespace);
        let (window, mut views) = a_window_on(host).await;
        window.relay_to("ada-idle").await.expect("relayed");
        tokio::time::timeout(Duration::from_secs(300), window.hold_project(PROJECT, ""))
            .await
            .expect("the hold waited, and no longer than its time")
            .expect("the runner came back and holds the project");
        let said = ran(
            &window,
            &mut views,
            "cat \"$HOME/idle-kept\"; echo finished",
            "finished",
        )
        .await;
        assert!(said.contains("kept"), "{said}");
    });
    window_side.shutdown_timeout(Duration::from_secs(5));
    let resumed = clusterpool::vouched_for(&kubectl, &operator, &kept)
        .expect("the statement checks")
        .expect("vouched for");
    clusterpool::held_to(&paired, &resumed).expect("the runner it was paired with");
    assert_eq!(resumed.key, paired.pinned.key);
    assert_eq!(idle(), None, "no longer idle");
    assert_eq!(
        in_the_pod(&kubectl, &runner, "ada-idle", "cat \"$HOME/idle-kept\"").trim(),
        "kept"
    );
    let _ = kubectl.delete_claim(&namespace, "ada-idle");
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
        kubectl.pool_pin(&pool).ok()
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
        // A profile kept there (#3819): nothing signed it in, and its harness says so, or that
        // it cannot tell yet.
        let standing = client
            .keep_profile(work())
            .await
            .expect("the runner keeps the profile");
        assert_ne!(standing, ProfileStanding::SignedIn, "nothing signed it in");
    });
    // Its standing, asked by the sign-in's own kubectl exec into the vouched pod.
    standing_by_exec(&kubectl, &runner, &paired.vouched.pod);
    // What a sign-in leaves in the profile's folder, in the runner's home on the kept volume.
    let folder = in_the_pod(&kubectl, &runner, &paired.vouched.pod, MARK_SIGN_IN);
    assert!(folder.trim().starts_with('/'), "{folder}");

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
        // The profile is still kept, and asked again (#3819).
        client
            .keep_profile(work())
            .await
            .expect("the runner keeps the profile again");
    });
    // A sign-in on a kept claim outlives the pod (#3819, spec #4023 Q6).
    assert_eq!(
        in_the_pod(&kubectl, &runner, &paired.vouched.pod, READ_SIGN_IN).trim(),
        "signed in before the pod went"
    );
    standing_by_exec(&kubectl, &runner, &paired.vouched.pod);

    // Stopped and run again (#2058): suspended, never destroyed, it is the same runner, paired
    // as it was.
    let stop = |stopped: bool| {
        kubectl
            .run(
                &kubectl.args(&[
                    "patch",
                    "claims.runners.purlis.dev",
                    "ada-kept",
                    "--namespace",
                    &namespace,
                    "--type",
                    "merge",
                    "-p",
                    &format!("{{\"spec\":{{\"stopped\":{stopped}}}}}"),
                ]),
                None,
                64 * 1024,
            )
            .expect("the claim patched");
    };
    stop(true);
    eventually("the stopped runner's pod ends", || {
        field(&kubectl, &runner, "pod", "ada-kept", "{.metadata.uid}")
            .is_none()
            .then_some(())
    });
    stop(false);
    let resumed = eventually("the operator vouches for the runner run again", || {
        let now = clusterpool::vouched_for(&kubectl, &operator, &kept).ok()??;
        (clusterpool::held_to(&paired, &now).is_ok()).then_some(now)
    });
    assert_eq!(resumed.key, paired.pinned.key);
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

/// `purlis` with `args`, on the machine at `machine`: what it printed to its error stream, and
/// whether it succeeded. Its input is closed, and it is held to [`PATIENCE`] twice over.
fn the_command(purlis: &str, machine: &std::path::Path, args: &[&str]) -> (bool, String) {
    use std::process::Stdio;
    let mut command = std::process::Command::new(purlis);
    command
        .args(args)
        .current_dir(machine.join("project"))
        .env("CHARTER_ROOT", machine.join("project"))
        .env("PURLIS_CONFIG_HOME", machine.join("config-home"))
        .env("PURLIS_DATA_HOME", machine.join("data"))
        .env_remove("PURLIS_CHAT")
        .env_remove("CHARTER_CHAT")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut running = purlis_core::forklock::spawn(&mut command).expect("the binary runs");
    let until = Instant::now() + PATIENCE * 2;
    while running.try_wait().expect("it is waited on").is_none() {
        if Instant::now() > until {
            let _ = running.kill();
            let out = running.wait_with_output().expect("it ended");
            panic!(
                "purlis {args:?} did not end: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let out = running.wait_with_output().expect("it ended");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
#[ignore = "needs a cluster with purlis's cluster operator: operator/e2e/kind.sh"]
fn the_command_signs_in_on_a_cluster_pool_only_marked_and_vouched_and_a_runner_forgets() {
    purlis_core::unsteered!();
    let image = std::env::var("CLUSTER_POOL_TEST_IMAGE")
        .expect("CLUSTER_POOL_TEST_IMAGE names the runner image by digest");
    let context = std::env::var("CLUSTER_POOL_TEST_CONTEXT")
        .expect("CLUSTER_POOL_TEST_CONTEXT names the cluster's kubeconfig context");
    let purlis = std::env::var("CLUSTER_POOL_TEST_PURLIS")
        .expect("CLUSTER_POOL_TEST_PURLIS names this build's binary");
    let machine = tempfile::tempdir().unwrap();
    for folder in ["config-home", "data", "project"] {
        std::fs::create_dir_all(machine.path().join(folder)).unwrap();
    }
    std::fs::write(machine.path().join("project/charter.toml"), "").unwrap();
    let config = machine.path().join("config-home");

    let kubectl = Kubectl::at("kubectl").with_context(&context);
    let suffix = ulid::Ulid::generate().to_string().to_lowercase();
    let namespace = format!("e2e-cli-{}", &suffix[..10]);
    let pool = format!("e2e-cli-{}", &suffix[..10]);
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
        kubectl.pool_pin(&pool).ok()
    });

    // Added as the window adds one: claimed, vouched for, paired and kept, and not marked.
    let ours = Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap(),
    };
    let desktop = Pinned {
        device: ours.device.clone(),
        key: ours.key.public(),
    };
    let kubeconfig = std::env::var_os("KUBECONFIG").map_or_else(
        || std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join(".kube/config"),
        std::path::PathBuf::from,
    );
    let team = purlis_core::runners::ClusterPool::new(
        "team",
        &kubeconfig,
        &context,
        &pool,
        &namespace,
        operator.clone(),
    )
    .expect("a cluster pool");
    let added = purlis_core::pools::add_cluster(
        &config,
        &purlis_core::pools::kubectl_here(&config).expect("kubectl on this machine"),
        &team,
        &desktop,
        PATIENCE * 2,
    )
    .expect("the pool is added");
    let paired = Paired {
        claimed: added.claimed.clone(),
        vouched: vouched(&kubectl, &operator, &added.claimed),
        pinned: added.pinned.clone(),
    };

    // Its runner keeps two profiles, named and forgotten over the link (#4183).
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (client, _views) = linked(&kubectl, &operator, &paired, ours).await;
        for name in ["work", "chosen-no-longer"] {
            let mut profile = work();
            profile.name = name.into();
            client.keep_profile(profile).await.expect("it keeps it");
        }
        let kept = client
            .kept_profiles()
            .await
            .expect("it names what it keeps");
        assert!(kept.iter().any(|one| one == "chosen-no-longer"), "{kept:?}");
        client
            .forget_profile("chosen-no-longer")
            .await
            .expect("it forgets it");
        let kept = client
            .kept_profiles()
            .await
            .expect("it names what it keeps");
        assert_eq!(kept, ["work"], "only what is chosen stays");
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    let gone = in_the_pod(
        &kubectl,
        &paired.vouched.runner_namespace,
        &paired.vouched.pod,
        "find \"$HOME\" -type d -path '*/profiles/chosen-no-longer' | wc -l",
    );
    assert_eq!(gone.trim(), "0", "its folder went with it");

    let (made, said) = the_command(
        &purlis,
        machine.path(),
        &["profile", "create", "work", "--harness", "claude"],
    );
    assert!(made, "{said}");
    let sign_in = ["profile", "sign-in", "work", "--pool", "team"];

    // Unmarked: refused, in words.
    let (signed_in, said) = the_command(&purlis, machine.path(), &sign_in);
    assert!(!signed_in);
    assert!(said.contains("not marked as your own"), "{said}");

    // Marked and vouched for: the gate passes, the runner's version is this one's, its standing
    // is asked over kubectl exec, and its own sign-in runs. The runner has no harness to log in
    // with, so no vendor is reached and it ends there.
    purlis_core::runners::set_cluster_owned(&config, "team", true).unwrap();
    let (signed_in, said) = the_command(&purlis, machine.path(), &sign_in);
    assert!(
        said.contains("asking pool team's runner whether it keeps 'work'"),
        "{said}"
    );
    assert!(!said.contains("could not confirm"), "{said}");
    assert!(!said.contains("could not say whether"), "{said}");
    assert!(!said.contains("so purlis signs nothing in"), "{said}");
    assert!(!signed_in, "nothing signed it in: {said}");

    // The mark taken back: refused again.
    purlis_core::runners::set_cluster_owned(&config, "team", false).unwrap();
    let (_, said) = the_command(&purlis, machine.path(), &sign_in);
    assert!(said.contains("not marked as your own"), "{said}");

    // Marked, with its claim stopped: not vouched for now, so refused, and nothing asked of it.
    purlis_core::runners::set_cluster_owned(&config, "team", true).unwrap();
    kubectl
        .run(
            &kubectl.args(&[
                "patch",
                "claims.runners.purlis.dev",
                &paired.claimed.name,
                "--namespace",
                &namespace,
                "--type",
                "merge",
                "-p",
                "{\"spec\":{\"stopped\":true}}",
            ]),
            None,
            64 * 1024,
        )
        .expect("the claim patched");
    let (signed_in, said) = the_command(&purlis, machine.path(), &sign_in);
    assert!(!signed_in);
    assert!(
        said.contains("could not confirm pool team's runner"),
        "{said}"
    );
    assert!(!said.contains("asking pool team's runner"), "{said}");
    let _ = kubectl.delete_claim(&namespace, &paired.claimed.name);
}
