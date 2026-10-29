// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator vouches for a runner's link key (#2056): it signs the statement the desktop
//! reads, byte for byte as the shared worked example (`fixtures/runner-link-key/`), and only for
//! a key a pod of its own template published as its first step.

use base64::Engine;
use k8s_openapi::api::core::v1::{
    Container, ContainerState, ContainerStateTerminated, ContainerStatus, EnvVar, ExecAction,
    HostAlias, Lifecycle, LifecycleHandler, Pod, PodDNSConfig, PodResourceClaim,
    PodSecurityContext, PodSpec, PodStatus, PodTemplateSpec, Probe, ResourceRequirements,
    Toleration, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{
    FieldsV1, ManagedFieldsEntry, OwnerReference,
};
use kube::api::ObjectMeta;
use purlis_cluster_operator::upstream::{
    self, KEY_STEP, Reference, SandboxClaim, SandboxClaimStatus, template_for,
};
use purlis_cluster_operator::vouch::{
    self, About, Held, MountedKeys, NotYet, PEM_BEGIN, PEM_END, SigningKey, SigningKeys, Vouching,
};
use purlis_cluster_operator::{
    Claim, ClaimSpec, ClaimStatus, LinkKeyIs, NextSigningKeyVouch, Pool, PoolSpec, PoolStatus,
    pool_status_patch, status_patch,
};

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:2222222222222222222222222222222222222222222222222222222222222222";
const KEY: &str = "2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f70819";

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/runner-link-key")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
}

fn bytes_of_hex(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|at| u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).unwrap())
        .collect()
}

fn the_examples_key() -> Vec<u8> {
    bytes_of_hex(fixture("test-only-signing-key.pkcs8.hex").trim())
}

fn the_examples_next_key() -> Vec<u8> {
    bytes_of_hex(fixture("test-only-next-signing-key.pkcs8.hex").trim())
}

/// The cluster of the worked example: the uid of its `kube-system` namespace (#3305).
fn cluster() -> String {
    fixture("cluster.txt").trim().to_owned()
}

/// The same cluster, for `About`, which borrows.
const CLUSTER: &str = "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b";

fn the_example() -> About<'static> {
    About {
        cluster: CLUSTER,
        namespace: "ada",
        claim: "ada-1",
        claim_uid: "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60",
        runner_namespace: "purlis-runners-b5fc85e55755f9e0",
        pod: "ada-1",
        key: KEY,
        requester: "ada@example.com",
        until: 1798761600,
    }
}

/// Now, as the tests' operator signs: a statement it signs holds until ten minutes after.
const NOW: u64 = 1_798_000_000;

#[test]
fn the_statement_and_its_signature_are_the_worked_example_the_desktop_reads() {
    let signing = SigningKey::from_pkcs8(&the_examples_key()).unwrap();
    let text = vouch::statement(&the_example()).unwrap();
    assert_eq!(text, fixture("statement.txt"));
    assert_eq!(signing.public(), fixture("operator-key.txt").trim());
    assert_eq!(signing.sign(&text), fixture("signature.txt").trim());
    assert_eq!(CLUSTER, cluster());
    assert!(text.starts_with(&format!("purlis runner link key v3\ncluster: {CLUSTER}\n")));
}

#[test]
fn during_a_roll_the_next_key_signs_the_same_statement_as_the_worked_example_says() {
    let next = SigningKey::from_pkcs8(&the_examples_next_key()).unwrap();
    let text = vouch::statement(&the_example()).unwrap();
    assert_eq!(next.public(), fixture("next-operator-key.txt").trim());
    assert_eq!(next.sign(&text), fixture("next-signature.txt").trim());
}

#[test]
fn the_current_keys_word_for_the_next_is_the_worked_example_the_desktop_reads() {
    let signing = signing();
    let text = vouch::roll_statement(
        &cluster(),
        fixture("operator-key.txt").trim(),
        fixture("next-operator-key.txt").trim(),
        1_798_761_600,
    )
    .unwrap();
    assert_eq!(text, fixture("key-roll.txt"));
    assert_eq!(
        signing.sign(&text),
        fixture("key-roll-signature.txt").trim()
    );
    let from = fixture("operator-key.txt");
    let to = fixture("next-operator-key.txt");
    for (cluster, from, to) in [
        ("not-a-uid", from.trim(), to.trim()),
        (CLUSTER, "79b5", to.trim()),
        (CLUSTER, from.trim(), "ed25519:E7F1"),
    ] {
        assert_eq!(vouch::roll_statement(cluster, from, to, 1), None);
    }
}

#[test]
fn the_signing_key_is_read_from_the_pem_openssl_writes() {
    let pem = format!(
        "{PEM_BEGIN}\n{}\n{PEM_END}\n",
        base64::engine::general_purpose::STANDARD.encode(the_examples_key())
    );
    let signing = SigningKey::from_pem(&pem).unwrap();
    assert_eq!(signing.public(), fixture("operator-key.txt").trim());
    assert!(!format!("{signing:?}").contains("PRIVATE"));
    for bad in [
        String::new(),
        format!("{PEM_BEGIN}\nnot base64!\n{PEM_END}"),
        format!("{PEM_BEGIN}\nAAAA\n{PEM_END}"),
        format!(
            "{}\nAAAA\n{}",
            PEM_BEGIN.replace("BEGIN ", "BEGIN RSA "),
            PEM_END.replace("END ", "END RSA ")
        ),
    ] {
        assert!(SigningKey::from_pem(&bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_value_that_is_not_one_plain_line_makes_no_statement() {
    let lines = [
        About {
            requester: "ada\npod: other",
            ..the_example()
        },
        About {
            requester: "",
            ..the_example()
        },
        About {
            requester: " ada",
            ..the_example()
        },
        About {
            key: "2A3B",
            ..the_example()
        },
    ];
    for about in lines {
        assert_eq!(vouch::statement(&about), None, "{about:?}");
    }
}

fn pool() -> Pool {
    Pool::new(
        "team",
        PoolSpec {
            image: IMAGE.to_owned(),
            kept_volume: Default::default(),
            stopped: false,
        },
    )
}

/// A pod as upstream makes it from the operator's template for the pool, its first step ended
/// as `ended` says.
fn pod(ended: Option<(i32, &str)>) -> Pod {
    let template = template_for(&pool(), "ada", Vec::new());
    let spec = template.spec.pod_template.spec.unwrap();
    Pod {
        metadata: ObjectMeta {
            name: Some("ada-1".into()),
            ..ObjectMeta::default()
        },
        spec: Some(spec),
        status: ended.map(|(code, message)| PodStatus {
            init_container_statuses: Some(vec![ContainerStatus {
                name: KEY_STEP.to_owned(),
                state: Some(ContainerState {
                    terminated: Some(ContainerStateTerminated {
                        exit_code: code,
                        message: Some(message.to_owned()),
                        ..ContainerStateTerminated::default()
                    }),
                    ..ContainerState::default()
                }),
                ..ContainerStatus::default()
            }]),
            ..PodStatus::default()
        }),
    }
}

#[test]
fn the_pods_first_step_makes_the_key_before_anything_else_and_hands_it_to_the_runner() {
    let spec = template_for(&pool(), "ada", Vec::new())
        .spec
        .pod_template
        .spec
        .unwrap();
    let steps = spec.init_containers.unwrap();
    assert_eq!(steps.len(), 1);
    let step = &steps[0];
    assert_eq!(step.name, KEY_STEP);
    assert_eq!(step.image.as_deref(), Some(IMAGE));
    assert_eq!(
        step.args.clone().unwrap(),
        [
            "bridge",
            "--make-key",
            "/run/purlis-link/link-key",
            "--publish",
            "/dev/termination-log",
            "--home",
            "/var/lib/purlis/home"
        ]
    );
    assert_eq!(step.security_context, spec.containers[0].security_context);
    let folder = spec.volumes.unwrap();
    let folder = &folder[0];
    assert_eq!(
        folder.empty_dir.as_ref().unwrap().medium.as_deref(),
        Some("Memory"),
        "the private half is handed over in memory"
    );
    for container in [step, &spec.containers[0]] {
        let mounts = container.volume_mounts.clone().unwrap();
        assert_eq!(mounts.len(), 2);
        assert_eq!(
            (mounts[0].name.as_str(), mounts[0].mount_path.as_str()),
            (folder.name.as_str(), upstream::LINK_FOLDER)
        );
        assert_eq!(
            (mounts[1].name.as_str(), mounts[1].mount_path.as_str()),
            (upstream::KEPT_VOLUME, upstream::KEPT_FOLDER)
        );
    }
}

fn first_step(pod: &mut Pod) -> &mut Container {
    &mut pod.spec.as_mut().unwrap().init_containers.as_mut().unwrap()[0]
}

fn pod_template() -> PodTemplateSpec {
    template_for(&pool(), "ada", Vec::new()).spec.pod_template
}

fn template() -> PodSpec {
    template_of(&pool())
}

fn template_of(pool: &Pool) -> PodSpec {
    template_for(pool, "ada", Vec::new())
        .spec
        .pod_template
        .spec
        .unwrap()
}

#[test]
fn a_key_is_vouched_for_only_once_the_pods_own_first_step_published_it() {
    assert_eq!(
        vouch::key_published(&pod(Some((0, KEY))), &template()),
        Ok(KEY.to_owned())
    );
    assert_eq!(
        vouch::key_published(&pod(None), &template()),
        Err(NotYet::Starting)
    );
    for (ended, why) in [
        ((1, KEY), "a step that failed"),
        ((0, ""), "no key"),
        ((0, "2A3B"), "not a key"),
    ] {
        assert!(
            matches!(
                vouch::key_published(&pod(Some(ended)), &template()),
                Err(NotYet::Refused(_))
            ),
            "{why}"
        );
    }
}

/// One change to a pod.
type Change = (&'static str, Box<dyn Fn(&mut Pod)>);

fn runner(pod: &mut Pod) -> &mut Container {
    &mut pod.spec.as_mut().unwrap().containers[0]
}

fn spec(pod: &mut Pod) -> &mut PodSpec {
    pod.spec.as_mut().unwrap()
}

fn exec(command: &str) -> ExecAction {
    ExecAction {
        command: Some(vec!["/bin/sh".into(), "-c".into(), command.into()]),
    }
}

#[test]
fn a_pod_differing_from_the_template_in_anything_that_decides_what_runs_is_never_vouched_for() {
    let another_image = Pool::new(
        "team",
        PoolSpec {
            image: "ghcr.io/purlis/purlis-runner@sha256:3333333333333333333333333333333333333333333333333333333333333333".into(),
            kept_volume: Default::default(),
            stopped: false,
        },
    );
    assert!(matches!(
        vouch::key_published(&pod(Some((0, KEY))), &template_of(&another_image)),
        Err(NotYet::Refused(_))
    ));
    let changes: Vec<Change> = vec![
        (
            "a step before the key's",
            Box::new(|pod| {
                let mut step = first_step(pod).clone();
                step.name = "first".into();
                spec(pod).init_containers.as_mut().unwrap().insert(0, step);
            }),
        ),
        (
            "a command",
            Box::new(|pod| first_step(pod).command = Some(vec!["/bin/sh".into()])),
        ),
        (
            "arguments",
            Box::new(|pod| first_step(pod).args = Some(vec!["--version".into()])),
        ),
        (
            "an environment",
            Box::new(|pod| {
                first_step(pod).env = Some(vec![EnvVar {
                    name: "LD_PRELOAD".into(),
                    value: Some("/tmp/x.so".into()),
                    ..EnvVar::default()
                }]);
            }),
        ),
        (
            "a mount",
            Box::new(|pod| {
                first_step(pod)
                    .volume_mounts
                    .get_or_insert_default()
                    .push(VolumeMount {
                        name: "purlis-link".into(),
                        mount_path: "/usr/local/bin".into(),
                        ..VolumeMount::default()
                    });
            }),
        ),
        (
            "the runner's image",
            Box::new(|pod| runner(pod).image = Some("elsewhere@sha256:44".into())),
        ),
        (
            "the runner's confinement",
            Box::new(|pod| runner(pod).security_context = None),
        ),
        (
            "a lifecycle hook",
            Box::new(|pod| {
                runner(pod).lifecycle = Some(Lifecycle {
                    post_start: Some(LifecycleHandler {
                        exec: Some(exec("cat /run/purlis-link/link-key")),
                        ..LifecycleHandler::default()
                    }),
                    ..Lifecycle::default()
                });
            }),
        ),
        (
            "a probe",
            Box::new(|pod| {
                runner(pod).startup_probe = Some(Probe {
                    exec: Some(exec("true")),
                    ..Probe::default()
                });
            }),
        ),
        (
            "a step made a sidecar",
            Box::new(|pod| first_step(pod).restart_policy = Some("Always".into())),
        ),
        (
            "where the message is read from",
            Box::new(|pod| {
                first_step(pod).termination_message_policy = Some("FallbackToLogsOnError".into());
            }),
        ),
        (
            "a working folder",
            Box::new(|pod| runner(pod).working_dir = Some("/tmp".into())),
        ),
        ("a terminal", Box::new(|pod| runner(pod).stdin = Some(true))),
        (
            "host aliases",
            Box::new(|pod| {
                spec(pod).host_aliases = Some(vec![HostAlias {
                    hostnames: Some(vec!["github.com".into()]),
                    ip: "10.0.0.9".into(),
                }]);
            }),
        ),
        (
            "a DNS config",
            Box::new(|pod| {
                spec(pod).dns_config = Some(PodDNSConfig {
                    nameservers: Some(vec!["10.0.0.9".into()]),
                    ..PodDNSConfig::default()
                });
            }),
        ),
        (
            "a runtime class",
            Box::new(|pod| spec(pod).runtime_class_name = Some("runc".into())),
        ),
        (
            "a pod security context",
            Box::new(|pod| {
                spec(pod).security_context = Some(PodSecurityContext {
                    run_as_user: Some(0),
                    ..PodSecurityContext::default()
                });
            }),
        ),
        (
            "a node selector",
            Box::new(|pod| {
                spec(pod).node_selector = Some([("node".to_owned(), "theirs".to_owned())].into());
            }),
        ),
        (
            "host users",
            Box::new(|pod| spec(pod).host_users = Some(true)),
        ),
        (
            "resource claims",
            Box::new(|pod| {
                spec(pod).resource_claims = Some(vec![PodResourceClaim {
                    name: "gpu".into(),
                    ..PodResourceClaim::default()
                }]);
            }),
        ),
        (
            "a service account",
            Box::new(|pod| spec(pod).service_account_name = Some("admin".into())),
        ),
        (
            "a credential",
            Box::new(|pod| spec(pod).automount_service_account_token = None),
        ),
        (
            "the node's processes",
            Box::new(|pod| spec(pod).host_pid = Some(true)),
        ),
        (
            "shared processes",
            Box::new(|pod| spec(pod).share_process_namespace = Some(true)),
        ),
        (
            "another volume",
            Box::new(|pod| {
                spec(pod).volumes.get_or_insert_default().push(Volume {
                    name: "more".into(),
                    ..Volume::default()
                });
            }),
        ),
        (
            "an ephemeral container",
            Box::new(|pod| {
                spec(pod).ephemeral_containers = Some(vec![Default::default()]);
            }),
        ),
        (
            "a toleration of its own",
            Box::new(|pod| {
                spec(pod).tolerations = Some(vec![Toleration {
                    operator: Some("Exists".into()),
                    ..Toleration::default()
                }]);
            }),
        ),
    ];
    for (what, change) in &changes {
        let mut changed = pod(Some((0, KEY)));
        change(&mut changed);
        assert!(
            matches!(
                vouch::key_published(&changed, &template()),
                Err(NotYet::Refused(_))
            ),
            "{what}"
        );
    }
}

#[test]
fn what_the_cluster_sets_on_a_pod_does_not_stop_it_being_vouched_for() {
    let mut set = pod(Some((0, KEY)));
    {
        let spec = spec(&mut set);
        spec.node_name = Some("kind-worker".into());
        spec.scheduler_name = Some("default-scheduler".into());
        spec.dns_policy = Some("ClusterFirst".into());
        spec.restart_policy = Some("Always".into());
        spec.termination_grace_period_seconds = Some(30);
        spec.priority = Some(0);
        spec.preemption_policy = Some("PreemptLowerPriority".into());
        spec.service_account_name = Some("default".into());
        spec.service_account = Some("default".into());
        spec.tolerations = Some(
            [
                "node.kubernetes.io/not-ready",
                "node.kubernetes.io/unreachable",
            ]
            .map(|key| Toleration {
                key: Some(key.into()),
                operator: Some("Exists".into()),
                effect: Some("NoExecute".into()),
                toleration_seconds: Some(300),
                ..Toleration::default()
            })
            .to_vec(),
        );
        for container in spec
            .containers
            .iter_mut()
            .chain(spec.init_containers.iter_mut().flatten())
        {
            container.image_pull_policy = Some("IfNotPresent".into());
            container.termination_message_path = Some("/dev/termination-log".into());
            container.termination_message_policy = Some("File".into());
            container.resources = Some(Default::default());
        }
    }
    assert_eq!(vouch::key_published(&set, &template()), Ok(KEY.to_owned()));
    assert!(!vouch::SET_BY_THE_CLUSTER.is_empty());
    // Upstream's own name servers, where it manages the pod's network; no others.
    let upstreams = |pod: &mut Pod, servers: Vec<String>| {
        spec(pod).dns_policy = Some("None".into());
        spec(pod).dns_config = Some(PodDNSConfig {
            nameservers: Some(servers),
            ..PodDNSConfig::default()
        });
    };
    let managed = upstream::MANAGED_NAME_SERVERS.map(str::to_owned).to_vec();
    upstreams(&mut set, managed.clone());
    assert_eq!(vouch::key_published(&set, &template()), Ok(KEY.to_owned()));
    for servers in [
        vec!["10.0.0.9".to_owned()],
        [managed, vec!["10.0.0.9".into()]].concat(),
    ] {
        upstreams(&mut set, servers);
        assert!(matches!(
            vouch::key_published(&set, &template()),
            Err(NotYet::Refused(_))
        ));
    }
}

fn owned_by(kind: &str, api_version: &str, name: &str, uid: &str) -> Vec<OwnerReference> {
    vec![OwnerReference {
        api_version: api_version.into(),
        kind: kind.into(),
        name: name.into(),
        uid: uid.into(),
        controller: Some(true),
        block_owner_deletion: None,
    }]
}

const UPSTREAM_UID: &str = "11111111-1111-4111-8111-111111111111";
const SANDBOX_UID: &str = "22222222-2222-4222-8222-222222222222";
const POD_UID: &str = "33333333-3333-4333-8333-333333333333";

/// Ada's runner namespace, as the operator made it (#2055).
fn runner_namespace() -> k8s_openapi::api::core::v1::Namespace {
    let mut runner = purlis_cluster_operator::person::namespace_for("ada@example.com");
    runner.metadata.uid = Some("0d6f2c4a-1b3e-4f5a-8c7d-9e0f1a2b3c4d".into());
    runner
}

#[test]
fn the_examples_runner_namespace_is_the_one_worked_out_for_its_requester() {
    assert_eq!(
        purlis_cluster_operator::person::runner_namespace(the_example().requester),
        the_example().runner_namespace
    );
}

fn claim() -> Claim {
    let mut claim = Claim::new(
        "ada-1",
        ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
            kept: false,
            stopped: false,
        },
    );
    claim.metadata.namespace = Some("ada".into());
    claim.metadata.uid = Some("6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60".into());
    claim
}

fn upstream_claim(names: &str) -> SandboxClaim {
    let mut held = upstream::sandbox_claim_for(&claim(), &Default::default(), &runner_namespace());
    held.metadata.uid = Some(UPSTREAM_UID.into());
    held.status = Some(SandboxClaimStatus {
        conditions: Vec::new(),
        sandbox: Some(Reference { name: names.into() }),
    });
    held
}

fn sandbox(owner_uid: &str) -> ObjectMeta {
    ObjectMeta {
        name: Some("ada-1".into()),
        uid: Some(SANDBOX_UID.into()),
        owner_references: Some(owned_by(
            "SandboxClaim",
            "extensions.agents.x-k8s.io/v1beta1",
            "ada-1",
            owner_uid,
        )),
        ..ObjectMeta::default()
    }
}

fn runner_pod(owner: Vec<OwnerReference>) -> Pod {
    let mut pod = pod(Some((0, KEY)));
    pod.metadata.uid = Some(POD_UID.into());
    pod.metadata.namespace = runner_namespace().metadata.name;
    pod.metadata.owner_references = Some(owner);
    pod
}

fn the_sandboxes() -> Vec<OwnerReference> {
    owned_by("Sandbox", "agents.x-k8s.io/v1beta1", "ada-1", SANDBOX_UID)
}

fn signing() -> SigningKey {
    SigningKey::from_pkcs8(&the_examples_key()).unwrap()
}

fn next_signing() -> SigningKey {
    SigningKey::from_pkcs8(&the_examples_next_key()).unwrap()
}

/// The worked example's key alone, outside a roll.
fn keys() -> SigningKeys {
    SigningKeys {
        current: signing(),
        next: None,
    }
}

/// The worked example's key, rolling to the next one.
fn rolling() -> SigningKeys {
    SigningKeys {
        current: signing(),
        next: Some(next_signing()),
    }
}

fn vouching(
    upstream: &SandboxClaim,
    sandbox: Option<&ObjectMeta>,
    pod: Option<&Pod>,
    held_template: Option<&ObjectMeta>,
) -> Vouching {
    let claim = claim();
    vouch::vouch(
        &Held {
            now: NOW,
            cluster: CLUSTER,
            claim: &claim,
            upstream,
            template: held_template,
            template_spec: None,
            sandbox,
            pod,
        },
        &pod_template(),
        &keys(),
    )
}

#[test]
fn a_runner_outside_its_requesters_runner_namespace_is_never_vouched_for() {
    let pod = runner_pod(the_sandboxes());
    let sandbox = sandbox(UPSTREAM_UID);
    for elsewhere in ["ada", "purlis-runners-0000000000000000"] {
        let mut upstream = upstream_claim("ada-1");
        upstream.metadata.namespace = Some(elsewhere.into());
        assert!(
            matches!(
                vouching(&upstream, Some(&sandbox), Some(&pod), None),
                Vouching::Refused(why) if why.contains("runner namespace")
            ),
            "{elsewhere}"
        );
    }
}

#[test]
fn a_claim_is_followed_to_its_pod_by_uid_and_its_key_vouched_for() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let pod = runner_pod(the_sandboxes());
    let Vouching::Vouched(link_key) = vouching(&upstream, Some(&sandbox), Some(&pod), None) else {
        panic!("not vouched for");
    };
    assert!(link_key.statement.contains("pod: ada-1\n"));
    assert!(!link_key.statement.contains("pod uid"));
    // The Claim's namespace, and the runner namespace its pod is in (#2055).
    assert!(link_key.statement.contains("namespace: ada\n"));
    assert!(
        link_key
            .statement
            .contains("runner namespace: purlis-runners-b5fc85e55755f9e0\n")
    );
    assert!(link_key.statement.contains(&format!("key: {KEY}\n")));
    assert_eq!(link_key.signature, signing().sign(&link_key.statement));
    assert!(matches!(
        vouching(&upstream, None, None, None),
        Vouching::Starting(_)
    ));
    assert!(matches!(
        vouching(&upstream, Some(&sandbox), None, None),
        Vouching::Starting(_)
    ));
}

#[test]
fn a_pod_made_under_the_sandboxs_name_by_someone_else_is_never_vouched_for() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let squatted = [
        ("no owner", Vec::new()),
        (
            "the sandbox's name, another uid",
            owned_by("Sandbox", "agents.x-k8s.io/v1beta1", "ada-1", POD_UID),
        ),
        (
            "the sandbox's uid, another group",
            owned_by("Sandbox", "evil.example/v1", "ada-1", SANDBOX_UID),
        ),
        (
            "the sandbox's uid, not its controller",
            vec![OwnerReference {
                controller: None,
                ..the_sandboxes()[0].clone()
            }],
        ),
    ];
    for (what, owner) in squatted {
        let pod = runner_pod(owner);
        assert!(
            matches!(
                vouching(&upstream, Some(&sandbox), Some(&pod), None),
                Vouching::Refused(_)
            ),
            "{what}"
        );
    }
}

#[test]
fn an_upstream_claim_naming_another_claims_sandbox_is_never_vouched_for() {
    // Its status names a sandbox made for another upstream claim.
    let upstream = upstream_claim("ada-1");
    let anothers = sandbox("44444444-4444-4444-8444-444444444444");
    let pod = runner_pod(the_sandboxes());
    assert!(matches!(
        vouching(&upstream, Some(&anothers), Some(&pod), None),
        Vouching::Refused(_)
    ));
    // Or one of another name than the one it names.
    let elsewhere = upstream_claim("bea-1");
    assert!(!matches!(
        vouching(&elsewhere, Some(&sandbox(UPSTREAM_UID)), Some(&pod), None),
        Vouching::Vouched(_)
    ));
}

#[test]
fn a_template_another_manager_changed_is_never_vouched_for() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let pod = runner_pod(the_sandboxes());
    let managed = |manager: &str, fields: serde_json::Value| ObjectMeta {
        managed_fields: Some(vec![ManagedFieldsEntry {
            manager: Some(manager.into()),
            fields_v1: Some(FieldsV1(fields)),
            ..ManagedFieldsEntry::default()
        }]),
        ..ObjectMeta::default()
    };
    let ours = managed(
        purlis_cluster_operator::FIELD_MANAGER,
        serde_json::json!({ "f:spec": {} }),
    );
    assert!(matches!(
        vouching(&upstream, Some(&sandbox), Some(&pod), Some(&ours)),
        Vouching::Vouched(_)
    ));
    let only_metadata = managed(
        "kube-controller-manager",
        serde_json::json!({ "f:metadata": {} }),
    );
    assert!(matches!(
        vouching(&upstream, Some(&sandbox), Some(&pod), Some(&only_metadata)),
        Vouching::Vouched(_)
    ));
    let theirs = managed("kubectl-edit", serde_json::json!({ "f:spec": {} }));
    assert!(matches!(
        vouching(&upstream, Some(&sandbox), Some(&pod), Some(&theirs)),
        Vouching::Refused(_)
    ));
}

#[test]
fn a_statement_once_written_goes_when_it_no_longer_holds() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let pod = runner_pod(the_sandboxes());
    let vouched = vouching(&upstream, Some(&sandbox), Some(&pod), None);
    let mut written = ClaimStatus {
        conditions: vec![vouched.condition()],
        link_key: vouched.link_key(),
    };
    assert!(written.link_key.is_some());
    // Looked at again and refused: the statement is taken away.
    let refused = Vouching::Refused("its pod changed".into());
    let next = ClaimStatus {
        conditions: vec![refused.condition()],
        link_key: refused.link_key(),
    };
    let patch = status_patch(Some(&written), next.clone(), LinkKeyIs::Said).expect("a change");
    assert_eq!(patch["status"]["linkKey"], serde_json::Value::Null);
    // Its pod replaced by one that made another key: that key's statement is written over
    // the old one.
    let mut replaced = runner_pod(the_sandboxes());
    replaced.metadata.uid = Some("55555555-5555-4555-8555-555555555555".into());
    let another = "9".repeat(64);
    replaced
        .status
        .as_mut()
        .unwrap()
        .init_container_statuses
        .as_mut()
        .unwrap()[0]
        .state
        .as_mut()
        .unwrap()
        .terminated
        .as_mut()
        .unwrap()
        .message = Some(another.clone());
    let again = vouching(&upstream, Some(&sandbox), Some(&replaced), None);
    let patch = status_patch(
        Some(&written),
        ClaimStatus {
            conditions: vec![again.condition()],
            link_key: again.link_key(),
        },
        LinkKeyIs::Said,
    )
    .expect("a change");
    assert!(
        patch["status"]["linkKey"]["statement"]
            .as_str()
            .unwrap()
            .contains(&format!("key: {another}\n"))
    );
    // Its pod made again with the key it keeps: the statement says the same, and nothing is
    // written (#2057).
    let mut remade = runner_pod(the_sandboxes());
    remade.metadata.uid = Some("55555555-5555-4555-8555-555555555555".into());
    let same = vouching(&upstream, Some(&sandbox), Some(&remade), None);
    assert_eq!(same.link_key(), written.link_key);
    // A pass that did not look at the key leaves it.
    written.conditions.clear();
    let kept = status_patch(Some(&written), ClaimStatus::default(), LinkKeyIs::Kept);
    assert!(
        kept.is_none_or(|patch| patch["status"].get("linkKey") != Some(&serde_json::Value::Null))
    );
    // A claim vouched for is looked at again now and then; one starting, soon.
    assert_eq!(vouched.again(), vouch::RECHECK);
    assert_eq!(Vouching::Starting(String::new()).again(), vouch::KEY_WAIT);
}

#[test]
fn a_name_or_uid_that_is_not_kubernetes_own_makes_no_statement() {
    for about in [
        About {
            pod: "--server=https://elsewhere",
            ..the_example()
        },
        About {
            pod: "Ada-1",
            ..the_example()
        },
        About {
            namespace: "a.b",
            ..the_example()
        },
        About {
            runner_namespace: "a.b",
            ..the_example()
        },
        About {
            runner_namespace: "--namespace=kube-system",
            ..the_example()
        },
        About {
            claim_uid: "not-a-uid",
            ..the_example()
        },
        About {
            cluster: "kube-system",
            ..the_example()
        },
    ] {
        assert_eq!(vouch::statement(&about), None, "{about:?}");
    }
}

#[test]
fn the_link_key_path_is_the_one_both_sides_read() {
    assert_eq!(
        format!("{}/link-key", upstream::LINK_FOLDER),
        fixture("link-key-path.txt").trim()
    );
    assert!(upstream::key_step_args().contains(&fixture("link-key-path.txt").trim().to_owned()));
}

/// Now, as the roll tests' operator signs: well before the worked example's `until`.
const ROLL_NOW: u64 = 1_798_200_000;

const DAY: u64 = 24 * 3600;

#[test]
fn a_pools_status_names_the_operators_key_and_cluster_and_is_written_until_it_does() {
    let status = vouch::pool_status(&keys(), CLUSTER, None, ROLL_NOW);
    assert_eq!(
        status,
        PoolStatus {
            signing_key: Some(fixture("operator-key.txt").trim().to_owned()),
            cluster: Some(CLUSTER.to_owned()),
            next_signing_key: None,
            next_signing_key_vouch: None,
        }
    );
    // Written while the pool says anything else, with a next key no longer held taken away.
    let patch = pool_status_patch(None, &status).expect("written");
    assert_eq!(
        patch["status"]["signingKey"],
        fixture("operator-key.txt").trim()
    );
    assert_eq!(patch["status"]["cluster"], CLUSTER);
    assert_eq!(patch["status"]["nextSigningKey"], serde_json::Value::Null);
    assert_eq!(
        patch["status"]["nextSigningKeyVouch"],
        serde_json::Value::Null
    );
    let older = PoolStatus {
        signing_key: Some("ed25519:00".into()),
        ..PoolStatus::default()
    };
    assert!(pool_status_patch(Some(&older), &status).is_some());
    assert_eq!(pool_status_patch(Some(&status), &status), None);
}

#[test]
fn during_a_roll_a_pools_status_names_the_next_key_and_the_current_keys_word_for_it() {
    let status = vouch::pool_status(&rolling(), CLUSTER, None, ROLL_NOW);
    assert_eq!(
        status.next_signing_key.as_deref(),
        Some(fixture("next-operator-key.txt").trim())
    );
    let word = status
        .next_signing_key_vouch
        .clone()
        .expect("the current key's word");
    assert_eq!(
        word.statement,
        vouch::roll_statement(
            CLUSTER,
            fixture("operator-key.txt").trim(),
            fixture("next-operator-key.txt").trim(),
            ROLL_NOW + 7 * DAY,
        )
        .unwrap()
    );
    assert_eq!(word.signature, signing().sign(&word.statement));
    // Looked at again before its last day: the same word, nothing written.
    let later = vouch::pool_status(&rolling(), CLUSTER, Some(&status), ROLL_NOW + 5 * DAY);
    assert_eq!(later, status);
    assert_eq!(pool_status_patch(Some(&status), &later), None);
    // In it: signed again, holding seven days from then.
    let renewed = vouch::pool_status(&rolling(), CLUSTER, Some(&status), ROLL_NOW + 6 * DAY + 1);
    assert_eq!(
        vouch::until_of(&renewed.next_signing_key_vouch.unwrap().statement),
        Some(ROLL_NOW + 13 * DAY + 1)
    );
    // A word on the status that is not the current key's own is never kept.
    let forged = PoolStatus {
        next_signing_key_vouch: Some(NextSigningKeyVouch {
            signature: "0".repeat(128),
            ..word.clone()
        }),
        ..status.clone()
    };
    let rewritten = vouch::pool_status(&rolling(), CLUSTER, Some(&forged), ROLL_NOW);
    assert_eq!(rewritten, status);
    // The roll over: the next key and the word go.
    let promoted = SigningKeys {
        current: next_signing(),
        next: None,
    };
    let after = vouch::pool_status(&promoted, CLUSTER, Some(&status), ROLL_NOW + 20);
    assert_eq!(after.signing_key, status.next_signing_key);
    assert_eq!(after.next_signing_key, None);
    let patch = pool_status_patch(Some(&status), &after).expect("written");
    assert_eq!(patch["status"]["nextSigningKey"], serde_json::Value::Null);
    assert_eq!(
        patch["status"]["nextSigningKeyVouch"],
        serde_json::Value::Null
    );
}

#[test]
fn during_a_roll_a_statement_is_signed_by_both_keys() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let pod = runner_pod(the_sandboxes());
    let at = |keys: &SigningKeys, claim: &Claim| {
        let Vouching::Vouched(link_key) = vouch::vouch(
            &Held {
                now: NOW,
                cluster: CLUSTER,
                claim,
                upstream: &upstream,
                template: None,
                template_spec: None,
                sandbox: Some(&sandbox),
                pod: Some(&pod),
            },
            &pod_template(),
            keys,
        ) else {
            panic!("not vouched for");
        };
        link_key
    };
    let mut claim = claim();
    let before = at(&keys(), &claim);
    assert_eq!(before.next_signature, None);
    assert!(before.statement.contains(&format!("cluster: {CLUSTER}\n")));
    claim.status = Some(ClaimStatus {
        conditions: Vec::new(),
        link_key: Some(before.clone()),
    });
    // A roll starting: the statement on the claim is signed again at once, by both keys.
    let during = at(&rolling(), &claim);
    assert_eq!(during.statement, before.statement);
    assert_eq!(during.signature, signing().sign(&during.statement));
    assert_eq!(
        during.next_signature,
        Some(next_signing().sign(&during.statement))
    );
    // And kept while it holds.
    claim.status.as_mut().unwrap().link_key = Some(during.clone());
    assert_eq!(at(&rolling(), &claim), during);
    // The roll over: signed by the new key alone.
    let promoted = SigningKeys {
        current: next_signing(),
        next: None,
    };
    let after = at(&promoted, &claim);
    assert_eq!(after.signature, next_signing().sign(&after.statement));
    assert_eq!(after.next_signature, None);
    // Written over the claim's statement, its second signature goes: a merge patch keeps a
    // field it does not name.
    let patch = status_patch(
        claim.status.as_ref(),
        ClaimStatus {
            conditions: Vec::new(),
            link_key: Some(after.clone()),
        },
        LinkKeyIs::Said,
    )
    .expect("a change");
    assert_eq!(
        patch["status"]["linkKey"]["nextSignature"],
        serde_json::Value::Null
    );
    assert_eq!(
        patch["status"]["linkKey"]["signature"],
        after.signature.as_str()
    );
}

fn pem_of(der: &[u8]) -> String {
    format!(
        "{PEM_BEGIN}\n{}\n{PEM_END}\n",
        base64::engine::general_purpose::STANDARD.encode(der)
    )
}

/// A folder of the test's own, holding the files `files` names.
fn mounted(name: &str, files: &[(&str, String)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "purlis-operator-keys-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir
}

#[test]
fn the_mounted_keys_are_read_again_as_a_team_admin_rolls_them() {
    let (key, next) = (
        pem_of(&the_examples_key()),
        pem_of(&the_examples_next_key()),
    );
    let dir = mounted("roll", &[("key.pem", key)]);
    let keys = MountedKeys::read(&dir).expect("the key");
    assert_eq!(
        keys.now().current.public(),
        fixture("operator-key.txt").trim()
    );
    assert!(keys.now().next.is_none());
    // next.pem added beside it: a roll, with no restart.
    std::fs::write(dir.join("next.pem"), &next).unwrap();
    assert_eq!(
        keys.now().next.as_ref().map(SigningKey::public).as_deref(),
        Some(fixture("next-operator-key.txt").trim())
    );
    // The roll over: next.pem moved to key.pem.
    std::fs::write(dir.join("key.pem"), &next).unwrap();
    std::fs::remove_file(dir.join("next.pem")).unwrap();
    let now = keys.now();
    assert_eq!(
        now.current.public(),
        fixture("next-operator-key.txt").trim()
    );
    assert!(now.next.is_none());
    // Keys that cannot be read, as while the kubelet swaps them, leave the last ones in use.
    std::fs::remove_file(dir.join("key.pem")).unwrap();
    assert_eq!(
        keys.now().current.public(),
        fixture("next-operator-key.txt").trim()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_operator_starts_only_with_a_key_and_a_next_key_it_reads() {
    let key = pem_of(&the_examples_key());
    assert!(MountedKeys::read(mounted("none", &[])).is_err());
    let bad_next = mounted(
        "bad-next",
        &[("key.pem", key.clone()), ("next.pem", "no key".into())],
    );
    assert!(MountedKeys::read(bad_next).is_err());
    // A next key that is the current one is no roll.
    let same = mounted("same", &[("key.pem", key.clone()), ("next.pem", key)]);
    assert!(MountedKeys::read(same).unwrap().now().next.is_none());
}

fn labelled(labels: &[(&str, &str)], annotations: &[(&str, &str)]) -> Pod {
    let mut pod = runner_pod(the_sandboxes());
    let map = |pairs: &[(&str, &str)]| {
        Some(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        )
    };
    pod.metadata.labels = map(labels);
    pod.metadata.annotations = map(annotations);
    pod
}

#[test]
fn a_pod_is_held_to_the_labels_and_annotations_upstream_gives_it() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let upstreams = [
        (vouch::CLAIM_UID_LABEL, UPSTREAM_UID),
        ("agents.x-k8s.io/sandbox-name-hash", "900bf6e9"),
        ("agents.x-k8s.io/sandbox-template-ref-hash", "85e22c3a"),
    ];
    let vouched = |pod: &Pod| vouching(&upstream, Some(&sandbox), Some(pod), None);
    assert!(matches!(
        vouched(&labelled(&upstreams, &[])),
        Vouching::Vouched(_)
    ));
    assert!(matches!(
        vouched(&labelled(&upstreams, &[("agents.x-k8s.io/note", "x")])),
        Vouching::Vouched(_)
    ));
    let refused = [
        labelled(&[("app", "anything")], &[]),
        labelled(&[(vouch::CLAIM_UID_LABEL, SANDBOX_UID)], &[]),
        labelled(&upstreams, &[("k8s.v1.cni.cncf.io/networks", "host")]),
        labelled(&upstreams, &[("io.kubernetes.cri-o.Devices", "/dev/kvm")]),
    ];
    for pod in &refused {
        assert!(
            matches!(vouched(pod), Vouching::Refused(_)),
            "{:?} {:?}",
            pod.metadata.labels,
            pod.metadata.annotations
        );
    }
}

#[test]
fn a_pools_template_is_held_to_the_operators_own_whatever_manager_changed_it() {
    let wanted = pod_template();
    assert!(vouch::template_is_ours(&wanted, &wanted));
    let mut empty_metadata = wanted.clone();
    empty_metadata.metadata = Some(ObjectMeta::default());
    assert!(vouch::template_is_ours(&empty_metadata, &wanted));
    let mut labelled = wanted.clone();
    labelled.metadata = Some(ObjectMeta {
        labels: Some([("app".to_owned(), "open".to_owned())].into()),
        ..ObjectMeta::default()
    });
    assert!(!vouch::template_is_ours(&labelled, &wanted));
    let mut hooked = wanted.clone();
    hooked.spec.as_mut().unwrap().containers[0].working_dir = Some("/tmp".into());
    assert!(!vouch::template_is_ours(&hooked, &wanted));
    // And a pass that reads the template so changed vouches for nothing.
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let pod = runner_pod(the_sandboxes());
    let claim = claim();
    let vouching = vouch::vouch(
        &Held {
            now: NOW,
            cluster: CLUSTER,
            claim: &claim,
            upstream: &upstream,
            template: None,
            template_spec: Some(&labelled),
            sandbox: Some(&sandbox),
            pod: Some(&pod),
        },
        &wanted,
        &keys(),
    );
    assert!(matches!(vouching, Vouching::Refused(_)));
}

#[test]
fn a_container_may_ask_only_for_what_the_node_itself_gives() {
    let asking = |requests: &[&str]| {
        let mut asked = pod(Some((0, KEY)));
        runner(&mut asked).resources = Some(ResourceRequirements {
            requests: Some(
                requests
                    .iter()
                    .map(|name| ((*name).to_owned(), Quantity("1".into())))
                    .collect(),
            ),
            ..ResourceRequirements::default()
        });
        vouch::key_published(&asked, &template())
    };
    assert_eq!(
        asking(&["cpu", "memory", "ephemeral-storage", "hugepages-2Mi"]),
        Ok(KEY.to_owned())
    );
    for extended in [&["nvidia.com/gpu"][..], &["cpu", "example.com/device"][..]] {
        assert!(
            matches!(asking(extended), Err(NotYet::Refused(_))),
            "{extended:?}"
        );
    }
    let mut claiming = pod(Some((0, KEY)));
    runner(&mut claiming).resources = Some(ResourceRequirements {
        claims: Some(vec![Default::default()]),
        ..ResourceRequirements::default()
    });
    assert!(matches!(
        vouch::key_published(&claiming, &template()),
        Err(NotYet::Refused(_))
    ));
}

fn kept_claim() -> Claim {
    let mut claim = claim();
    claim.spec.kept = true;
    claim
}

/// What the operator says of a kept claim's runner in `pod`, its sandbox named `sandbox_name`.
fn vouching_kept(sandbox_name: &str, pod: &Pod) -> Vouching {
    vouching_kept_labelled(sandbox_name, pod, true)
}

/// [`vouching_kept`], with the pod named for its claim by upstream's label only if `labelled`.
fn vouching_kept_labelled(sandbox_name: &str, pod: &Pod, labelled: bool) -> Vouching {
    let claim = kept_claim();
    let mut upstream =
        upstream::sandbox_claim_for(&claim, &Default::default(), &runner_namespace());
    upstream.metadata.uid = Some(UPSTREAM_UID.into());
    upstream.status = Some(SandboxClaimStatus {
        conditions: Vec::new(),
        sandbox: Some(Reference {
            name: sandbox_name.into(),
        }),
    });
    let mut sandbox = sandbox(UPSTREAM_UID);
    sandbox.name = Some(sandbox_name.into());
    let mut pod = pod.clone();
    if labelled {
        pod.metadata
            .labels
            .get_or_insert_with(Default::default)
            .entry(vouch::CLAIM_UID_LABEL.to_owned())
            .or_insert_with(|| UPSTREAM_UID.to_owned());
    }
    pod.metadata.owner_references = Some(owned_by(
        "Sandbox",
        "agents.x-k8s.io/v1beta1",
        sandbox_name,
        SANDBOX_UID,
    ));
    vouch::vouch(
        &Held {
            now: NOW,
            cluster: CLUSTER,
            claim: &claim,
            upstream: &upstream,
            template: None,
            template_spec: None,
            sandbox: Some(&sandbox),
            pod: Some(&pod),
        },
        &pod_template(),
        &keys(),
    )
}

/// A runner's pod whose home is on the volume claim `claim_name`, as upstream makes a kept
/// claim's: the template's volume of that name taken out, the claim's last.
fn on_volume(claim_name: &str) -> Pod {
    let mut pod = runner_pod(the_sandboxes());
    let volumes = spec(&mut pod).volumes.as_mut().unwrap();
    volumes.retain(|volume| volume.name != upstream::KEPT_VOLUME);
    volumes.push(Volume {
        name: upstream::KEPT_VOLUME.into(),
        persistent_volume_claim: Some(
            k8s_openapi::api::core::v1::PersistentVolumeClaimVolumeSource {
                claim_name: claim_name.into(),
                ..Default::default()
            },
        ),
        ..Volume::default()
    });
    pod
}

#[test]
fn a_kept_claims_pod_is_vouched_for_with_its_own_volume_in_place_of_the_templates() {
    assert!(matches!(
        vouching_kept("ada-1", &on_volume("purlis-kept-ada-1")),
        Vouching::Vouched(_)
    ));
    let refused = [
        (
            "the template's own folder",
            "ada-1",
            runner_pod(the_sandboxes()),
        ),
        (
            "another claim's volume",
            "ada-1",
            on_volume("purlis-kept-ada-2"),
        ),
        ("a volume of anyone's", "ada-1", on_volume("data")),
        (
            "a sandbox not named as the claim",
            "purlis-pool-team-x7k2q",
            on_volume("purlis-kept-purlis-pool-team-x7k2q"),
        ),
    ];
    for (what, sandbox, pod) in &refused {
        assert!(
            matches!(vouching_kept(sandbox, pod), Vouching::Refused(_)),
            "{what}"
        );
    }
    // A kept claim's pod names its claim, and no other.
    let mut another = on_volume("purlis-kept-ada-1");
    another.metadata.labels =
        Some([(vouch::CLAIM_UID_LABEL.to_owned(), SANDBOX_UID.to_owned())].into());
    assert!(matches!(
        vouching_kept("ada-1", &another),
        Vouching::Refused(_)
    ));
    assert!(matches!(
        vouching_kept_labelled("ada-1", &on_volume("purlis-kept-ada-1"), false),
        Vouching::Refused(_)
    ));
    // A fresh claim's pod keeps the template's folder, and no volume of its own.
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let on_a_volume = {
        let mut pod = on_volume("purlis-kept-ada-1");
        pod.metadata.owner_references = Some(the_sandboxes());
        pod
    };
    assert!(matches!(
        vouching(&upstream, Some(&sandbox), Some(&on_a_volume), None),
        Vouching::Refused(_)
    ));
}

#[test]
fn a_kept_claim_asks_upstream_for_its_own_volume_and_a_fresh_one_for_none() {
    let volume = purlis_cluster_operator::KeptVolume {
        size: "20Gi".into(),
        storage_class_name: Some("fast".into()),
    };
    let kept = serde_json::to_value(upstream::sandbox_claim_for(
        &kept_claim(),
        &volume,
        &runner_namespace(),
    ))
    .unwrap();
    assert_eq!(
        kept["spec"]["volumeClaimTemplates"],
        serde_json::json!([{
            "metadata": { "name": "purlis-kept" },
            "spec": {
                "accessModes": ["ReadWriteOnce"],
                "resources": { "requests": { "storage": "20Gi" } },
                "storageClassName": "fast",
            },
        }])
    );
    let fresh = serde_json::to_value(upstream::sandbox_claim_for(
        &claim(),
        &volume,
        &runner_namespace(),
    ))
    .unwrap();
    assert!(
        fresh["spec"].get("volumeClaimTemplates").is_none(),
        "{fresh}"
    );
    assert_eq!(
        upstream::kept_volume_name(&kept_claim()),
        "purlis-kept-ada-1"
    );
    // The template lets a claim add a volume, and holds none a claim could replace.
    let template = serde_json::to_value(template_for(&pool(), "ada", Vec::new())).unwrap();
    assert_eq!(template["spec"]["volumeClaimTemplatesPolicy"], "Allowed");
    assert!(template["spec"].get("volumeClaimTemplates").is_none());
}

#[test]
fn a_warm_runner_a_fresh_claim_took_keeps_the_labels_its_warm_pool_gave_it() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let warm = [
        (vouch::CLAIM_UID_LABEL, UPSTREAM_UID),
        ("agents.x-k8s.io/sandbox-name-hash", "900bf6e9"),
        ("agents.x-k8s.io/sandbox-template-ref-hash", "85e22c3a"),
        // The warm pool `purlis-pool-team`'s name, as upstream hashes it.
        ("agents.x-k8s.io/warm-pool-sandbox", "7c9f77a9"),
        ("agents.x-k8s.io/sandbox-pod-template-hash", "6c9d8e7f"),
        ("agents.x-k8s.io/sandbox-template-hash", "1a2b3c4d"),
    ];
    assert!(matches!(
        vouching(&upstream, Some(&sandbox), Some(&labelled(&warm, &[])), None),
        Vouching::Vouched(_)
    ));
    let mut another = warm;
    another[3].1 = "4be1c2d3";
    assert!(matches!(
        vouching(
            &upstream,
            Some(&sandbox),
            Some(&labelled(&another, &[])),
            None
        ),
        Vouching::Refused(_)
    ));
    // FNV-1a, 32 bits, as Go's hash/fnv gives it.
    assert_eq!(vouch::name_hash(""), "811c9dc5");
    assert_eq!(vouch::name_hash("a"), "e40c292c");
    assert_eq!(vouch::name_hash("purlis-pool-team"), "7c9f77a9");
}

#[test]
fn the_pods_first_step_is_told_the_claim_its_home_is_made_for() {
    let spec = template_for(&pool(), "ada", Vec::new())
        .spec
        .pod_template
        .spec
        .unwrap();
    let step = &spec.init_containers.unwrap()[0];
    let env = serde_json::to_value(step.env.as_ref().unwrap()).unwrap();
    assert_eq!(
        env,
        serde_json::json!([{
            "name": "PURLIS_CLAIM_UID",
            "valueFrom": { "fieldRef": {
                "apiVersion": "v1",
                "fieldPath": "metadata.labels['agents.x-k8s.io/claim-uid']",
            } },
        }])
    );
    assert_eq!(upstream::CLAIM_VAR, "PURLIS_CLAIM_UID");
}

#[test]
fn a_statement_holds_ten_minutes_and_is_signed_again_in_its_last_five() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let pod = runner_pod(the_sandboxes());
    let at = |now: u64, claim: &Claim| {
        vouch::vouch(
            &Held {
                now,
                cluster: CLUSTER,
                claim,
                upstream: &upstream,
                template: None,
                template_spec: None,
                sandbox: Some(&sandbox),
                pod: Some(&pod),
            },
            &pod_template(),
            &keys(),
        )
    };
    let mut claim = claim();
    let Vouching::Vouched(first) = at(NOW, &claim) else {
        panic!("not vouched for");
    };
    assert!(
        first
            .statement
            .ends_with(&format!("until: {}\n", NOW + 600))
    );
    assert_eq!(vouch::until_of(&first.statement), Some(NOW + 600));
    claim.status = Some(ClaimStatus {
        conditions: Vec::new(),
        link_key: Some(first.clone()),
    });
    // Looked at again before its last five minutes: the same statement, nothing written.
    assert_eq!(at(NOW + 290, &claim).link_key(), Some(first.clone()));
    // In them: signed again, holding ten minutes from now.
    let again = at(NOW + 310, &claim).link_key().expect("vouched");
    assert_ne!(again, first);
    assert_eq!(vouch::until_of(&again.statement), Some(NOW + 910));
    // A statement on the status that is not the operator's own is never kept.
    claim
        .status
        .as_mut()
        .unwrap()
        .link_key
        .as_mut()
        .unwrap()
        .signature = "0".repeat(128);
    assert_ne!(
        at(NOW + 10, &claim).link_key(),
        claim.status.as_ref().unwrap().link_key
    );
}

/// **An adopted warm runner's pod is found the way upstream records it** (#2058, from #2056):
/// by the name its sandbox's `agents.x-k8s.io/pod-name` annotation gives, where a sandbox
/// adopted before upstream's warm pool made whole sandboxes carries one, and by the sandbox's own
/// name otherwise. The pod must still be the one that sandbox controls, of the pool's template.
#[test]
fn an_adopted_runners_pod_is_found_by_the_name_its_sandbox_records() {
    let upstream = upstream_claim("ada-1");
    let plain = sandbox(UPSTREAM_UID);
    assert_eq!(vouch::pod_name_of(&plain), "ada-1");
    let mut recorded = sandbox(UPSTREAM_UID);
    recorded.annotations = Some(
        [(
            vouch::POD_NAME_ANNOTATION.to_owned(),
            "warm-7x2k".to_owned(),
        )]
        .into_iter()
        .collect(),
    );
    assert_eq!(vouch::POD_NAME_ANNOTATION, "agents.x-k8s.io/pod-name");
    assert_eq!(vouch::pod_name_of(&recorded), "warm-7x2k");
    // An empty annotation names no pod: the sandbox's own name holds, as upstream reads it.
    let mut empty = sandbox(UPSTREAM_UID);
    empty.annotations = Some(
        [(vouch::POD_NAME_ANNOTATION.to_owned(), String::new())]
            .into_iter()
            .collect(),
    );
    assert_eq!(vouch::pod_name_of(&empty), "ada-1");

    let mut adopted = runner_pod(the_sandboxes());
    adopted.metadata.name = Some("warm-7x2k".into());
    let Vouching::Vouched(link_key) = vouching(&upstream, Some(&recorded), Some(&adopted), None)
    else {
        panic!("the adopted pod is not vouched for");
    };
    assert!(link_key.statement.contains("pod: warm-7x2k\n"));
    // A pod of another name than the one recorded is never vouched for, even the sandbox's own.
    let named_as_the_sandbox = runner_pod(the_sandboxes());
    assert!(matches!(
        vouching(
            &upstream,
            Some(&recorded),
            Some(&named_as_the_sandbox),
            None
        ),
        Vouching::Refused(_)
    ));
    // And the recorded pod must be the one the sandbox controls.
    let mut squatted = runner_pod(owned_by(
        "Sandbox",
        "agents.x-k8s.io/v1beta1",
        "ada-1",
        POD_UID,
    ));
    squatted.metadata.name = Some("warm-7x2k".into());
    assert!(matches!(
        vouching(&upstream, Some(&recorded), Some(&squatted), None),
        Vouching::Refused(_)
    ));
}
