// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the operator decides before it mints (#4206): that a runner holds its vouched key, what a
//! token may cover, and what it holds GitHub's answer to. The live refusals and the mint are on the
//! kind job (`minting_on_a_kind_cluster.rs`).

use k8s_openapi::ByteString;
use k8s_openapi::api::admissionregistration::v1::{
    ValidatingAdmissionPolicy, ValidatingAdmissionPolicyBinding,
};
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::ObjectMeta;
use purlis_cluster_operator::forge::{self, AppKey, Covered, Read};
use purlis_cluster_operator::github::{self, NamedRepo, Token};
use purlis_cluster_operator::mint::{self, Asked, Challenges, Held};
use purlis_cluster_operator::vouch::{self, About, SigningKey, SigningKeys};
use purlis_cluster_operator::{
    Claim, ClaimForge, ClaimSpec, ClaimStatus, FORGE_POLICY, LinkKey, POLICY, Pool, PoolForge,
    PoolSpec, SecretRef, forge_in_force_since,
};
use ring::agreement::{self, EphemeralPrivateKey, UnparsedPublicKey, X25519};
use serde_json::{Map, Value, json};
use tokio::io::AsyncWriteExt;

mod app_key;

const IMAGE: &str =
    "example.test/runner@sha256:0000000000000000000000000000000000000000000000000000000000000000";
const UID: &str = "0f0f0f0f-0f0f-4f0f-8f0f-0f0f0f0f0f0f";
const NOW: u64 = 2_000_000_000;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn covered(allow: &[&str], claim: &[&str]) -> Result<Covered, String> {
    forge::covered(&strings(allow), &strings(claim))
}

#[test]
fn a_token_covers_exactly_the_repos_on_both_lists() {
    assert_eq!(
        covered(&["acme/*"], &["acme/app", "acme/web"]),
        Ok(Covered {
            owner: "acme".into(),
            names: strings(&["app", "web"]),
        })
    );
    // What is on the Claim alone is left out, and what is on the Pool alone is not added.
    assert_eq!(
        covered(&["acme/app", "acme/docs"], &["acme/app", "acme/secrets"]),
        Ok(Covered {
            owner: "acme".into(),
            names: strings(&["app"]),
        })
    );
}

#[test]
fn github_names_do_not_tell_case_apart_and_neither_does_the_list() {
    assert_eq!(
        covered(&["ACME/*"], &["acme/App", "Acme/app"]),
        Ok(Covered {
            owner: "acme".into(),
            names: strings(&["App"]),
        })
    );
    assert!(covered(&["acme/APP"], &["Acme/app"]).is_ok());
}

/// GitHub reads an omitted list as every repo of the installation, so no list means no mint.
#[test]
fn nothing_on_both_lists_mints_nothing() {
    assert!(covered(&["acme/*"], &[]).is_err());
    assert!(covered(&["acme/app"], &["acme/other"]).is_err());
    assert!(covered(&["other/*"], &["acme/app"]).is_err());
}

#[test]
fn an_allow_list_entry_is_read_as_a_whole_name() {
    assert!(covered(&["acme/app"], &["acme/app-secrets"]).is_err());
    assert!(covered(&["acme/*"], &["acme-other/app"]).is_err());
}

#[test]
fn one_token_is_of_one_owner() {
    let why = covered(&["acme/*", "other/*"], &["acme/app", "other/app"]).expect_err("refused");
    assert!(why.contains("owners"), "{why}");
    // An owner the allow-list does not hold does not count.
    assert!(covered(&["acme/*"], &["acme/app", "other/app"]).is_ok());
}

#[test]
fn a_name_that_could_not_be_a_path_part_is_left_out() {
    assert!(covered(&["acme/*"], &["acme/.."]).is_err());
    assert!(covered(&["acme/*"], &["acme/a?b"]).is_err());
    assert!(covered(&["acme/*"], &["acme/a/b"]).is_err());
}

#[test]
fn more_repos_than_github_takes_are_refused() {
    let many: Vec<String> = (0..=forge::MOST_REPOS)
        .map(|at| format!("acme/repo{at}"))
        .collect();
    assert!(forge::covered(&strings(&["acme/*"]), &many).is_err());
}

fn signing_keys() -> SigningKeys {
    let rng = ring::rand::SystemRandom::new();
    let der = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).expect("a key");
    SigningKeys {
        current: SigningKey::from_pkcs8(der.as_ref()).expect("a key"),
        next: None,
    }
}

fn made_at(seconds: i64) -> Option<Time> {
    Some(Time(
        k8s_openapi::jiff::Timestamp::from_second(seconds).expect("a time"),
    ))
}

fn claim_with(keys: &SigningKeys, key: &str, until: u64) -> Claim {
    let mut claim = Claim::new(
        "work",
        ClaimSpec {
            pool: "team".into(),
            requester: "ada".into(),
            kept: false,
            stopped: false,
            forge: Some(ClaimForge {
                repos: strings(&["acme/app"]),
            }),
        },
    );
    claim.metadata.namespace = Some("team-ns".into());
    claim.metadata.uid = Some(UID.into());
    claim.metadata.creation_timestamp = made_at(1000);
    let statement = vouch::statement(&About {
        cluster: UID,
        namespace: "team-ns",
        claim: "work",
        claim_uid: UID,
        runner_namespace: "purlis-runners-ada",
        pod: "work-pod",
        key,
        requester: "ada",
        until,
    })
    .expect("a statement");
    claim.status = Some(ClaimStatus {
        link_key: Some(LinkKey {
            signature: keys.current.sign(&statement),
            next_signature: None,
            statement,
        }),
        conditions: Vec::new(),
        mint: None,
    });
    claim
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A runner: a link key, and the proof it answers a challenge with.
struct Runner {
    private: EphemeralPrivateKey,
    public: Vec<u8>,
}

fn runner() -> Runner {
    let rng = ring::rand::SystemRandom::new();
    let private = EphemeralPrivateKey::generate(&X25519, &rng).expect("a key");
    let public = private.compute_public_key().expect("its public half");
    Runner {
        public: public.as_ref().to_vec(),
        private,
    }
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

impl Runner {
    fn prove(self, challenge: &str, text: &str) -> String {
        let bytes = unhex(challenge);
        let peer = UnparsedPublicKey::new(&X25519, bytes.clone());
        agreement::agree_ephemeral(self.private, &peer, |shared| {
            mint::proof_for(shared, &bytes, text).expect("a proof")
        })
        .expect("an agreement")
    }
}

#[test]
fn the_holder_of_the_link_key_proves_it_and_no_one_else_can() {
    let challenges = Challenges::default();
    let text =
        |challenge: &str| mint::proof_text("ed25519:op", "team-ns", "work", "Iv23", challenge);

    let holder = runner();
    let key = holder.public.clone();
    let challenge = challenges.make().expect("a challenge");
    let proof = holder.prove(&challenge, &text(&challenge));
    assert!(challenges.proved(&challenge, &key, &[text(&challenge)], &proof));

    // Another key's proof does not hold for this one.
    let challenge = challenges.make().expect("a challenge");
    let proof = runner().prove(&challenge, &text(&challenge));
    assert!(!challenges.proved(&challenge, &key, &[text(&challenge)], &proof));

    // A proof is of what was asked: another app is another text.
    let holder = runner();
    let key = holder.public.clone();
    let challenge = challenges.make().expect("a challenge");
    let proof = holder.prove(&challenge, &text(&challenge));
    let other = mint::proof_text("ed25519:op", "team-ns", "work", "Iv23other", &challenge);
    assert!(!challenges.proved(&challenge, &key, &[other], &proof));
}

#[test]
fn a_challenge_answers_once_and_only_if_it_was_made_here() {
    let challenges = Challenges::default();

    // A proof made for another operator's key is no proof to this one.
    let holder = runner();
    let key = holder.public.clone();
    let challenge = challenges.make().expect("a challenge");
    let theirs = mint::proof_text("ed25519:other", "team-ns", "work", "Iv23", &challenge);
    let proof = holder.prove(&challenge, &theirs);
    let ours = mint::proof_text("ed25519:op", "team-ns", "work", "Iv23", &challenge);
    assert!(!challenges.proved(&challenge, &key, &[ours], &proof));

    let holder = runner();
    let key = holder.public.clone();
    let challenge = challenges.make().expect("a challenge");
    let text = mint::proof_text("ed25519:op", "team-ns", "work", "Iv23", &challenge);
    let proof = holder.prove(&challenge, &text);
    assert!(challenges.proved(&challenge, &key, std::slice::from_ref(&text), &proof));
    assert!(
        !challenges.proved(&challenge, &key, std::slice::from_ref(&text), &proof),
        "a proof was taken twice"
    );

    let unknown = hex(&[7u8; 32]);
    assert!(!challenges.proved(&unknown, &key, std::slice::from_ref(&text), &proof));
}

/// A low-order public key would make the agreement the same whoever computes it.
#[test]
fn a_key_of_low_order_proves_nothing() {
    let challenges = Challenges::default();
    let challenge = challenges.make().expect("a challenge");
    let text = mint::proof_text("ed25519:op", "team-ns", "work", "Iv23", &challenge);
    let proof = mint::proof_for(&[0u8; 32], &unhex(&challenge), &text).expect("a proof");
    assert!(!challenges.proved(&challenge, &[0u8; 32], std::slice::from_ref(&text), &proof));
}

#[test]
fn only_a_statement_of_this_operator_on_this_claim_that_still_holds_vouches() {
    let keys = signing_keys();
    let key = "ab".repeat(32);
    let claim = claim_with(&keys, &key, NOW + 100);
    let said = mint::vouched(&claim, &keys, NOW).expect("vouched");
    assert_eq!(said.key_hex, key);
    assert_eq!(said.pod, "work-pod");
    assert_eq!(said.runner_namespace, "purlis-runners-ada");

    assert!(mint::vouched(&claim, &keys, NOW + 100).is_err(), "expired");
    assert!(
        mint::vouched(&claim, &signing_keys(), NOW).is_err(),
        "another operator's"
    );

    let mut bare = claim.clone();
    bare.status = None;
    assert!(mint::vouched(&bare, &keys, NOW).is_err());

    // A statement copied from another claim names that claim.
    let mut other = claim.clone();
    other.metadata.name = Some("other".into());
    assert!(mint::vouched(&other, &keys, NOW).is_err());
    let mut other = claim.clone();
    other.metadata.uid = Some("1f1f1f1f-0f0f-4f0f-8f0f-0f0f0f0f0f0f".into());
    assert!(mint::vouched(&other, &keys, NOW).is_err());
    let mut other = claim;
    other.spec.requester = "eve".into();
    assert!(mint::vouched(&other, &keys, NOW).is_err());
}

fn binding_of(policy: &str, made: i64) -> ValidatingAdmissionPolicyBinding {
    let mut binding: ValidatingAdmissionPolicyBinding = serde_json::from_value(json!({
        "metadata": { "name": policy },
        "spec": { "policyName": policy, "validationActions": ["Deny"] },
    }))
    .expect("a binding");
    binding.metadata.creation_timestamp = made_at(made);
    binding
}

fn pool_of(mint_in_cluster: bool) -> Pool {
    let mut spec = PoolSpec::new(IMAGE);
    spec.forge = Some(PoolForge {
        app_key_secret_ref: SecretRef {
            name: "org-app".into(),
        },
        repos: strings(&["acme/*"]),
        mint_in_cluster,
    });
    let mut pool = Pool::new("team", spec);
    pool.metadata.creation_timestamp = made_at(1000);
    pool
}

/// The Pool and Claim made at 1000, and both policies bound long before.
fn may_mint(pool: &Pool, claim: &Claim) -> Result<mint::Grant, String> {
    let binding = binding_of(POLICY, 10);
    mint::may_mint(&Held {
        claim,
        pool,
        claims_binding: Some(&binding),
        forge_since: made_at(100).map(|at| at.0),
    })
}

#[test]
fn a_claim_on_an_opted_in_pool_under_the_policies_may_mint() {
    let keys = signing_keys();
    let claim = claim_with(&keys, &"ab".repeat(32), NOW);
    let grant = may_mint(&pool_of(true), &claim).expect("a grant");
    assert_eq!(grant.secret, "org-app");
    assert_eq!(grant.covered.names, ["app"]);
}

/// The desktop's record of the pool has no say: a runner asks by itself, so this is the only gate.
#[test]
fn a_pool_not_opted_in_mints_nothing_whatever_anyone_says() {
    let keys = signing_keys();
    let claim = claim_with(&keys, &"ab".repeat(32), NOW);
    assert!(may_mint(&pool_of(false), &claim).is_err());
    let mut bare = pool_of(true);
    bare.spec.forge = None;
    assert!(may_mint(&bare, &claim).is_err());
}

#[test]
fn a_stopped_or_going_claim_or_pool_mints_nothing() {
    let keys = signing_keys();
    let claim = claim_with(&keys, &"ab".repeat(32), NOW);
    let pool = pool_of(true);
    let mut stopped = claim.clone();
    stopped.spec.stopped = true;
    assert!(may_mint(&pool, &stopped).is_err());
    let mut stopped = pool.clone();
    stopped.spec.stopped = true;
    assert!(may_mint(&stopped, &claim).is_err());
    let mut going = claim.clone();
    going.metadata.deletion_timestamp = made_at(2000);
    assert!(may_mint(&pool, &going).is_err());
    let mut going = pool;
    going.metadata.deletion_timestamp = made_at(2000);
    assert!(may_mint(&going, &claim).is_err());
}

#[test]
fn a_claim_with_no_repos_mints_nothing() {
    let keys = signing_keys();
    let mut claim = claim_with(&keys, &"ab".repeat(32), NOW);
    claim.spec.forge = None;
    assert!(may_mint(&pool_of(true), &claim).is_err());
    claim.spec.forge = Some(ClaimForge { repos: Vec::new() });
    assert!(may_mint(&pool_of(true), &claim).is_err());
}

/// What was written before the policy on App keys held was not held to it.
#[test]
fn what_was_made_before_the_policy_on_app_keys_settled_mints_nothing() {
    let keys = signing_keys();
    let claim = claim_with(&keys, &"ab".repeat(32), NOW);
    let pool = pool_of(true);
    let binding = binding_of(POLICY, 10);
    let asked = |since: Option<k8s_openapi::jiff::Timestamp>, claim: &Claim, pool: &Pool| {
        mint::may_mint(&Held {
            claim,
            pool,
            claims_binding: Some(&binding),
            forge_since: since,
        })
    };
    assert!(asked(None, &claim, &pool).is_err(), "not in force");
    let later = made_at(5000).map(|at| at.0);
    let why = asked(later, &claim, &pool).expect_err("refused");
    assert!(why.contains("claim"), "{why}");
    let mut fresh = claim.clone();
    fresh.metadata.creation_timestamp = made_at(6000);
    let why = asked(later, &fresh, &pool).expect_err("refused");
    assert!(why.contains("pool"), "{why}");
    let mut fresh_pool = pool;
    fresh_pool.metadata.creation_timestamp = made_at(6000);
    assert!(asked(later, &fresh, &fresh_pool).is_ok());
}

#[test]
fn the_policy_on_app_keys_is_in_force_only_with_its_binding_and_itself_settled() {
    let binding = binding_of(FORGE_POLICY, 1000);
    let mut policy = ValidatingAdmissionPolicy::default();
    policy.metadata.creation_timestamp = made_at(2000);
    assert_eq!(forge_in_force_since(None, Some(&binding)), None);
    assert_eq!(forge_in_force_since(Some(&policy), None), None);
    let since = forge_in_force_since(Some(&policy), Some(&binding)).expect("in force");
    assert_eq!(since.as_second(), 2030, "the later of the two, settled");

    let mut audit = binding.clone();
    audit.spec.validation_actions = strings(&["Audit"]);
    assert_eq!(forge_in_force_since(Some(&policy), Some(&audit)), None);
    let mut other = binding;
    other.spec.policy_name = "other".into();
    assert_eq!(forge_in_force_since(Some(&policy), Some(&other)), None);
}

#[test]
fn the_review_asks_of_the_requester_by_name_about_this_pool() {
    let review = mint::use_review("ada", "team");
    assert_eq!(review.spec.user.as_deref(), Some("ada"));
    assert_eq!(review.spec.groups, None);
    let asked = review.spec.resource_attributes.expect("its attributes");
    assert_eq!(asked.group.as_deref(), Some("runners.purlis.dev"));
    assert_eq!(asked.resource.as_deref(), Some("pools"));
    assert_eq!(asked.name.as_deref(), Some("team"));
    assert_eq!(asked.verb.as_deref(), Some("use"));
}

fn permissions() -> Map<String, Value> {
    json!({ "contents": "write", "pull_requests": "read" })
        .as_object()
        .expect("an object")
        .clone()
}

fn token() -> Token {
    Token {
        token: "made-up-token".into(),
        expires_at: None,
        repository_selection: Some("selected".into()),
        repositories: vec![NamedRepo { name: "App".into() }],
        permissions: permissions(),
    }
}

/// Whatever the installation holds, a token never asks for what administers anything.
#[test]
fn a_token_asks_only_for_repo_level_permissions_that_administer_nothing() {
    let held = json!({
        "contents": "write",
        "administration": "write",
        "secrets": "write",
        "packages": "write",
        "repository_projects": "admin",
        "workflows": "write",
        "members": "read",
        "organization_administration": "write",
    });
    let asked = github::mintable(held.as_object().expect("an object"));
    assert_eq!(Value::Object(asked), json!({ "contents": "write" }));
}

fn pod_with(phase: &str, key: &str) -> k8s_openapi::api::core::v1::Pod {
    serde_json::from_value(json!({
        "metadata": { "name": "work-pod" },
        "status": {
            "phase": phase,
            "initContainerStatuses": [{
                "name": "link-key",
                "image": "i", "imageID": "i", "ready": true, "restartCount": 0,
                "state": { "terminated": { "exitCode": 0, "message": key } },
            }],
        },
    }))
    .expect("a pod")
}

#[test]
fn a_runner_of_another_image_than_its_pools_mints_nothing() {
    let pod: k8s_openapi::api::core::v1::Pod = serde_json::from_value(json!({
        "spec": {
            "initContainers": [{ "name": "link-key", "image": "new" }],
            "containers": [{ "name": "runner", "image": "new" }],
        },
    }))
    .expect("a pod");
    assert!(mint::runs_image(&pod, "new"));
    assert!(!mint::runs_image(&pod, "old"));
    assert!(!mint::runs_image(
        &k8s_openapi::api::core::v1::Pod::default(),
        "new"
    ));
}

#[test]
fn only_a_running_pod_still_publishing_the_key_holds_it() {
    let key = "ab".repeat(32);
    assert!(mint::pod_holds(&pod_with("Running", &key), &key));
    assert!(!mint::pod_holds(
        &pod_with("Running", &"cd".repeat(32)),
        &key
    ));
    assert!(!mint::pod_holds(&pod_with("Failed", &key), &key));
    assert!(!mint::pod_holds(&pod_with("Succeeded", &key), &key));
    let mut going = pod_with("Running", &key);
    going.metadata.deletion_timestamp = made_at(2000);
    assert!(!mint::pod_holds(&going, &key));
    assert!(!mint::pod_holds(
        &k8s_openapi::api::core::v1::Pod::default(),
        &key
    ));
}

#[test]
fn every_token_request_names_its_repositories_and_permissions() {
    let asked = github::token_request(&strings(&["app"]), &permissions());
    assert_eq!(asked["repositories"], json!(["app"]));
    assert_eq!(asked["permissions"], Value::Object(permissions()));
}

#[test]
fn a_token_that_says_it_covers_anything_else_is_refused() {
    let names = strings(&["app"]);
    assert_eq!(github::checked(&token(), &names, &permissions()), Ok(()));

    let mut all = token();
    all.repository_selection = Some("all".into());
    assert!(github::checked(&all, &names, &permissions()).is_err());
    let mut silent = token();
    silent.repository_selection = None;
    assert!(github::checked(&silent, &names, &permissions()).is_err());

    let mut more = token();
    more.repositories.push(NamedRepo {
        name: "other".into(),
    });
    assert!(github::checked(&more, &names, &permissions()).is_err());
    let mut none = token();
    none.repositories.clear();
    assert!(github::checked(&none, &names, &permissions()).is_err());

    let mut stronger = token();
    stronger
        .permissions
        .insert("pull_requests".into(), json!("write"));
    assert!(github::checked(&stronger, &names, &permissions()).is_err());
    let mut added = token();
    added.permissions.insert("admin".into(), json!("write"));
    assert!(github::checked(&added, &names, &permissions()).is_err());
    let mut empty = token();
    empty.permissions.clear();
    assert!(github::checked(&empty, &names, &permissions()).is_err());
}

#[test]
fn what_is_printed_of_a_token_never_holds_it() {
    assert!(!format!("{:?}", token()).contains("made-up-token"));
}

/// The client's own typed read logs a body it cannot parse, which would be the key.
#[test]
fn a_secret_read_that_does_not_parse_says_nothing_of_what_it_read() {
    let body = r#"{"kind":"Secret","data":{"key":"not base64 hunter2"}}"#;
    let Read::Failed(why) = forge::secret_of(body) else {
        panic!("taken");
    };
    assert!(!why.contains("hunter2"), "{why}");
    let Read::Failed(why) = forge::secret_of("hunter2") else {
        panic!("taken");
    };
    assert!(!why.contains("hunter2"), "{why}");

    let pem = app_key::pem();
    let body = json!({
        "kind": "Secret",
        "apiVersion": "v1",
        "metadata": { "name": "org-app" },
        "data": {
            "app": base64_of(b"Iv23"),
            "key": base64_of(pem.as_bytes()),
        },
    });
    let key = forge::key_of(forge::secret_of(&body.to_string())).expect("the key");
    assert_eq!(key.app(), "Iv23");
}

fn base64_of(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[test]
fn a_key_signs_a_jwt_github_would_read() {
    use base64::Engine as _;
    let pem = app_key::pem();
    let secret = Secret {
        data: Some(
            [
                ("app".to_owned(), ByteString(b"Iv23".to_vec())),
                ("key".to_owned(), ByteString(pem.into_bytes())),
            ]
            .into(),
        ),
        ..Secret::default()
    };
    let jwt = AppKey::of(&secret).expect("a key").jwt(NOW).expect("a JWT");
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(parts.len(), 3);
    let url = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let claims: Value = serde_json::from_slice(&url.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(claims["iss"], "Iv23");
    assert_eq!(claims["iat"], NOW - 60);
    assert_eq!(claims["exp"], NOW + 540);
    let header: Value = serde_json::from_slice(&url.decode(parts[0]).unwrap()).unwrap();
    assert_eq!(header["alg"], "RS256");
}

#[test]
fn a_request_is_held_to_its_spelling() {
    let asked = Asked {
        namespace: "team-ns".into(),
        claim: "work".into(),
        app: "Iv23".into(),
        challenge: "a".repeat(64),
        proof: "b".repeat(64),
    };
    assert!(asked.well_spelled());
    for bad in [
        Asked {
            namespace: "team\nforged".into(),
            ..asked.clone()
        },
        Asked {
            claim: "work\nchallenge: 00".into(),
            ..asked.clone()
        },
        Asked {
            app: "Iv23\nforged".into(),
            ..asked.clone()
        },
        Asked {
            app: String::new(),
            ..asked.clone()
        },
        Asked {
            challenge: "a".into(),
            ..asked.clone()
        },
        Asked {
            proof: "b".repeat(63),
            ..asked.clone()
        },
    ] {
        assert!(!bad.well_spelled(), "{bad:?}");
    }
    let extra = json!({
        "namespace": "n", "claim": "c", "app": "a", "challenge": "", "proof": "", "repos": ["x/y"],
    });
    assert!(serde_json::from_value::<Asked>(extra).is_err());
}

async fn request_of(sent: &[u8]) -> Option<(String, String, Vec<u8>)> {
    let (mut near, mut far) = tokio::io::duplex(64 * 1024);
    near.write_all(sent).await.expect("written");
    drop(near);
    mint::read_request(&mut far).await.expect("read")
}

#[tokio::test]
async fn a_request_is_read_only_when_its_length_is_plain() {
    let (method, target, body) =
        request_of(b"POST /forge/token HTTP/1.1\r\ncontent-length: 2\r\n\r\n{}")
            .await
            .expect("read");
    assert_eq!((method.as_str(), target.as_str()), ("POST", "/forge/token"));
    assert_eq!(body, b"{}");

    for bad in [
        &b"POST / HTTP/1.1\r\ntransfer-encoding: chunked\r\n\r\n"[..],
        b"POST / HTTP/1.1\r\ncontent-length: 2\r\ncontent-length: 3\r\n\r\n{}",
        b"POST / HTTP/1.1\r\ncontent-length: nine\r\n\r\n",
        b"POST / HTTP/1.1\r\ncontent-length: 99999999\r\n\r\n",
        b"POST / HTTP/1.1\r\ncontent-length: 5\r\n\r\n{}",
        b"POST / HTTP/1.1\r\n",
    ] {
        assert_eq!(
            request_of(bad).await,
            None,
            "{}",
            String::from_utf8_lossy(bad)
        );
    }
    let long = vec![b'a'; 32 * 1024];
    assert_eq!(request_of(&long).await, None);
}

#[test]
fn the_ports_certificate_holds_the_signing_key_and_tls_takes_it() {
    let keys = signing_keys();
    let certified = mint::certified(&keys.current).expect("TLS takes it");
    assert_eq!(certified.keys_match(), Ok(()));
    let certificate = mint::certificate(&keys.current);
    let public = keys.current.public_bytes();
    assert!(
        certificate.windows(public.len()).any(|at| at == public),
        "the certificate holds the signing key's public half"
    );
    assert_eq!(
        mint::certificate(&keys.current),
        certificate,
        "one key, one certificate"
    );
}

#[test]
fn a_runner_namespaces_way_out_is_to_the_operators_mint_port_alone() {
    let policy = mint::egress_policy("purlis-runners-ada", "purlis-system");
    assert_eq!(
        policy.metadata,
        ObjectMeta {
            name: Some(forge::MINT_EGRESS_POLICY.into()),
            namespace: Some("purlis-runners-ada".into()),
            ..ObjectMeta::default()
        }
    );
    let spec = serde_json::to_value(policy.spec).expect("JSON");
    assert_eq!(spec["policyTypes"], json!(["Egress"]));
    assert_eq!(spec.get("ingress"), None);
    assert_eq!(
        spec["egress"],
        json!([{
            "to": [{
                "namespaceSelector": {
                    "matchLabels": { "kubernetes.io/metadata.name": "purlis-system" },
                },
                "podSelector": {
                    "matchLabels": { "app.kubernetes.io/name": "purlis-cluster-operator" },
                },
            }],
            "ports": [{ "port": mint::PORT, "protocol": "TCP" }],
        }])
    );
}

fn proof_fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../fixtures/forge-proof/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("the worked example")
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let text = proof_fixture(name);
    let text = text.trim();
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

/// The runner is built apart, and holds its side to the same files (`forge/mintport.rs`).
#[test]
fn a_proof_is_the_worked_example_a_runner_holds_too() {
    let text = proof_fixture("text.txt");
    assert_eq!(
        mint::proof_for(
            &fixture_bytes("shared.hex"),
            &fixture_bytes("challenge.hex"),
            &text
        )
        .expect("a proof"),
        proof_fixture("proof.hex").trim()
    );
    assert_eq!(
        text,
        mint::proof_text(
            &format!("ed25519:{}", "ab".repeat(32)),
            "team",
            "ada-1",
            "Iv23liExample",
            proof_fixture("challenge.hex").trim(),
        )
    );
}

fn a_pool_minting(opted_in: bool) -> Pool {
    let mut spec = PoolSpec::new(IMAGE.to_owned());
    spec.forge = Some(PoolForge {
        app_key_secret_ref: SecretRef {
            name: "org-app".to_owned(),
        },
        repos: strings(&["acme/*"]),
        mint_in_cluster: opted_in,
    });
    Pool::new("pool", spec)
}

#[test]
fn a_claims_status_says_where_to_ask_only_for_a_vouched_runner_on_a_pool_that_opts_in() {
    let key = format!("ed25519:{}", "ab".repeat(32));
    let address = || Some("10.96.0.5:8443".to_owned());
    let said = mint::mint_status(&a_pool_minting(true), true, address(), &key).expect("said");
    assert_eq!(said.address, "10.96.0.5:8443");
    assert_eq!(said.operator_key, key);
    assert_eq!(
        serde_json::to_value(&said).expect("a value"),
        json!({ "address": "10.96.0.5:8443", "operatorKey": key }),
    );
    assert_eq!(
        mint::mint_status(&a_pool_minting(false), true, address(), &key),
        None
    );
    assert_eq!(
        mint::mint_status(&a_pool_minting(true), false, address(), &key),
        None
    );
    assert_eq!(
        mint::mint_status(&a_pool_minting(true), true, None, &key),
        None
    );
    assert_eq!(
        mint::mint_status(
            &Pool::new("pool", PoolSpec::new(IMAGE.to_owned())),
            true,
            address(),
            &key
        ),
        None
    );
}

#[test]
fn the_ports_address_is_the_services_cluster_ip_and_nothing_else() {
    use k8s_openapi::api::core::v1::{Service, ServiceSpec};
    let with = |ip: Option<&str>| Service {
        spec: Some(ServiceSpec {
            cluster_ip: ip.map(str::to_owned),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    };
    assert_eq!(
        mint::address_of(&with(Some("10.96.0.5"))).as_deref(),
        Some("10.96.0.5:8443")
    );
    assert_eq!(
        mint::address_of(&with(Some("fd00::5"))).as_deref(),
        Some("[fd00::5]:8443")
    );
    for none in [None, Some("None"), Some("")] {
        assert_eq!(mint::address_of(&with(none)), None, "{none:?}");
    }
}

#[test]
fn a_status_that_stops_saying_where_to_ask_takes_it_away() {
    use purlis_cluster_operator::{LinkKeyIs, status_patch};
    let with = |mint| ClaimStatus {
        conditions: Vec::new(),
        link_key: None,
        mint,
    };
    let said = purlis_cluster_operator::ClaimMint {
        address: "10.96.0.5:8443".to_owned(),
        operator_key: format!("ed25519:{}", "ab".repeat(32)),
    };
    let held = with(Some(said));
    let patch = status_patch(Some(&held), with(None), LinkKeyIs::Said).expect("a change");
    assert_eq!(patch["status"]["mint"], Value::Null);
    // A pass that did not look at the runner leaves it as it was.
    assert_eq!(status_patch(Some(&held), with(None), LinkKeyIs::Kept), None);
}
