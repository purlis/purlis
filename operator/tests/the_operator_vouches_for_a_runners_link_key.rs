// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator vouches for a runner's link key (#2056): it signs the statement the desktop
//! reads, byte for byte as the shared worked example (`fixtures/runner-link-key/`), and only for
//! a key a pod of its own template published as its first step.

use base64::Engine;
use k8s_openapi::api::core::v1::{
    Container, ContainerState, ContainerStateTerminated, ContainerStatus, EnvVar, ExecAction,
    HostAlias, Lifecycle, LifecycleHandler, Pod, PodDNSConfig, PodResourceClaim,
    PodSecurityContext, PodSpec, PodStatus, Probe, Toleration, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{
    FieldsV1, ManagedFieldsEntry, OwnerReference,
};
use kube::api::ObjectMeta;
use purlis_cluster_operator::upstream::{
    self, KEY_STEP, Reference, SandboxClaim, SandboxClaimStatus, template_for,
};
use purlis_cluster_operator::vouch::{
    self, About, Held, NotYet, PEM_BEGIN, PEM_END, SigningKey, Vouching,
};
use purlis_cluster_operator::{
    Claim, ClaimSpec, ClaimStatus, LinkKeyIs, Pool, PoolSpec, PoolStatus, pool_status_names,
    status_patch,
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

fn the_example() -> About<'static> {
    About {
        namespace: "ada",
        claim: "ada-1",
        claim_uid: "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60",
        pod: "ada-1",
        pod_uid: "0a1b2c3d-4e5f-4061-8728-394a5b6c7d8e",
        key: KEY,
        requester: "ada@example.com",
    }
}

#[test]
fn the_statement_and_its_signature_are_the_worked_example_the_desktop_reads() {
    let signing = SigningKey::from_pkcs8(&the_examples_key()).unwrap();
    let text = vouch::statement(&the_example()).unwrap();
    assert_eq!(text, fixture("statement.txt"));
    assert_eq!(signing.public(), fixture("operator-key.txt").trim());
    assert_eq!(signing.sign(&text), fixture("signature.txt").trim());
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
            "/dev/termination-log"
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
        assert_eq!(mounts.len(), 1);
        assert_eq!(
            (mounts[0].name.as_str(), mounts[0].mount_path.as_str()),
            (folder.name.as_str(), upstream::LINK_FOLDER)
        );
    }
}

fn first_step(pod: &mut Pod) -> &mut Container {
    &mut pod.spec.as_mut().unwrap().init_containers.as_mut().unwrap()[0]
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
        spec.security_context = Some(PodSecurityContext::default());
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

fn claim() -> Claim {
    let mut claim = Claim::new(
        "ada-1",
        ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
        },
    );
    claim.metadata.namespace = Some("ada".into());
    claim.metadata.uid = Some("6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60".into());
    claim
}

fn upstream_claim(names: &str) -> SandboxClaim {
    let mut held = upstream::sandbox_claim_for(&claim());
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
    pod.metadata.namespace = Some("ada".into());
    pod.metadata.owner_references = Some(owner);
    pod
}

fn the_sandboxes() -> Vec<OwnerReference> {
    owned_by("Sandbox", "agents.x-k8s.io/v1beta1", "ada-1", SANDBOX_UID)
}

fn signing() -> SigningKey {
    SigningKey::from_pkcs8(&the_examples_key()).unwrap()
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
            claim: &claim,
            upstream,
            template: held_template,
            sandbox,
            pod,
        },
        &template(),
        &signing(),
    )
}

#[test]
fn a_claim_is_followed_to_its_pod_by_uid_and_its_key_vouched_for() {
    let upstream = upstream_claim("ada-1");
    let sandbox = sandbox(UPSTREAM_UID);
    let pod = runner_pod(the_sandboxes());
    let Vouching::Vouched(link_key) = vouching(&upstream, Some(&sandbox), Some(&pod), None) else {
        panic!("not vouched for");
    };
    assert!(
        link_key
            .statement
            .contains(&format!("pod uid: {POD_UID}\n"))
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
    // Its pod replaced: the new pod's statement is written over the old one.
    let mut replaced = runner_pod(the_sandboxes());
    replaced.metadata.uid = Some("55555555-5555-4555-8555-555555555555".into());
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
            .contains("pod uid: 55555555-5555-4555-8555-555555555555")
    );
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
            claim_uid: "not-a-uid",
            ..the_example()
        },
        About {
            pod_uid: "6F1C2B8E-0F43-4C2E-9D7A-1B2C3D4E5F60",
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

#[test]
fn a_pools_status_is_written_until_it_names_the_operators_signing_key() {
    let public = SigningKey::from_pkcs8(&the_examples_key())
        .unwrap()
        .public();
    let mut pool = pool();
    assert!(!pool_status_names(&pool, &public));
    pool.status = Some(PoolStatus {
        signing_key: Some("ed25519:00".into()),
    });
    assert!(!pool_status_names(&pool, &public));
    pool.status = Some(PoolStatus {
        signing_key: Some(public.clone()),
    });
    assert!(pool_status_names(&pool, &public));
}
