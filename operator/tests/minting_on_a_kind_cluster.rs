// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator mints for a vouched runner (#4206), live, on the kind cluster
//! `operator/e2e/kind.sh` makes, against a stand-in for GitHub this test serves: no App of ours is
//! on real GitHub for a pull request's run, so what the stand-in answers is the documented API, not
//! GitHub itself.
//!
//! The runner's link key is read out of its pod, as only the cluster's admin could, and the proof
//! is made with it here: `openssl` does the X25519 agreement with a key that already exists. The
//! requests go through `curl`, pinned to the signing key every Pool's status names, as a runner
//! does. Then the runner image's own client asks from inside its pod (#4207). What runs a chat's
//! tool on that token is tested apart, and a token's revoke goes to GitHub, not this stand-in.

use base64::Engine as _;
use k8s_openapi::api::core::v1::{Pod, Secret};
use k8s_openapi::api::networking::v1::NetworkPolicy;
use kube::api::{
    Api, DeleteParams, ListParams, LogParams, ObjectMeta, Patch, PatchParams, PostParams,
};
use kube::{Client, ResourceExt};
use purlis_cluster_operator::mint::{self, CHALLENGE_PATH, TOKEN_PATH};
use purlis_cluster_operator::{Claim, ClaimForge, ClaimSpec, Pool, PoolForge, PoolSpec, SecretRef};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod app_key;

const APP: &str = "Iv23liE2e";
/// The Secret kind.sh names in the chart's `forge.keySecretNames`.
const KEY_SECRET: &str = "e2e-forge-app-key";
const OPERATOR_NS: &str = "purlis-system";
const ADA: &str = "e2e-mint-ada";
const WRITER: &str = "e2e-mint-writer";
const NAMESPACE: &str = "e2e-mint";
const POOL: &str = "e2e-mint";
const CLAIM: &str = "e2e-mint";
const TOKEN: &str = "e2e-token-minted-for-the-kind-job";
const FORWARDED: u16 = 18443;

fn runner_image() -> String {
    std::env::var("PURLIS_RUNNER_IMAGE").expect("PURLIS_RUNNER_IMAGE")
}

async fn eventually<T, F, Fut>(what: &str, within: Duration, mut check: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let start = Instant::now();
    loop {
        if let Some(found) = check().await {
            return found;
        }
        assert!(start.elapsed() < within, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn acting_as(user: &str) -> Client {
    let mut config = kube::Config::infer().await.expect("the cluster");
    config.auth_info.impersonate = Some(user.to_owned());
    Client::try_from(config).expect("a client")
}

/// What the stand-in for GitHub was asked to mint.
#[derive(Default)]
struct Asked {
    tokens: Vec<Value>,
    bearers: Vec<String>,
}

fn answer(status: &str, body: &Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The two requests a mint makes, as GitHub's REST API documents them.
fn github_answer(head: &str, body: &[u8], asked: &Mutex<Asked>) -> String {
    let target = head.lines().next().unwrap_or_default();
    let bearer = head
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("authorization:"))
        .and_then(|line| line.split_once("Bearer "))
        .map(|(_, bearer)| bearer.trim().to_owned())
        .unwrap_or_default();
    let mut asked = asked.lock().expect("the record");
    asked.bearers.push(bearer);
    if target.starts_with("GET /repos/acme/") && target.contains("/installation ") {
        return answer(
            "200 OK",
            &json!({
                "id": 42,
                "account": { "login": "acme", "type": "Organization" },
                "permissions": {
                    "contents": "write", "pull_requests": "read", "administration": "write",
                },
            }),
        );
    }
    if target.starts_with("POST /app/installations/42/access_tokens ") {
        let body: Value = serde_json::from_slice(body).expect("a JSON body");
        asked.tokens.push(body.clone());
        let repositories: Vec<Value> = body["repositories"]
            .as_array()
            .expect("named repositories")
            .iter()
            .map(|name| json!({ "name": name }))
            .collect();
        return answer(
            "201 Created",
            &json!({
                "token": TOKEN,
                "expires_at": "2099-01-01T00:00:00Z",
                "repository_selection": "selected",
                "repositories": repositories,
                "permissions": body["permissions"],
            }),
        );
    }
    answer("404 Not Found", &json!({ "message": "Not Found" }))
}

/// Serves the stand-in on every address of this machine, for as long as the test runs.
async fn stand_in_for_github(asked: Arc<Mutex<Asked>>) {
    let port: u16 = std::env::var("FAKE_GITHUB_PORT")
        .expect("FAKE_GITHUB_PORT")
        .parse()
        .expect("a port");
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("the stand-in's port");
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            let asked = asked.clone();
            tokio::spawn(async move {
                let mut read = Vec::new();
                let mut buffer = [0u8; 4096];
                let answered = loop {
                    let Ok(n) = stream.read(&mut buffer).await else {
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
                        break github_answer(&head, &read[end + 4..end + 4 + length], &asked);
                    }
                    if n == 0 {
                        return;
                    }
                };
                let _ = stream.write_all(answered.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
}

/// `kubectl port-forward` to the mint Service, ended when dropped. A way in for this machine alone:
/// that a runner can reach the port itself is asked from inside its pod.
struct Forward(Child);

impl Forward {
    fn start() -> Forward {
        Forward(
            Command::new("kubectl")
                .args([
                    "-n",
                    OPERATOR_NS,
                    "port-forward",
                    "service/purlis-cluster-operator-mint",
                    &format!("{FORWARDED}:{}", mint::PORT),
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("kubectl"),
        )
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `sha256//<base64>` of the Ed25519 key `signing_key`, as `curl --pinnedpubkey` reads it.
fn pin_of(signing_key: &str) -> String {
    let hex = signing_key.strip_prefix("ed25519:").expect("a key");
    let mut der = unhex("302a300506032b6570032100");
    der.extend(unhex(hex));
    let digest = ring::digest::digest(&ring::digest::SHA256, &der);
    format!(
        "sha256//{}",
        base64::engine::general_purpose::STANDARD.encode(digest.as_ref())
    )
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

/// `POST`s `body` to the mint port through `curl`, pinned to `pin`: its status and body.
fn post(pin: &str, path: &str, body: &Value) -> (u16, Value) {
    let out = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--max-time",
            "45",
            "--insecure",
            "--pinnedpubkey",
            pin,
            "--tlsv1.3",
            "--write-out",
            "\n%{http_code}",
            "--data-binary",
            "@-",
            &format!("https://127.0.0.1:{FORWARDED}{path}"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .take()
                .expect("its input")
                .write_all(body.to_string().as_bytes())?;
            child.wait_with_output()
        })
        .expect("curl");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let Some((said, status)) = text.rsplit_once('\n') else {
        return (0, json!({ "curl": String::from_utf8_lossy(&out.stderr) }));
    };
    (
        status.trim().parse().unwrap_or(0),
        serde_json::from_str(said).unwrap_or(Value::Null),
    )
}

/// The X25519 agreement of the raw private key `private` and public key `public`, by `openssl`.
fn agreed(private: &[u8], public: &[u8]) -> Vec<u8> {
    let scratch = std::env::temp_dir().join(format!("purlis-e2e-mint-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("a folder");
    let mut private_der = unhex("302e020100300506032b656e04220420");
    private_der.extend_from_slice(private);
    let mut public_der = unhex("302a300506032b656e032100");
    public_der.extend_from_slice(public);
    let (ours, theirs) = (scratch.join("ours.der"), scratch.join("theirs.der"));
    std::fs::write(&ours, private_der).expect("written");
    std::fs::write(&theirs, public_der).expect("written");
    let out = Command::new("openssl")
        .args([
            "pkeyutl",
            "-derive",
            "-keyform",
            "DER",
            "-peerform",
            "DER",
            "-inkey",
        ])
        .arg(&ours)
        .arg("-peerkey")
        .arg(&theirs)
        .output()
        .expect("openssl");
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(
        out.status.success(),
        "openssl: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// The signing key the Pool names, which each proof is made for.
static OPERATOR: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Asks for a token, as the holder of `link_key` (32 raw bytes).
fn ask(pin: &str, link_key: &[u8], claim: &str, app: &str) -> (u16, Value) {
    let (status, said) = post(pin, CHALLENGE_PATH, &json!({}));
    assert_eq!(status, 200, "{said}");
    let challenge = said["challenge"].as_str().expect("a challenge").to_owned();
    let bytes = unhex(&challenge);
    let operator = OPERATOR.get().expect("the signing key");
    let text = mint::proof_text(operator, NAMESPACE, claim, app, &challenge);
    let proof = mint::proof_for(&agreed(link_key, &bytes), &bytes, &text).expect("a proof");
    post(
        pin,
        TOKEN_PATH,
        &json!({
            "namespace": NAMESPACE, "claim": claim, "app": app,
            "challenge": challenge, "proof": proof,
        }),
    )
}

fn refused_saying(answered: &(u16, Value), words: &str) {
    assert_eq!(answered.0, 403, "{answered:?}");
    let said = answered.1["refused"].as_str().unwrap_or_default();
    assert!(said.contains(words), "{said:?} does not say {words:?}");
    assert!(answered.1.get("token").is_none(), "{answered:?}");
}

fn exec(namespace: &str, pod: &str, container: &str, command: &[&str]) -> std::process::Output {
    Command::new("kubectl")
        .args(["-n", namespace, "exec", pod, "-c", container, "--"])
        .args(command)
        .output()
        .expect("kubectl")
}

/// [`exec`], `input` on its standard input.
fn exec_fed(namespace: &str, pod: &str, command: &[&str], input: &str) -> std::process::Output {
    let mut run = Command::new("kubectl")
        .args(["-n", namespace, "exec", "-i", pod, "-c", "runner", "--"])
        .args(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("kubectl");
    run.stdin
        .take()
        .expect("its input")
        .write_all(input.as_bytes())
        .expect("written");
    run.wait_with_output().expect("kubectl")
}

async fn apply_rbac(client: &Client, body: Value) {
    let kind = body["kind"].as_str().expect("a kind").to_owned();
    let resource = kube::api::ApiResource {
        group: "rbac.authorization.k8s.io".into(),
        version: "v1".into(),
        api_version: "rbac.authorization.k8s.io/v1".into(),
        plural: format!("{}s", kind.to_lowercase()),
        kind: kind.clone(),
    };
    let api: Api<kube::api::DynamicObject> = Api::all_with(client.clone(), &resource);
    let object: kube::api::DynamicObject = serde_json::from_value(body).expect("an RBAC object");
    api.patch(
        &object.name_any(),
        &PatchParams::apply("e2e").force(),
        &Patch::Apply(&object),
    )
    .await
    .unwrap_or_else(|error| panic!("{kind}: {error}"));
}

/// A ClusterRole of one rule, bound to `user`.
async fn grant(client: &Client, name: &str, user: &str, rule: Value) {
    apply_rbac(
        client,
        json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRole",
            "metadata": { "name": name }, "rules": [rule],
        }),
    )
    .await;
    apply_rbac(
        client,
        json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRoleBinding",
            "metadata": { "name": name },
            "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": name },
            "subjects": [{ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": user }],
        }),
    )
    .await;
}

async fn revoke(client: &Client, name: &str) {
    let params = DeleteParams::default();
    let _ = Api::<k8s_openapi::api::rbac::v1::ClusterRoleBinding>::all(client.clone())
        .delete(name, &params)
        .await;
    let _ = Api::<k8s_openapi::api::rbac::v1::ClusterRole>::all(client.clone())
        .delete(name, &params)
        .await;
}

fn pool_spec(key: &str, repos: &[&str]) -> PoolSpec {
    let mut spec = PoolSpec::new(runner_image());
    spec.forge = Some(PoolForge {
        app_key_secret_ref: SecretRef {
            name: key.to_owned(),
        },
        repos: repos.iter().map(|repo| (*repo).to_owned()).collect(),
        mint_in_cluster: true,
    });
    spec
}

fn repos_patch(repos: &[&str]) -> Patch<Value> {
    Patch::Merge(json!({ "spec": { "forge": { "repos": repos } } }))
}

async fn operator_log(client: &Client) -> String {
    let pods = Api::<Pod>::namespaced(client.clone(), OPERATOR_NS);
    let listed = pods
        .list(&ListParams::default().labels("app.kubernetes.io/name=purlis-cluster-operator"))
        .await
        .expect("the operator's pod");
    pods.logs(
        &listed.items[0].name_any(),
        &LogParams {
            tail_lines: Some(2000),
            ..LogParams::default()
        },
    )
    .await
    .expect("its log")
}

async fn clean(client: &Client) {
    let params = DeleteParams::default();
    let _ = Api::<Claim>::namespaced(client.clone(), NAMESPACE)
        .delete(CLAIM, &params)
        .await;
    let _ = Api::<Claim>::namespaced(client.clone(), NAMESPACE)
        .delete("e2e-mint-unvouched", &params)
        .await;
    let claims = Api::<Claim>::namespaced(client.clone(), NAMESPACE);
    eventually("the claims to go", Duration::from_secs(120), || {
        let claims = claims.clone();
        async move {
            claims
                .list(&ListParams::default())
                .await
                .ok()
                .filter(|listed| listed.items.is_empty())
                .map(|_| ())
        }
    })
    .await;
    let pools = Api::<Pool>::all(client.clone());
    let _ = pools.delete(POOL, &params).await;
    let _ = Api::<Secret>::namespaced(client.clone(), OPERATOR_NS)
        .delete(KEY_SECRET, &params)
        .await;
    for name in ["e2e-mint-pool-writer", "e2e-mint-key", "e2e-mint-use"] {
        revoke(client, name).await;
    }
    let _ = Api::<k8s_openapi::api::core::v1::Namespace>::all(client.clone())
        .delete(NAMESPACE, &params)
        .await;
}

/// The whole path, in one test so one runner serves it: admission on who may spend the key, the
/// way from a runner's pod to the port, the refusals, the mint, and what was left behind.
// Several threads: curl is waited for here while the stand-in answers the operator.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_vouched_runner_gets_a_token_covering_exactly_the_intersection_and_everyone_else_a_refusal()
 {
    let admin = Client::try_default().await.expect("the cluster");
    let asked = Arc::new(Mutex::new(Asked::default()));
    stand_in_for_github(asked.clone()).await;

    let namespaces = Api::<k8s_openapi::api::core::v1::Namespace>::all(admin.clone());
    namespaces
        .create(
            &PostParams::default(),
            &k8s_openapi::api::core::v1::Namespace {
                metadata: ObjectMeta {
                    name: Some(NAMESPACE.to_owned()),
                    ..ObjectMeta::default()
                },
                ..Default::default()
            },
        )
        .await
        .expect("the namespace");

    let secret = Secret {
        metadata: ObjectMeta {
            name: Some(KEY_SECRET.to_owned()),
            ..ObjectMeta::default()
        },
        string_data: Some(BTreeMap::from([
            ("app".to_owned(), APP.to_owned()),
            ("key".to_owned(), app_key::pem()),
        ])),
        ..Secret::default()
    };
    Api::<Secret>::namespaced(admin.clone(), OPERATOR_NS)
        .create(&PostParams::default(), &secret)
        .await
        .expect("the App's key");

    // Writing a Pool is not enough to spend a key: the writer needs `use` on it.
    grant(
        &admin,
        "e2e-mint-pool-writer",
        WRITER,
        json!({ "apiGroups": ["runners.purlis.dev"], "resources": ["pools"], "verbs": ["create", "patch", "get"] }),
    )
    .await;
    let writer = acting_as(WRITER).await;
    let as_writer = Api::<Pool>::all(writer.clone());
    let pool = Pool::new(
        POOL,
        pool_spec(
            KEY_SECRET,
            &["acme/app", "acme/web", "acme/docs", "other/*"],
        ),
    );
    let refused = eventually(
        "the policy to refuse the writer",
        Duration::from_secs(30),
        || {
            let (as_writer, pool) = (as_writer.clone(), pool.clone());
            async move {
                match as_writer.create(&PostParams::default(), &pool).await {
                    Err(kube::Error::Api(status)) if status.message.contains("forgekeys") => {
                        Some(status.message)
                    }
                    _ => None,
                }
            }
        },
    )
    .await;
    assert!(refused.contains(KEY_SECRET), "{refused}");

    grant(
        &admin,
        "e2e-mint-key",
        WRITER,
        json!({
            "apiGroups": ["runners.purlis.dev"], "resources": ["forgekeys"],
            "resourceNames": [KEY_SECRET], "verbs": ["use"],
        }),
    )
    .await;
    eventually(
        "use on the key to take the Pool",
        Duration::from_secs(30),
        || {
            let (as_writer, pool) = (as_writer.clone(), pool.clone());
            async move { as_writer.create(&PostParams::default(), &pool).await.ok() }
        },
    )
    .await;
    // `use` on this key says nothing of another.
    let other_key = as_writer
        .patch(
            POOL,
            &PatchParams::default(),
            &Patch::Merge(
                json!({ "spec": { "forge": { "appKeySecretRef": { "name": "another-key" } } } }),
            ),
        )
        .await;
    assert!(other_key.is_err(), "{other_key:?}");

    grant(
        &admin,
        "e2e-mint-use",
        ADA,
        json!({
            "apiGroups": ["runners.purlis.dev"], "resources": ["pools"],
            "resourceNames": [POOL], "verbs": ["use"],
        }),
    )
    .await;
    grant(
        &admin,
        "e2e-mint-claims",
        ADA,
        json!({ "apiGroups": ["runners.purlis.dev"], "resources": ["claims"], "verbs": ["create", "get", "patch", "delete"] }),
    )
    .await;
    let ada = acting_as(ADA).await;
    let claims = Api::<Claim>::namespaced(ada.clone(), NAMESPACE);
    let spec = |repos: &[&str]| ClaimSpec {
        pool: POOL.to_owned(),
        requester: ADA.to_owned(),
        kept: false,
        stopped: false,
        forge: Some(ClaimForge {
            repos: repos.iter().map(|repo| (*repo).to_owned()).collect(),
        }),
    };
    let claim = Claim::new(
        CLAIM,
        spec(&["acme/app", "acme/web", "acme/secrets", "elsewhere/x"]),
    );
    eventually("the Claim to be taken", Duration::from_secs(30), || {
        let (claims, claim) = (claims.clone(), claim.clone());
        async move { claims.create(&PostParams::default(), &claim).await.ok() }
    })
    .await;

    // The runner, vouched, and its key out of its pod.
    let read = Api::<Claim>::namespaced(admin.clone(), NAMESPACE);
    let held = eventually(
        "the runner to be vouched for",
        Duration::from_secs(300),
        || {
            let read = read.clone();
            async move {
                read.get(CLAIM)
                    .await
                    .ok()
                    .filter(|claim| claim.status.as_ref().is_some_and(|s| s.link_key.is_some()))
            }
        },
    )
    .await;
    let statement = held
        .status
        .as_ref()
        .and_then(|status| status.link_key.as_ref())
        .expect("a statement")
        .statement
        .clone();
    let line = |name: &str| {
        statement
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}: ")))
            .expect(name)
            .to_owned()
    };
    let (runner_ns, pod) = (line("runner namespace"), line("pod"));
    // A statement needs only the pod's first step ended: the runner may yet be starting.
    let out = eventually("the runner to run", Duration::from_secs(120), || {
        let (runner_ns, pod) = (runner_ns.clone(), pod.clone());
        async move {
            let out = exec(
                &runner_ns,
                &pod,
                "runner",
                &["cat", "/run/purlis-link/link-key"],
            );
            out.status.success().then_some(out)
        }
    })
    .await;
    let link_key = unhex(String::from_utf8_lossy(&out.stdout).trim());
    assert_eq!(link_key.len(), 32);

    // Its namespace has its way out to the port, and nothing but the port.
    let policy = Api::<NetworkPolicy>::namespaced(admin.clone(), &runner_ns)
        .get(purlis_cluster_operator::forge::MINT_EGRESS_POLICY)
        .await
        .expect("the way out");
    assert_eq!(
        policy.spec.and_then(|spec| spec.policy_types),
        Some(vec!["Egress".to_owned()])
    );
    let service =
        Api::<k8s_openapi::api::core::v1::Service>::namespaced(admin.clone(), OPERATOR_NS)
            .get("purlis-cluster-operator-mint")
            .await
            .expect("the Service");
    let address = service
        .spec
        .and_then(|spec| spec.cluster_ip)
        .expect("its address");
    let reach = format!("exec 3<>/dev/tcp/{address}/{}", mint::PORT);
    let from_runner = exec(
        &runner_ns,
        &pod,
        "runner",
        &["timeout", "20", "bash", "-c", &reach],
    );
    assert!(
        from_runner.status.success(),
        "a runner could not reach the port: {}",
        String::from_utf8_lossy(&from_runner.stderr)
    );
    let operators = Api::<Pod>::namespaced(admin.clone(), OPERATOR_NS)
        .list(&ListParams::default().labels("app.kubernetes.io/name=purlis-cluster-operator"))
        .await
        .expect("the operator's pod");
    let operator_ip = operators.items[0]
        .status
        .as_ref()
        .and_then(|status| status.pod_ip.clone())
        .expect("its address");
    let metrics = format!("exec 3<>/dev/tcp/{operator_ip}/8080");
    let not_metrics = exec(
        &runner_ns,
        &pod,
        "runner",
        &["timeout", "5", "bash", "-c", &metrics],
    );
    assert!(
        !not_metrics.status.success(),
        "a runner reached another port"
    );

    let pool_status = Api::<Pool>::all(admin.clone())
        .get(POOL)
        .await
        .expect("the pool");
    let signing_key = pool_status
        .status
        .and_then(|status| status.signing_key)
        .expect("the key the Pool names");
    let pin = pin_of(&signing_key);
    OPERATOR.set(signing_key).expect("set once");
    let _forward = Forward::start();
    eventually("the port to answer", Duration::from_secs(30), || {
        let pin = pin.clone();
        async move {
            let (status, _) = post(&pin, CHALLENGE_PATH, &json!({}));
            (status == 200).then_some(())
        }
    })
    .await;

    // A certificate of any other key is not the operator's: curl refuses it.
    let wrong = post(
        &pin_of(&format!("ed25519:{}", "11".repeat(32))),
        CHALLENGE_PATH,
        &json!({}),
    );
    assert_eq!(wrong.0, 0, "{wrong:?}");

    // Another key on this claim: told nothing but that.
    let refused = ask(&pin, &[9u8; 32], CLAIM, APP);
    refused_saying(&refused, mint::NOT_VOUCHED);
    // A claim with no vouched runner, and none at all: the same words.
    claims
        .create(
            &PostParams::default(),
            &Claim::new("e2e-mint-unvouched", spec(&["acme/app"])),
        )
        .await
        .expect("a claim");
    refused_saying(
        &ask(&pin, &link_key, "e2e-mint-unvouched", APP),
        mint::NOT_VOUCHED,
    );
    refused_saying(
        &ask(&pin, &link_key, "e2e-mint-none", APP),
        mint::NOT_VOUCHED,
    );

    // A tool line naming another App (ruling Q6).
    refused_saying(&ask(&pin, &link_key, CLAIM, "Iv23liOther"), "Iv23liOther");

    // The mint: exactly the intersection, by name, from one installation.
    let minted = ask(&pin, &link_key, CLAIM, APP);
    assert_eq!(minted.0, 200, "{minted:?}");
    assert_eq!(minted.1["token"], TOKEN);
    assert_eq!(minted.1["repos"], json!(["app", "web"]));
    {
        let asked = asked.lock().expect("the record");
        assert_eq!(
            asked.tokens,
            [json!({
                "repositories": ["app", "web"],
                "permissions": { "contents": "write", "pull_requests": "read" },
            })]
        );
        let jwt = asked.bearers.last().expect("a bearer");
        let url = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let claims: Value = serde_json::from_slice(
            &url.decode(jwt.split('.').nth(1).expect("its claims"))
                .expect("base64"),
        )
        .expect("JSON");
        assert_eq!(claims["iss"], APP);
    }

    // **A real runner client** (#4207): the runner image's own purlis, in the pod, over the way a
    // runner has, not this test's forward. Its Claim says where to ask, and the key.
    let said = read
        .get(CLAIM)
        .await
        .expect("the Claim")
        .status
        .and_then(|status| status.mint)
        .expect("the Claim's status says where to ask");
    assert_eq!(said.address, format!("{address}:{}", mint::PORT));
    assert_eq!(said.operator_key, *OPERATOR.get().expect("set"));
    let paired = exec_fed(
        &runner_ns,
        &pod,
        &[
            "purlis",
            "bridge",
            "--pair",
            "--key-made-at-start",
            "/run/purlis-link/link-key",
        ],
        &format!(
            "{}\n",
            json!({ "device": "01E2EDESKTOP", "key": "22".repeat(32) })
        ),
    );
    assert!(
        paired.status.success(),
        "the runner did not pair: {}",
        String::from_utf8_lossy(&paired.stderr)
    );
    let told = |keys: &[&str]| {
        json!({
            "namespace": NAMESPACE, "claim": CLAIM, "cluster": line("cluster"),
            "address": said.address, "keys": keys,
        })
        .to_string()
    };
    let tell = |keys: &[&str]| {
        let out = exec_fed(
            &runner_ns,
            &pod,
            &["purlis", "bridge", "--mint-port"],
            &format!("{}\n", told(keys)),
        );
        assert!(
            out.status.success(),
            "the runner was not told: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    let check = || {
        exec(
            &runner_ns,
            &pod,
            "runner",
            &["purlis", "bridge", "--mint-check", APP],
        )
    };
    // Held to another key, it asks nothing of the port.
    let other = format!("ed25519:{}", "11".repeat(32));
    tell(&[&other]);
    let refused = check();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("not the operator"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    tell(&[&said.operator_key]);
    let checked = check();
    assert!(
        checked.status.success(),
        "the runner got no token: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let out = String::from_utf8_lossy(&checked.stdout);
    assert!(!out.contains(TOKEN), "a check prints no token");
    let answered: Value = serde_json::from_str(out.trim()).expect("what it says");
    assert_eq!(answered["owner"], "acme");
    assert_eq!(answered["repos"], json!(["app", "web"]));
    assert_eq!(answered["operator"], json!(said.operator_key));

    // A repo outside the allow-list alone; more than one owner; nothing at all. Patched by the
    // requester, as only they may.
    let patched = |repos: &'static [&'static str]| {
        let claims = claims.clone();
        async move {
            claims
                .patch(CLAIM, &PatchParams::default(), &repos_patch(repos))
                .await
                .expect("the requester patches")
        }
    };
    patched(&["acme/secrets"]).await;
    refused_saying(&ask(&pin, &link_key, CLAIM, APP), "allow-list");
    patched(&["acme/app", "other/app"]).await;
    refused_saying(&ask(&pin, &link_key, CLAIM, APP), "owners");
    patched(&[]).await;
    refused_saying(&ask(&pin, &link_key, CLAIM, APP), "no repos");
    // The real client hears the operator's own words.
    let refused = check();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("no repos"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    patched(&["acme/app"]).await;

    // A revoked `use` stops new tokens: the operator asks before each mint.
    revoke(&admin, "e2e-mint-use").await;
    let refused = eventually("the right to go", Duration::from_secs(30), || {
        let (pin, link_key) = (pin.clone(), link_key.clone());
        async move {
            let answered = ask(&pin, &link_key, CLAIM, APP);
            (answered.0 == 403).then_some(answered)
        }
    })
    .await;
    refused_saying(&refused, "may not use the pool");
    let tokens_before = asked.lock().expect("the record").tokens.len();
    grant(
        &admin,
        "e2e-mint-use",
        ADA,
        json!({
            "apiGroups": ["runners.purlis.dev"], "resources": ["pools"],
            "resourceNames": [POOL], "verbs": ["use"],
        }),
    )
    .await;

    // A Pool no longer opted in mints nothing, whatever the desktop's record says.
    Api::<Pool>::all(admin.clone())
        .patch(
            POOL,
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": { "forge": { "mintInCluster": false } } })),
        )
        .await
        .expect("the Pool");
    refused_saying(&ask(&pin, &link_key, CLAIM, APP), "opt in");
    Api::<Pool>::all(admin.clone())
        .patch(
            POOL,
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": { "forge": { "mintInCluster": true } } })),
        )
        .await
        .expect("the Pool");

    // On a Pool that mints, changing the image is spending the key too.
    revoke(&admin, "e2e-mint-key").await;
    let image_refused = eventually("the key's right to go", Duration::from_secs(30), || {
        let as_writer = as_writer.clone();
        async move {
            let patch = Patch::Merge(json!({ "spec": { "image":
                "example.test/other@sha256:1111111111111111111111111111111111111111111111111111111111111111" } }));
            match as_writer.patch(POOL, &PatchParams::default(), &patch).await {
                Err(kube::Error::Api(status)) if status.message.contains("forgekeys") => {
                    Some(status.message)
                }
                _ => None,
            }
        }
    })
    .await;
    assert!(image_refused.contains("image"), "{image_refused}");

    // The operator's mark opens the port, so no one else puts it on another namespace.
    let marked = namespaces
        .create(
            &PostParams::default(),
            &k8s_openapi::api::core::v1::Namespace {
                metadata: ObjectMeta {
                    name: Some("e2e-mint-marked".to_owned()),
                    labels: Some(BTreeMap::from([(
                        "app.kubernetes.io/managed-by".to_owned(),
                        "purlis-cluster-operator".to_owned(),
                    )])),
                    ..ObjectMeta::default()
                },
                ..Default::default()
            },
        )
        .await;
    assert!(marked.is_err(), "{marked:?}");

    // A Pool and Claim from before the policy on App keys was in force mint nothing: its binding
    // made again counts as the policy made again.
    let bindings = Api::<
        k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding,
    >::all(admin.clone());
    let binding = bindings.get("purlis-forge").await.expect("the binding");
    bindings
        .delete("purlis-forge", &DeleteParams::default())
        .await
        .expect("deleted");
    refused_saying(&ask(&pin, &link_key, CLAIM, APP), "not in force");
    let mut again = binding.clone();
    again.metadata = ObjectMeta {
        name: Some("purlis-forge".to_owned()),
        labels: binding.metadata.labels.clone(),
        annotations: binding.metadata.annotations.clone(),
        ..ObjectMeta::default()
    };
    bindings
        .create(&PostParams::default(), &again)
        .await
        .expect("made again");
    refused_saying(&ask(&pin, &link_key, CLAIM, APP), "made before");
    assert_eq!(
        asked.lock().expect("the record").tokens.len(),
        tokens_before,
        "a refusal reached GitHub"
    );

    // Every mint and refusal is in the log, with no token in it.
    let log = operator_log(&admin).await;
    assert!(log.contains("minted a token"), "{log}");
    assert!(log.contains("refused a token"), "{log}");
    assert!(log.contains(ADA), "{log}");
    assert!(!log.contains(TOKEN), "the token is in the log");
    let pem = app_key::pem();
    let key_line = pem.lines().nth(1).expect("a line");
    assert!(!log.contains(key_line), "the key is in the log");

    // Nothing about the token is written to the API server.
    let claim_now =
        serde_json::to_string(&read.get(CLAIM).await.expect("the claim")).expect("JSON");
    assert!(!claim_now.contains(TOKEN));
    let pool_now = serde_json::to_string(
        &Api::<Pool>::all(admin.clone())
            .get(POOL)
            .await
            .expect("the pool"),
    )
    .expect("JSON");
    assert!(!pool_now.contains(TOKEN));
    for namespace in [NAMESPACE, OPERATOR_NS, runner_ns.as_str()] {
        let events = Api::<k8s_openapi::api::core::v1::Event>::namespaced(admin.clone(), namespace)
            .list(&ListParams::default())
            .await
            .expect("the events");
        assert!(
            !serde_json::to_string(&events.items)
                .expect("JSON")
                .contains(TOKEN)
        );
        let secrets = Api::<Secret>::namespaced(admin.clone(), namespace)
            .list(&ListParams::default())
            .await
            .expect("the secrets");
        let names: Vec<String> = secrets.items.iter().map(ResourceExt::name_any).collect();
        assert!(
            names.iter().all(|name| !name.contains(CLAIM)),
            "a per-claim Secret: {names:?}"
        );
    }

    clean(&admin).await;
    revoke(&admin, "e2e-mint-claims").await;
}
