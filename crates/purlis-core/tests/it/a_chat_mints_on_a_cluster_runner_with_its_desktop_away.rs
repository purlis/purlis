// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A whole chat on a cluster runner of a Pool that mints, on a kind cluster** (#4329). A stand-in
//! Claude Code, composed by the runner's own host inside purlis's sandbox as any chat is, runs its
//! persona's minting tool with its desktop gone. Its token comes from the operator, here from a
//! stand-in for GitHub's documented API. The desktop's side is the real one: the opt-in, the
//! Claim's repos, and the telling at a link, over the runner's own `purlis bridge`.
//!
//! Then what the sandbox holds, tried from inside that chat with the pod's real hand-over folder
//! mounted, and the refusal of a start outside the sandbox on such a runner, with a pool that names
//! no key as its control.
//!
//! Real GitHub is not tried here: see #4328. It needs what
//! [`a_chat_runs_on_a_cluster_runner_through_kubectl_exec`](super) needs, and `FAKE_GITHUB_PORT`, where
//! `operator/e2e/kind.sh` pointed the operator. It is ignored anywhere else, and fails when any is
//! missing.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::clusterpool::{self, Claimed, Keeping, Kubectl, Paired, PoolPin};
use purlis_core::sandbox::persona::digest_of;
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
};
use purlis_session_protocol::session::{Client, Launch, ProgramKind, StartProgram};

#[path = "../../../../operator/tests/app_key/mod.rs"]
mod app_key;

const PATIENCE: Duration = Duration::from_secs(300);
const APP: &str = "Iv23liE2e";
/// The Secret `operator/e2e/kind.sh` names in the chart's `forge.keySecretNames`.
const KEY_SECRET: &str = "e2e-forge-app-key";
const OPERATOR_NS: &str = "purlis-system";
const PROJECT: &str = "01J9PRJCT00000000000000000";
const TOKEN: &str = "e2e-a-stand-ins-token-for-a-whole-chat";
/// Where the stand-in harness is put, which no chat writes.
const HARNESS_AT: &str = "/home/purlis/e2e-bin/claude";

/// The chat, in the stand-in harness: it waits for its desktop to have gone, runs its persona's
/// tool, and writes what came of it, then what the sandbox holds of the pod's hand-over folder.
const THE_CHATS_SCRIPT: &str = r#"#!/bin/sh
for word in "$@"; do [ "$word" = --version ] && { echo '2.1.283 (Claude Code)'; exit 0; }; done
while [ ! -e go ]; do sleep 1; done
found=$(command -v printenv)
out=$(printenv GH_TOKEN 2>tool-said); code=$?
ls /run/purlis-link >/dev/null 2>&1; listed=$?
{
  echo "found $found"
  echo "tool $code"
  [ -n "$out" ] && echo handed-a-value
  echo "handover $listed"
} > said.part
mv said.part said
exec sleep 600
"#;

/// Removes what the test made, however it ends.
struct Removes {
    kubectl: Kubectl,
    namespace: String,
    pools: Vec<String>,
    binding: String,
}

impl Drop for Removes {
    fn drop(&mut self) {
        let mut each = vec![
            vec![
                "delete".to_owned(),
                "namespace".to_owned(),
                self.namespace.clone(),
                "--wait=false".to_owned(),
            ],
            vec![
                "delete".to_owned(),
                "clusterrolebinding,clusterrole".to_owned(),
                self.binding.clone(),
                "--ignore-not-found".to_owned(),
            ],
            vec![
                "delete".to_owned(),
                "secret".to_owned(),
                "--namespace".to_owned(),
                OPERATOR_NS.to_owned(),
                KEY_SECRET.to_owned(),
                "--ignore-not-found".to_owned(),
            ],
        ];
        for pool in &self.pools {
            each.push(vec![
                "delete".to_owned(),
                "pools.runners.purlis.dev".to_owned(),
                pool.clone(),
                "--wait=false".to_owned(),
            ]);
        }
        for args in each {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
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
            start.elapsed() < PATIENCE,
            "{what}: not within {PATIENCE:?}"
        );
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn applied(kubectl: &Kubectl, object: &serde_json::Value) {
    kubectl
        .run(
            &kubectl.args(&["apply", "--filename", "-"]),
            Some(&object.to_string()),
            64 * 1024,
        )
        .unwrap_or_else(|why| panic!("{object} was not applied: {why}"));
}

/// What the stand-in for GitHub was asked to mint: each token's body.
type Minted = Arc<Mutex<Vec<serde_json::Value>>>;

/// The two requests a mint makes, as GitHub's REST API documents them.
fn github_answer(head: &str, body: &[u8], minted: &Minted) -> String {
    let target = head.lines().next().unwrap_or_default();
    let (status, answer) =
        if target.starts_with("GET /repos/acme/") && target.contains("/installation ") {
            (
                "200 OK",
                serde_json::json!({
                    "id": 42,
                    "account": { "login": "acme", "type": "Organization" },
                    "permissions": { "contents": "write", "pull_requests": "read" },
                }),
            )
        } else if target.starts_with("POST /app/installations/42/access_tokens ") {
            let asked: serde_json::Value = serde_json::from_slice(body).expect("a JSON body");
            minted.lock().unwrap().push(asked.clone());
            let repositories: Vec<serde_json::Value> = asked["repositories"]
                .as_array()
                .expect("named repositories")
                .iter()
                .map(|name| serde_json::json!({ "name": name }))
                .collect();
            (
                "201 Created",
                serde_json::json!({
                    "token": TOKEN,
                    "expires_at": "2099-01-01T00:00:00Z",
                    "repository_selection": "selected",
                    "repositories": repositories,
                    "permissions": asked["permissions"],
                }),
            )
        } else {
            (
                "404 Not Found",
                serde_json::json!({ "message": "Not Found" }),
            )
        };
    let answer = answer.to_string();
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: \
         close\r\n\r\n{answer}",
        answer.len()
    )
}

/// Serves the stand-in on every address of this machine, for as long as the test runs.
fn stand_in_for_github(minted: Minted) {
    let port: u16 = std::env::var("FAKE_GITHUB_PORT")
        .expect("FAKE_GITHUB_PORT")
        .parse()
        .expect("a port");
    let listener = TcpListener::bind(("0.0.0.0", port)).expect("the stand-in's port");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let minted = minted.clone();
            std::thread::spawn(move || {
                let mut read = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let Ok(n) = stream.read(&mut buffer) else {
                        return;
                    };
                    read.extend_from_slice(&buffer[..n]);
                    let Some(end) = read.windows(4).position(|at| at == b"\r\n\r\n") else {
                        if n == 0 {
                            return;
                        }
                        continue;
                    };
                    let head = String::from_utf8_lossy(&read[..end]).into_owned();
                    let length: usize = head
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse().ok())?
                        })
                        .unwrap_or(0);
                    if read.len() >= end + 4 + length {
                        let answer =
                            github_answer(&head, &read[end + 4..end + 4 + length], &minted);
                        let _ = stream.write_all(answer.as_bytes());
                        return;
                    }
                    if n == 0 {
                        return;
                    }
                }
            });
        }
    });
}

/// Runs `script` with `sh -c` in the runner `pod` of `namespace`, handed `input`, and answers what
/// it printed.
fn in_the_pod(
    kubectl: &Kubectl,
    namespace: &str,
    pod: &str,
    script: &str,
    input: Option<&str>,
) -> String {
    let interactive = if input.is_some() { "-i" } else { "--tty=false" };
    kubectl
        .run(
            &kubectl.args(&[
                "exec",
                interactive,
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
            input,
            64 * 1024,
        )
        .unwrap_or_else(|why| panic!("{script} did not run in the runner's pod: {why}"))
}

/// A Pool `name` of `image`, opting in with the test's key where `minting`, once its operator key
/// is pinned; then a Claim on it, the operator's vouching, and a pairing over `kubectl exec`.
fn a_paired_runner(
    kubectl: &Kubectl,
    namespace: &str,
    name: &str,
    image: &str,
    minting: bool,
    desktop: &Pinned,
) -> (PoolPin, Paired) {
    let mut spec = serde_json::json!({ "image": image });
    if minting {
        spec["forge"] = serde_json::json!({
            "appKeySecretRef": { "name": KEY_SECRET },
            "repos": ["acme/*"],
            "mintInCluster": true,
        });
    }
    // Retried: use on the key takes a moment to reach the policy.
    let pool = serde_json::json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Pool",
        "metadata": { "name": name },
        "spec": spec,
    });
    eventually("the Pool to be taken", || {
        kubectl
            .run(
                &kubectl.args(&["apply", "--filename", "-"]),
                Some(&pool.to_string()),
                64 * 1024,
            )
            .ok()
    });
    let pin = eventually("the pool names the operator's key", || {
        kubectl.pool_pin(name).ok()
    });
    let requester = kubectl.whoami().expect("who this kubeconfig is");
    let uid = kubectl
        .claim(namespace, name, name, &requester, Keeping::Fresh)
        .expect("the claim is made");
    let claimed = Claimed {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        uid,
        requester,
    };
    let vouched = eventually("the operator vouches for the runner", || {
        clusterpool::vouched_for(kubectl, &pin, &claimed).expect("the claim reads")
    });
    let paired =
        clusterpool::pair(kubectl, &claimed, &vouched, desktop).expect("the runner is paired");
    (pin, paired)
}

/// A link to `paired`'s runner, as a desktop's: and the tasks that keep it.
async fn linked(kubectl: &Kubectl, pin: &PoolPin, paired: &Paired, ours: Identity) -> Client {
    let connector = CommandConnector::new(
        kubectl.program(),
        kubectl.connect_args(&paired.vouched.runner_namespace, &paired.vouched.pod),
        {
            let (kubectl, pin, paired) = (kubectl.clone(), pin.clone(), paired.clone());
            Arc::new(move || {
                clusterpool::still_vouched(&kubectl, &pin, &paired).map_err(|why| why.to_string())
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
            let (client, _views, _events) = Client::new(*link);
            tokio::spawn(async move { while links.next().await.is_some() {} });
            client
        }
        Ok(Some(Linked::Down(down))) => panic!("not linked: {}", down.why),
        Ok(None) => panic!("the kept link ended"),
        Err(_) => panic!("no link within the test's patience"),
    }
}

/// A start of `/bin/sh` outside any sandbox, of `kind`, by its words, as a link may name one.
fn outside_the_sandbox(kind: ProgramKind) -> StartProgram {
    StartProgram {
        launch: Some(Launch {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "exec sleep 60".into()],
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
        ..StartProgram::of(PROJECT, kind)
    }
}

/// The project a chat runs in, written here as a person's is, with this machine's consent to its
/// profile and its tool's digest: packed for the runner.
fn the_project(into: &Path) -> (String, Vec<u8>) {
    let manifest = format!(
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n\n\
         [sandbox.personas.steward]\ntools = {{ printenv = {{ vault = \"forge\", mint = \
         \"github-app\", app = \"{APP}\", key = \"app-key\" }} }}\n"
    );
    std::fs::write(into.join("purlis.toml"), &manifest).expect("the manifest");
    std::fs::create_dir_all(into.join("personas/steward")).expect("its persona");
    std::fs::write(into.join("personas/steward/persona.md"), "# steward\n").expect("its persona");
    std::fs::write(
        into.join(purlis_core::profiles::LOCAL_FILE),
        format!("[harness.work]\nkind = \"claude\"\ncommand = [{HARNESS_AT:?}]\n"),
    )
    .expect("its profile");
    let declared = purlis_core::profiles::current(into);
    purlis_core::profiletrust::record_launched(
        into,
        "work",
        &purlis_core::profiletrust::fingerprint(declared.get("work").expect("declared")),
    )
    .expect("this machine's consent");
    let tools = purlis_core::sandbox::Plane::of(Some(&manifest))
        .said()
        .policy
        .expect("the sandbox on")
        .personas["steward"]
        .tools
        .clone();
    let digest = digest_of(&[], &tools, false);
    let packed = purlis_core::forklock::output(
        std::process::Command::new("tar")
            .args(["--create", "--directory"])
            .arg(into)
            .arg("."),
    )
    .expect("tar runs");
    assert!(packed.status.success(), "the project was not packed");
    (digest, packed.stdout)
}

#[test]
#[ignore = "needs a cluster with purlis's cluster operator: operator/e2e/kind.sh"]
fn a_chat_runs_its_minting_tool_in_its_sandbox_with_its_desktop_away() {
    purlis_core::unsteered!();
    let image = std::env::var("CLUSTER_POOL_TEST_IMAGE")
        .expect("CLUSTER_POOL_TEST_IMAGE names the runner image by digest");
    let context = std::env::var("CLUSTER_POOL_TEST_CONTEXT")
        .expect("CLUSTER_POOL_TEST_CONTEXT names the cluster's kubeconfig context");
    let kubectl = Kubectl::at("kubectl").with_context(&context);
    let suffix = ulid::Ulid::generate().to_string().to_lowercase();
    let namespace = format!("e2e-whole-{}", &suffix[..10]);
    let minting = format!("e2e-whole-{}", &suffix[..10]);
    let plain = format!("e2e-plain-{}", &suffix[..10]);
    let binding = format!("e2e-whole-{}", &suffix[..10]);
    let _removes = Removes {
        kubectl: kubectl.clone(),
        namespace: namespace.clone(),
        pools: vec![minting.clone(), plain.clone()],
        binding: binding.clone(),
    };
    let minted: Minted = Arc::default();
    stand_in_for_github(minted.clone());

    kubectl
        .run(
            &kubectl.args(&["create", "namespace", &namespace]),
            None,
            64 * 1024,
        )
        .expect("a namespace of the test's own");
    applied(
        &kubectl,
        &serde_json::json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "metadata": { "name": KEY_SECRET, "namespace": OPERATOR_NS },
            "stringData": { "app": APP, "key": app_key::pem() },
        }),
    );
    // By name, since a right a group holds is refused at a mint: the requester uses the Pool, and
    // its writer the key.
    let who = kubectl.whoami().expect("who this kubeconfig is");
    applied(
        &kubectl,
        &serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1",
            "kind": "ClusterRole",
            "metadata": { "name": binding },
            "rules": [
                { "apiGroups": ["runners.purlis.dev"], "resources": ["pools"],
                  "resourceNames": [minting], "verbs": ["use"] },
                { "apiGroups": ["runners.purlis.dev"], "resources": ["forgekeys"],
                  "resourceNames": [KEY_SECRET], "verbs": ["use"] },
            ],
        }),
    );
    applied(
        &kubectl,
        &serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1",
            "kind": "ClusterRoleBinding",
            "metadata": { "name": binding },
            "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": binding },
            "subjects": [{ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": who }],
        }),
    );

    let ours = Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap(),
    };
    let desktop = Pinned {
        device: ours.device.clone(),
        key: ours.key.public(),
    };
    let (pin, paired) = a_paired_runner(&kubectl, &namespace, &minting, &image, true, &desktop);
    let (runners, pod) = (
        paired.vouched.runner_namespace.clone(),
        paired.vouched.pod.clone(),
    );

    // The desktop's side, as a link does it: the Claim's repos, and the runner told by the word it
    // is told by at a link, over its own `purlis bridge` (#4330).
    kubectl
        .add_claim_repos(&namespace, &minting, &["acme/app".to_owned()])
        .expect("the workspace's repos reach the Claim");
    let seen = kubectl.pool(&minting).expect("the pool reads");
    let told = eventually("the Claim's status says where to ask", || {
        clusterpool::to_tell_at_a_link(&seen, &kubectl, &pin, &paired.claimed).expect("it reads")
    });
    for _ in 0..2 {
        assert!(
            clusterpool::tell_again(&kubectl, &runners, &pod, &told).expect("told at a link"),
            "the port shows the key it is told"
        );
    }

    // The stand-in harness, in a folder no chat writes.
    in_the_pod(
        &kubectl,
        &runners,
        &pod,
        &format!(
            "mkdir -p \"$(dirname {HARNESS_AT})\" && cat > {HARNESS_AT} && chmod 0755 {HARNESS_AT}"
        ),
        Some(THE_CHATS_SCRIPT),
    );
    let packing = tempfile::tempdir().expect("a folder for the project");
    let (digest, packed) = the_project(packing.path());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let root = runtime.block_on(async {
        let ours = Identity {
            device: ours.device.clone(),
            key: ours.key.clone(),
        };
        let client = linked(&kubectl, &pin, &paired, ours).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the runner holds the project where it keeps it");

        // A start outside the sandbox is refused from the runner's first moment, saying why.
        let refused = client
            .start_program(outside_the_sandbox(ProgramKind::Harness))
            .await
            .expect_err("a start outside the sandbox on a Pool that mints");
        assert!(
            refused
                .to_string()
                .contains("mints GitHub tokens in the cluster"),
            "{refused}"
        );
        // A shell the person opens is the one thing that is not.
        let shell = client
            .start_program(outside_the_sandbox(ProgramKind::Shell))
            .await
            .expect("a shell starts");
        client.stop(&shell).await.expect("it stops");

        // The project, unpacked where the runner holds it.
        let root = in_the_pod(
            &kubectl,
            &runners,
            &pod,
            &format!("find \"$HOME\" -type d -path '*/projects/{PROJECT}' | head -n 1"),
            None,
        )
        .trim()
        .to_owned();
        assert!(!root.is_empty(), "the runner holds the project somewhere");
        let mut unpack = std::process::Command::new(kubectl.program());
        unpack
            .args(kubectl.args(&[
                "exec",
                "-i",
                "--namespace",
                &runners,
                "--container",
                clusterpool::RUNNER_CONTAINER,
                &format!("pod/{pod}"),
                "--",
                "tar",
                "--extract",
                "--directory",
                &root,
            ]))
            .stdin(std::process::Stdio::piped());
        let mut unpacking = purlis_core::forklock::spawn(&mut unpack).expect("kubectl exec runs");
        unpacking
            .stdin
            .take()
            .expect("its input")
            .write_all(&packed)
            .expect("the project is sent");
        assert!(unpacking.wait().expect("it ends").success(), "not unpacked");

        // The chat, composed by the runner's own host, in its sandbox.
        client
            .start_program(StartProgram {
                program: Some("01J9CHAT000000000000000000".into()),
                run: Some("01J9RVN0000000000000000000".into()),
                persona: Some("steward".into()),
                profile: Some("work".into()),
                tools_allowed: Some(digest),
                ..StartProgram::of(PROJECT, ProgramKind::Harness)
            })
            .await
            .expect("the chat starts in its sandbox");
        root
    });
    // The desktop goes: every task of this link with it. What the chat does now, it does alone.
    runtime.shutdown_timeout(Duration::from_secs(5));
    assert!(
        in_the_pod(
            &kubectl,
            &runners,
            &pod,
            "ls /run/purlis-link && echo reached",
            None
        )
        .contains("reached"),
        "outside the sandbox the hand-over folder is there, so a refusal inside is the sandbox's"
    );
    in_the_pod(&kubectl, &runners, &pod, &format!("touch {root}/go"), None);

    let said = eventually("the chat says what came of its tool", || {
        let said = in_the_pod(
            &kubectl,
            &runners,
            &pod,
            &format!("cat {root}/said 2>/dev/null; true"),
            None,
        );
        (!said.is_empty()).then_some(said)
    });
    let shown = || {
        format!(
            "{said}\nthe tool said: {}",
            in_the_pod(
                &kubectl,
                &runners,
                &pod,
                &format!("cat {root}/tool-said 2>/dev/null; true"),
                None
            )
        )
    };
    assert!(said.contains("/tools/steward/bin/printenv"), "{}", shown());
    assert!(said.contains("tool 0"), "{}", shown());
    assert!(said.contains("handed-a-value"), "{}", shown());
    assert!(
        said.lines()
            .any(|line| line.starts_with("handover ") && line != "handover 0"),
        "the chat listed the pod's hand-over folder: {}",
        shown()
    );
    let minted = minted.lock().unwrap().clone();
    assert_eq!(minted.len(), 1, "{minted:?}");
    assert_eq!(minted[0]["repositories"], serde_json::json!(["app"]));

    // The control: a pool naming no key refuses no such start.
    let (plain_pin, plain_paired) =
        a_paired_runner(&kubectl, &namespace, &plain, &image, false, &desktop);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let client = linked(&kubectl, &plain_pin, &plain_paired, ours).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("it holds the project");
        let started = client
            .start_program(outside_the_sandbox(ProgramKind::Harness))
            .await
            .expect("a pool that mints nothing starts it");
        client.stop(&started).await.expect("it stops");
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
}
