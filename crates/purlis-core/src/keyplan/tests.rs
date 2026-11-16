// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn anthropic() -> &'static Declaration {
    crate::model_provider::built_ins()
        .iter()
        .find(|declaration| declaration.id == "anthropic")
        .expect("1a ships the Anthropic built-in")
}

fn a_plan(run: &str) -> KeyProxyPlan {
    plan_for(
        run,
        anthropic(),
        SignInKind::ApiKey,
        vec!["https://api.anthropic.com".into()],
        Some("sha256/AAAA".into()),
        PlanLocks {
            hosts: Some(vec!["api.anthropic.com".into()]),
            never_hosts: vec!["evil.example".into()],
        },
        PlanKey::of("a-stand-in-model-key"),
    )
}

#[test]
fn key_proxy_plan_round_trips_and_refuses_a_reserved_field_or_a_missing_key() {
    // The schema round-trips, every field 1a fills.
    let mut plan = a_plan("01J9RUN0000000000000000001");
    plan.template.insert("region".into(), "us".into());
    let json = serde_json::to_string(&plan).expect("a plan is JSON");
    assert_eq!(arrive(&json), Ok(plan.clone()));

    // Its key never shows in its Debug.
    assert!(!format!("{plan:?}").contains("a-stand-in-model-key"));

    // A reserved field set is refused at arrival, naming it.
    for field in KeyProxyPlan::RESERVED {
        let mut value = serde_json::to_value(&plan).expect("a plan is JSON");
        value[field] = serde_json::json!({ "digest": "00" });
        let refused = arrive(&value.to_string()).expect_err(field);
        assert!(
            matches!(&refused, Refused::Refused(why) if why.contains(field)),
            "{field}: {refused:?}"
        );
    }

    // A plan with no key in the same round trip is refused.
    let mut keyless = plan.clone();
    keyless.key = None;
    let refused = arrive(&serde_json::to_string(&keyless).expect("JSON")).expect_err("no key");
    assert!(
        matches!(&refused, Refused::Refused(why) if why.contains("key")),
        "{refused:?}"
    );
    // And by a runner holding plans, which keeps none of either.
    let mut plans = Plans::on(Binary::this());
    assert!(plans.take(keyless).is_err());
    let mut reserved = plan;
    reserved.confirmations = Some(serde_json::json!([]));
    assert!(plans.take(reserved).is_err());
    assert!(plans.is_empty());
}

#[test]
fn a_plan_with_an_unknown_field_is_refused() {
    let plan = a_plan("01J9RUN0000000000000000002");
    for at in [None, Some("built_in"), Some("locks")] {
        let mut value = serde_json::to_value(&plan).expect("a plan is JSON");
        let object = match at {
            None => &mut value,
            Some(field) => &mut value[field],
        };
        object["models"] = serde_json::json!({ "background": "elsewhere" });
        let refused = arrive(&value.to_string()).expect_err("an unknown field");
        assert!(
            matches!(&refused, Refused::NotTheSchema(why) if why.contains("models")),
            "{at:?}: {refused:?}"
        );
    }
}
