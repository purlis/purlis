// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which chats the key proxy carries (design §3.1, §3.7, §3.8). Every expected answer is written
//! out.

use super::*;
use crate::harness::Harness;
use crate::harness_declaration::{self as declaration, Origin};
use crate::model_provider::Format;

fn claude() -> &'static Declaration {
    declaration::of(Harness::ClaudeCode)
}

#[test]
fn a_proxied_route_at_level_three_is_refused_in_words() {
    let why = resolve(claude(), Way::YourKey, Level::Three).expect_err("level 3 takes no route");

    assert!(why.contains("Claude Code"), "{why}");
    assert!(why.contains("level 3"), "{why}");
    assert!(why.contains("levels 1 and 2"), "{why}");
    for level in [Level::One, Level::Two] {
        assert_eq!(
            resolve(claude(), Way::YourKey, level),
            Ok(Route::Proxied {
                format: Format::AnthropicMessages
            }),
            "{level:?}"
        );
    }
}

#[test]
fn a_built_in_with_no_formats_takes_no_proxied_route() {
    let no_formats = declaration::parse(
        "name = \"shelly\"\nprogram = \"shelly\"\n",
        Origin::BuiltIn,
        "built-in",
    )
    .expect("a built-in with no [route]");

    let why = resolve(&no_formats, Way::YourKey, Level::One).expect_err("no formats, no route");

    assert!(why.contains("shelly"), "{why}");
    assert!(why.contains("its own sign-in"), "{why}");
    // Codex and opencode state none in 1a: each takes its own sign-in only.
    for harness in [Harness::Codex, Harness::Opencode] {
        assert!(
            resolve(declaration::of(harness), Way::YourKey, Level::One).is_err(),
            "{harness:?}"
        );
    }
}

#[test]
fn the_three_unproxied_ways_start_no_listener() {
    for way in [
        Way::Subscription,
        Way::HarnessStoredKey,
        Way::OwnSettingsKey,
    ] {
        for level in [Level::One, Level::Two, Level::Three] {
            let route = resolve(claude(), way, level).expect("an unproxied way is never refused");

            assert_eq!(route, Route::OwnSignIn, "{way:?} at {level:?}");
            assert!(!route.starts_a_listener(), "{way:?} at {level:?}");
            assert_eq!(route.base_url_format(), None, "{way:?} at {level:?}");
        }
    }
}

#[test]
fn a_subscription_profile_is_never_routed_or_shared() {
    for harness in Harness::ALL {
        assert_eq!(
            resolve(declaration::of(harness), Way::Subscription, Level::Two),
            Ok(Route::OwnSignIn),
            "{harness:?}"
        );
    }
    // A subscription names no credential another profile could take up: a `subscription`
    // sign-in that names one is refused.
    let carried = crate::profilesignin::SignIn {
        kind: "subscription".to_owned(),
        label: None,
        credentials: [(
            "CLAUDE_CODE_OAUTH_TOKEN".to_owned(),
            "vault:mine/claude".to_owned(),
        )]
        .into(),
        rest: Default::default(),
    };
    for harness in ["claude", "codex", "opencode"] {
        assert!(carried.check(harness).is_err(), "{harness}");
    }
    // And no other harness's declaration may list the Claude plan as one its sign-in carries.
    let why = declaration::parse(
        "name = \"opencode\"\nprogram = \"opencode\"\n\n[sign_in]\nkinds = [\"subscription\"]\n\
         plans = [{ provider = \"anthropic\", title = \"Claude plan\" }]\n",
        Origin::BuiltIn,
        "built-in",
    )
    .expect_err("the Claude plan in another harness");
    assert!(why.contains("anthropic"), "{why}");
}

/// A provider declaration `id` serving an endpoint in each of `formats`, in that order.
fn a_provider(id: &str, formats: &[&str]) -> crate::model_provider::Declaration {
    let endpoints: String = formats
        .iter()
        .map(|format| {
            format!(
                "\n[[endpoint]]\nformat = \"{format}\"\nbase = \"https://{format}.{id}.example\"\n"
            )
        })
        .collect();
    crate::model_provider::parse(
        &format!("id = \"{id}\"\nname = \"{id}\"\nkind = \"vendor\"\n{endpoints}"),
        &crate::model_provider::From::Yours,
        crate::model_provider::built_ins(),
        &format!("{id}.toml"),
    )
    .expect("a provider declaration")
}

#[test]
fn the_pair_format_is_the_first_of_the_harness_formats_the_provider_serves() {
    let both_ways = declaration::parse(
        "name = \"shelly\"\nprogram = \"shelly\"\n\n\
         [route]\nformats = [\"openai-chat\", \"anthropic-messages\"]\n",
        Origin::BuiltIn,
        "built-in",
    )
    .expect("a built-in speaking both formats");
    let anthropic = crate::model_provider::built_ins()
        .iter()
        .find(|provider| provider.id == "anthropic")
        .expect("the Anthropic built-in");

    assert_eq!(
        pair_format(claude(), anthropic),
        Some(Format::AnthropicMessages)
    );
    assert_eq!(
        pair_format(
            &both_ways,
            &a_provider("duo", &["anthropic-messages", "openai-chat"])
        ),
        Some(Format::OpenaiChat),
        "the harness's order decides, not the provider's"
    );
    assert_eq!(
        pair_format(&both_ways, anthropic),
        Some(Format::AnthropicMessages)
    );
    assert_eq!(
        pair_format(claude(), &a_provider("chatty", &["openai-chat"])),
        None,
        "no format shared"
    );
    assert_eq!(
        pair_format(declaration::of(Harness::Codex), anthropic),
        None,
        "a harness stating no format pairs with nothing"
    );
}
