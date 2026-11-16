// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner start carries one key-proxy plan** (T1a-51, R-469, R-471, R-168, R-652, H-18).
//!
//! Driven at the plan seam over the in-memory link (T1a-50): the desktop sends the plan with
//! the run's start, the stand-in runner checks it at arrival against its own binary and keeps it
//! for that run alone, and answers. No runner host starts a key proxy here (T1a-66's), and no
//! upstream is reached.

#![cfg(unix)]

use purlis_core::harness_declaration::SignInKind;
use purlis_core::keyplan::{Binary, Plans, Refused, plan_for};
use purlis_core::model_provider::{self, Declaration, From};
use purlis_core::poollimits::Chosen;
use purlis_core::profilestore::onrunner::Kept;
use purlis_session_protocol::keyplan::{KeyProxyPlan, PLAN_DOES_NOT_MATCH, PlanKey, PlanLocks};
use purlis_session_protocol::session::{Command, Refusal};
use serde::{Deserialize, Serialize};

use crate::support::runner_link::{End, InMemoryLink};

const RUN: &str = "01J9RUN00000000000000000KP";
const KEY: &str = "a-stand-in-model-key";

/// What crosses the link in these tests: the desktop's command, and the runner's answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Wire {
    Call(Box<Command>),
    Answer(Result<(), Refusal>),
}

fn the_anthropic_text() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/model_provider/built_in/anthropic.toml"
    ))
    .expect("the shipped built-in")
}

fn built_in(text: &str) -> Declaration {
    model_provider::parse(text, &From::BuiltIn, &[], "anthropic.toml").expect("a built-in")
}

fn desktops_anthropic() -> &'static Declaration {
    model_provider::built_ins()
        .iter()
        .find(|declaration| declaration.id == "anthropic")
        .expect("1a ships the Anthropic built-in")
}

fn the_plan(declaration: &Declaration) -> KeyProxyPlan {
    plan_for(
        RUN,
        declaration,
        SignInKind::ApiKey,
        vec!["https://api.anthropic.com".into()],
        None,
        PlanLocks::default(),
        PlanKey::of(KEY),
    )
}

/// A runner's kept profile, as `keep_profile` makes it: it has no `sign_in` to fill (R-652).
fn kept() -> Kept {
    Kept {
        name: "claude-anthropic".into(),
        harness: "claude".into(),
        sandbox_only: false,
        pinned: None,
        settings_in: None,
        limits: Chosen::default(),
        highest: Chosen::default(),
        mcp_state: None,
    }
}

/// Sends `plan` from the desktop with the run's start; the stand-in runner takes it into
/// `plans` and answers. Returns the answer the desktop read.
async fn send_the_start(
    link: &mut InMemoryLink<Wire>,
    plans: &mut Plans,
    plan: KeyProxyPlan,
) -> Result<(), Refusal> {
    link.send(End::Desktop, Wire::Call(Box::new(Command::KeyProxyPlan(Box::new(plan)))))
        .await
        .expect("the link is up");
    let Some(Wire::Call(command)) = link.receive(End::Runner).await else {
        panic!("the runner reads the start");
    };
    let Command::KeyProxyPlan(plan) = *command else {
        panic!("the start's plan");
    };
    let answer = plans
        .take(*plan)
        .map(|_| ())
        .map_err(|refused| refused.on_the_wire());
    link.send(End::Runner, Wire::Answer(answer))
        .await
        .expect("the link is up");
    match link.receive(End::Desktop).await {
        Some(Wire::Answer(answer)) => answer,
        other => panic!("the runner's answer, not {other:?}"),
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
}

fn is_a_plan(wire: &Wire) -> bool {
    matches!(wire, Wire::Call(command) if matches!(**command, Command::KeyProxyPlan(_)))
}

#[test]
fn a_runner_start_carries_one_key_proxy_plan_and_the_runner_forgets_it_at_run_end() {
    runtime().block_on(async {
        let mut link = InMemoryLink::<Wire>::up();
        let mut plans = Plans::on(Binary::this());

        assert_eq!(
            send_the_start(&mut link, &mut plans, the_plan(desktops_anthropic())).await,
            Ok(())
        );
        assert_eq!(link.times_crossed(is_a_plan), 1, "one plan per run start");
        let held = plans.held(RUN).expect("kept for the run");
        assert_eq!(held.key.expose(), KEY, "the key came in the same round trip");

        // A second plan for the same run is refused: it crosses once.
        assert!(
            send_the_start(&mut link, &mut plans, the_plan(desktops_anthropic()))
                .await
                .is_err()
        );

        plans.end(RUN);
        assert!(plans.held(RUN).is_none(), "no plan after run end");
        assert!(plans.is_empty(), "nor any key");
    });
}

#[test]
fn a_built_in_digest_mismatch_refuses_the_start_naming_both_versions() {
    runtime().block_on(async {
        let mut link = InMemoryLink::<Wire>::up();
        // The stand-in runner's binary carries another Anthropic built-in: another digest.
        let other = built_in(&the_anthropic_text().replace(
            "name = \"Anthropic\"",
            "name = \"Anthropic, as an older purlis shipped it\"",
        ));
        assert_ne!(other.digest(), desktops_anthropic().digest());
        let mut plans = Plans::on(Binary {
            version: "0.4.9".into(),
            built_ins: vec![other],
        });

        let refusal = send_the_start(&mut link, &mut plans, the_plan(desktops_anthropic()))
            .await
            .expect_err("a mismatch refuses the start");
        assert_eq!(refusal.code, PLAN_DOES_NOT_MATCH);
        assert_eq!(refusal.why, "0.4.9", "the runner's version, beside the desktop's own");
        assert!(plans.is_empty(), "nothing kept, so no key proxy and no upstream");

        let mut plans = Plans::on(Binary {
            version: "0.4.9".into(),
            built_ins: vec![built_in(&the_anthropic_text().replace(
                "name = \"Anthropic\"",
                "name = \"Anthropic, as an older purlis shipped it\"",
            ))],
        });
        let refused = plans
            .take(the_plan(desktops_anthropic()))
            .expect_err("a mismatch");
        assert_eq!(
            refused,
            Refused::DoesNotMatch {
                id: "anthropic".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                runner_version: "0.4.9".into(),
            },
            "both versions named, for W75"
        );
        assert_eq!(refused.on_the_wire().why, "0.4.9");
    });
}

#[test]
fn the_runner_uses_the_plans_declaration_and_approves_nothing_of_its_own() {
    // The runner holds another declaration of its own, read from its disk.
    let its_own = model_provider::parse(
        &the_anthropic_text()
            .replace("id = \"anthropic\"", "id = \"my-anthropic\"")
            .replace("name = \"Anthropic\"", "name = \"My relay\""),
        &From::Yours,
        &[],
        "my-anthropic.toml",
    )
    .expect("the runner's own declaration");
    let mut binary = Binary::this();
    let mut built_ins = vec![its_own.clone()];
    built_ins.extend(binary.built_ins);
    binary.built_ins = built_ins;
    let mut plans = Plans::on(binary);

    let held = plans
        .take(the_plan(desktops_anthropic()))
        .expect("the plan's built-in is the binary's");
    assert_eq!(held.declaration.id, "anthropic");
    assert_eq!(held.declaration.digest(), desktops_anthropic().digest());
    plans.end(RUN);

    // A plan naming the runner's own declaration, even by its own digest, is refused: the
    // runner approves nothing of its own.
    let mut plan = the_plan(desktops_anthropic());
    plan.built_in.id = its_own.id.clone();
    plan.built_in.digest = its_own.digest().to_owned();
    assert!(matches!(
        plans.take(plan),
        Err(Refused::DoesNotMatch { .. })
    ));
    assert!(plans.is_empty());
}

#[test]
fn the_runner_learns_the_sign_in_kind_from_the_plan_not_its_kept_record() {
    let mut plans = Plans::on(Binary::this());
    let held = plans
        .take(the_plan(desktops_anthropic()))
        .expect("taken");
    let start = held.start_on(&kept());
    assert_eq!(start.sign_in, SignInKind::ApiKey);
    assert_eq!(start.profile, "claude-anthropic");

    // A sign-in kind 1a doesn't route is refused at arrival.
    let mut plans = Plans::on(Binary::this());
    let mut plan = the_plan(desktops_anthropic());
    plan.sign_in = "subscription".into();
    assert!(matches!(plans.take(plan), Err(Refused::SignIn(_))));
}

#[test]
fn a_runner_chat_uses_its_own_binarys_background_model_and_prices() {
    // The runner's binary carries the same built-in by digest, with its own background model and
    // price table: neither is in the digest, and neither travels in the plan.
    let runners = built_in(
        &(the_anthropic_text().replace(
            "background = \"claude-haiku-5-5\"",
            "background = \"claude-haiku-runner\"",
        ) + "\n[[models.table]]\nid = \"claude-haiku-runner\"\nprice = { input = 0.5, output = 2.5 }\n"),
    );
    assert_eq!(runners.digest(), desktops_anthropic().digest());
    let plan = the_plan(desktops_anthropic());
    let sent = serde_json::to_string(&plan).expect("JSON");
    assert!(!sent.contains("claude-haiku"), "no model travels: {sent}");

    let mut plans = Plans::on(Binary {
        version: "0.4.9".into(),
        built_ins: vec![runners],
    });
    let held = plans.take(plan).expect("the same digest");
    assert_eq!(held.background_model(), Some("claude-haiku-runner"));
    let price = held.price_of("claude-haiku-runner").expect("its own price");
    assert_eq!((price.input, price.output), (Some(0.5), Some(2.5)));
}
