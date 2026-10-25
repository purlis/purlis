// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator vouches for a runner's link key (#2056): it signs the statement the desktop
//! reads, byte for byte as the shared worked example (`fixtures/runner-link-key/`), and only for
//! a key a pod of its own template published as its first step.

use base64::Engine;
use k8s_openapi::api::core::v1::{
    Container, ContainerState, ContainerStateTerminated, ContainerStatus, EnvVar, Pod, PodSpec,
    PodStatus, Volume, VolumeMount,
};
use kube::api::ObjectMeta;
use purlis_cluster_operator::upstream::{self, KEY_STEP, template_for};
use purlis_cluster_operator::vouch::{self, About, NotYet, PEM_BEGIN, PEM_END, SigningKey};
use purlis_cluster_operator::{Pool, PoolSpec, PoolStatus, pool_status_names};

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

/// One change to a pod.
type Change = Box<dyn Fn(&mut Pod)>;

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
                Err(NotYet::NotOurs(_))
            ),
            "{why}"
        );
    }
}

#[test]
fn a_pod_not_of_the_operators_template_for_the_pool_is_never_vouched_for() {
    let another_image = Pool::new(
        "team",
        PoolSpec {
            image: "ghcr.io/purlis/purlis-runner@sha256:3333333333333333333333333333333333333333333333333333333333333333".into(),
        },
    );
    assert!(matches!(
        vouch::key_published(&pod(Some((0, KEY))), &template_of(&another_image)),
        Err(NotYet::NotOurs(_))
    ));
    let changes: Vec<Change> = vec![
        Box::new(|pod| {
            let spec = pod.spec.as_mut().unwrap();
            let mut step = spec.init_containers.as_ref().unwrap()[0].clone();
            step.name = "first".into();
            spec.init_containers.as_mut().unwrap().insert(0, step);
        }),
        Box::new(|pod| first_step(pod).command = Some(vec!["/bin/sh".into()])),
        Box::new(|pod| first_step(pod).args = Some(vec!["--version".into()])),
        Box::new(|pod| {
            first_step(pod).env = Some(vec![EnvVar {
                name: "LD_PRELOAD".into(),
                value: Some("/tmp/x.so".into()),
                ..EnvVar::default()
            }]);
        }),
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
        Box::new(|pod| {
            pod.spec.as_mut().unwrap().containers[0].image = Some("elsewhere@sha256:44".into());
        }),
        Box::new(|pod| pod.spec.as_mut().unwrap().containers[0].security_context = None),
        Box::new(|pod| pod.spec.as_mut().unwrap().host_pid = Some(true)),
        Box::new(|pod| pod.spec.as_mut().unwrap().share_process_namespace = Some(true)),
        Box::new(|pod| pod.spec.as_mut().unwrap().automount_service_account_token = None),
        Box::new(|pod| {
            pod.spec
                .as_mut()
                .unwrap()
                .volumes
                .get_or_insert_default()
                .push(Volume {
                    name: "more".into(),
                    ..Volume::default()
                });
        }),
    ];
    for (at, change) in changes.iter().enumerate() {
        let mut changed = pod(Some((0, KEY)));
        change(&mut changed);
        assert!(
            matches!(
                vouch::key_published(&changed, &template()),
                Err(NotYet::NotOurs(_))
            ),
            "change {at}"
        );
    }
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
