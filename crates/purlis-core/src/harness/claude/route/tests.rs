// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What Claude Code's adapter writes for a key chat (T1a-15): asserted on what the harness is
//! handed, its arguments and its environment, as the chat's start arms it. Every expected value
//! is written out. What Claude Code then does with it is `real_claude_code`'s, in the core's
//! integration tests.

use super::*;
use crate::harness::{Harness, Kit, StateHooks};
use crate::keyroute::Listener;

/// The run's token, as a listener hands it.
const RUN_TOKEN: &str = "run-token-7f3a9c0e1d2b4a6f8e0c";

/// The run's listener's port.
const PORT: u16 = 47_123;

/// The Anthropic built-in's background model, as T1a-07 exposes it.
fn background() -> Option<&'static str> {
    crate::model_provider::built_ins()
        .iter()
        .find(|declaration| declaration.id == "anthropic")
        .expect("the Anthropic built-in ships")
        .background_model()
}

/// A key route to [`PORT`] with [`RUN_TOKEN`], in a chat whose environment holds `env`.
fn route_in(env: &[(&str, &str)]) -> KeyRoute {
    let env: Vec<(String, String)> = env
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    KeyRoute::new(
        Listener {
            port: PORT,
            token: RUN_TOKEN.to_owned(),
        },
        background(),
        move |asked| {
            env.iter()
                .find(|(name, _)| name == asked)
                .map(|(_, value)| value.clone())
        },
    )
}

fn kit(route: Option<&KeyRoute>) -> Kit<'_> {
    Kit {
        binary: std::path::Path::new("/bin/purlis"),
        plugin: Some(std::path::Path::new("/app/plugin")),
        persona: None,
        skills: None,
        graded: false,
        judging: false,
        writes_its_piece: false,
        runs_a_shell: false,
        own: &[],
        laid_out: None,
        key_route: route,
    }
}

/// What Claude Code is handed for a chat on `route`: its arguments and its environment.
fn armed(route: Option<&KeyRoute>) -> (Vec<String>, Vec<(String, String)>) {
    let StateHooks::ThisSessionOnly { args, env, .. } = Harness::ClaudeCode.state_hooks(
        kit(route),
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("Claude Code is armed per session");
    };
    (args, env)
}

/// The one `--settings` value in `args`, as JSON.
fn settings_in(args: &[String]) -> serde_json::Value {
    let at: Vec<usize> = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| *arg == "--settings")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(at.len(), 1, "one --settings flag: {args:?}");
    serde_json::from_str(&args[at[0] + 1]).expect("--settings is JSON")
}

/// The `env` object of `args`' `--settings`.
fn settings_env_of(args: &[String]) -> serde_json::Map<String, serde_json::Value> {
    settings_in(args)["env"]
        .as_object()
        .cloned()
        .expect("--settings carries an env object")
}

/// R-248's list, word for word, with gap C-40's five `ANTHROPIC_GOOGLE_CLOUD_*` members.
const THE_ROW: [&str; 34] = [
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_CUSTOM_HEADERS",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
    "ANTHROPIC_PROFILE",
    "ANTHROPIC_BEDROCK_BASE_URL",
    "ANTHROPIC_VERTEX_BASE_URL",
    "ANTHROPIC_FOUNDRY_BASE_URL",
    "ANTHROPIC_FOUNDRY_API_KEY",
    "ANTHROPIC_FOUNDRY_AUTH_TOKEN",
    "ANTHROPIC_FOUNDRY_RESOURCE",
    "ANTHROPIC_AWS_BASE_URL",
    "ANTHROPIC_BEDROCK_MANTLE_BASE_URL",
    "AWS_BEARER_TOKEN_BEDROCK",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDE_CODE_USE_MANTLE",
    "CLAUDE_CODE_USE_ANTHROPIC_AWS",
    "CLAUDE_CODE_USE_ANTHROPIC_GOOGLE_CLOUD",
    "CLAUDE_CODE_USE_GATEWAY",
    "ANTHROPIC_FEDERATION_RULE_ID",
    "ANTHROPIC_ORGANIZATION_ID",
    "ANTHROPIC_IDENTITY_TOKEN_FILE",
    "ANTHROPIC_BETAS",
    "CLAUDE_CODE_EXTRA_BODY",
    "ANTHROPIC_AWS_API_KEY",
    "ANTHROPIC_AWS_WORKSPACE_ID",
    "ANTHROPIC_GOOGLE_CLOUD_AUTH",
    "ANTHROPIC_GOOGLE_CLOUD_BASE_URL",
    "ANTHROPIC_GOOGLE_CLOUD_LOCATION",
    "ANTHROPIC_GOOGLE_CLOUD_PROJECT",
    "ANTHROPIC_GOOGLE_CLOUD_WORKSPACE_ID",
];

/// The member of the row the adapter sets rather than blanks (H-17).
const EXEMPT: &str = "ANTHROPIC_BASE_URL";

/// AWS's Bedrock runtime endpoint, the row's last member.
const BEDROCK_RUNTIME: &str = "AWS_ENDPOINT_URL_BEDROCK_RUNTIME";

/// The seven provider switches (X8).
const SEVEN_SWITCHES: [&str; 7] = [
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDE_CODE_USE_MANTLE",
    "CLAUDE_CODE_USE_ANTHROPIC_AWS",
    "CLAUDE_CODE_USE_ANTHROPIC_GOOGLE_CLOUD",
    "CLAUDE_CODE_USE_GATEWAY",
];

/// Every model slot Claude Code reads, but the background model's.
const OTHER_MODEL_SLOTS: [&str; 6] = [
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_FABLE_MODEL",
    "ANTHROPIC_DEFAULT_MODEL",
    "ANTHROPIC_SMALL_FAST_MODEL",
];

/// R-232: the generated `--settings` for a key chat, against its pinned expectation; the person's
/// `NO_PROXY` kept with `127.0.0.1` added; the listing limit still in the one `env`.
#[test]
fn adapter_writes_the_route_into_settings_env() {
    let route = route_in(&[("NO_PROXY", "corp.example,.internal")]);
    let (args, _) = armed(Some(&route));

    let env = settings_env_of(&args);

    // The listing limit, as an unproxied chat of the same start has it.
    let mut expected = settings_env_of(&armed(None).0);
    assert_eq!(
        expected.keys().collect::<Vec<_>>(),
        ["SLASH_COMMAND_TOOL_CHAR_BUDGET"]
    );
    for name in THE_ROW
        .iter()
        .chain(std::iter::once(&BEDROCK_RUNTIME))
        .filter(|name| **name != EXEMPT)
    {
        expected.insert((*name).to_owned(), "".into());
    }
    expected.insert(
        "ANTHROPIC_BASE_URL".to_owned(),
        format!("http://127.0.0.1:{PORT}").into(),
    );
    expected.insert(
        "ANTHROPIC_DEFAULT_HAIKU_MODEL".to_owned(),
        background().expect("a background model").into(),
    );
    expected.insert(
        "NO_PROXY".to_owned(),
        "corp.example,.internal,127.0.0.1".into(),
    );
    expected.insert(
        "no_proxy".to_owned(),
        "corp.example,.internal,127.0.0.1".into(),
    );
    assert_eq!(env, expected);
}

/// R-232: a person's bypass that already names the loopback names it once, in both spellings.
#[test]
fn adapter_writes_the_persons_bypass_once() {
    let route = route_in(&[
        ("NO_PROXY", "127.0.0.1"),
        ("no_proxy", "localhost, 127.0.0.1"),
    ]);
    let env = settings_env_of(&armed(Some(&route)).0);

    assert_eq!(env["NO_PROXY"], "127.0.0.1,localhost");
    assert_eq!(env["no_proxy"], "127.0.0.1,localhost");
}

/// R-232 (V3): no `ANTHROPIC_MODEL`, and no model slot but the background model's, in `env`,
/// argv or the environment.
#[test]
fn adapter_never_writes_anthropic_model() {
    let route = route_in(&[]);
    let (args, env) = armed(Some(&route));
    let settings = settings_env_of(&args);

    assert!(settings.contains_key(BACKGROUND_SLOT), "{settings:?}");
    for slot in OTHER_MODEL_SLOTS {
        assert!(!settings.contains_key(slot), "{slot} in --settings env");
        assert!(
            !env.iter().any(|(name, _)| name == slot),
            "{slot} in the environment"
        );
        assert!(
            !args
                .iter()
                .enumerate()
                .filter(|(at, _)| args
                    .get(at.wrapping_sub(1))
                    .is_none_or(|f| f != "--settings"))
                .any(|(_, arg)| arg.contains(slot)),
            "{slot} in argv"
        );
    }
}

/// R-289: every variable the adapter sets for the route is in `--settings` `env`; the token is
/// the one the environment carries, and nothing of the route's is in the environment.
#[test]
fn adapter_writes_every_variable_but_the_token_into_settings_env() {
    let route = route_in(&[]);
    let (args, env) = armed(Some(&route));
    let (_, unrouted) = armed(None);
    let settings = settings_env_of(&args);

    let added: Vec<&String> = env
        .iter()
        .filter(|pair| !unrouted.contains(pair))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(added, [TOKEN], "only the token joins the environment");
    for name in settings_env(&route).keys() {
        assert!(settings.contains_key(name), "{name} not in --settings env");
        assert!(
            !env.iter().any(|(n, _)| n == name),
            "{name} in the environment"
        );
    }
    assert!(settings.contains_key(BASE_URL));
}

/// R-248: core's floor equals R-248's row (with C-40's members), the base URL exempt from
/// blanking; each member is empty in the generated `--settings`, the token's never in it.
#[test]
fn the_floor_list_equals_the_row() {
    let floor: BTreeSet<&str> = STRIP_FLOOR.iter().copied().collect();
    let row: BTreeSet<&str> = THE_ROW
        .iter()
        .copied()
        .chain(std::iter::once(BEDROCK_RUNTIME))
        .collect();
    assert_eq!(floor, row);
    assert_eq!(STRIP_FLOOR.len(), row.len(), "a member listed twice");
    assert!(!floor.contains(TOKEN));

    let route = route_in(&[]);
    let env = settings_env_of(&armed(Some(&route)).0);
    for name in STRIP_FLOOR.iter().filter(|name| **name != EXEMPT) {
        assert_eq!(env.get(*name), Some(&"".into()), "{name}");
    }
    assert_eq!(env[EXEMPT], format!("http://127.0.0.1:{PORT}"));
}

/// R-248 (X8): the key chat's `env` carries `CLAUDE_CODE_USE_GATEWAY` empty.
#[test]
fn the_floor_blanks_claude_code_use_gateway() {
    let route = route_in(&[]);
    let env = settings_env_of(&armed(Some(&route)).0);

    assert_eq!(env.get("CLAUDE_CODE_USE_GATEWAY"), Some(&"".into()));
}

/// R-248 (X8): the blanked list is the seven switches, each in the floor and given a reason, and
/// no name is in both lists.
#[test]
fn the_blanked_switches_are_the_seven_and_each_says_why() {
    let blanked: BTreeSet<&str> = BLANKED_SWITCHES.iter().map(|(name, _)| *name).collect();
    assert_eq!(blanked, SEVEN_SWITCHES.into_iter().collect());
    for (name, why) in BLANKED_SWITCHES.iter().chain(KNOWN_NOT_A_ROUTE) {
        assert!(name.starts_with(SWITCH_PREFIX), "{name}");
        assert!(!why.trim().is_empty(), "{name} says no reason");
    }
    for (name, _) in KNOWN_NOT_A_ROUTE {
        assert!(!blanked.contains(name), "{name} is in both lists");
        assert!(
            !STRIP_FLOOR.contains(name),
            "{name} routes nothing, and is blanked"
        );
    }
}

/// R-248 (X8): the switch guard fails on a made-up switch in a fixture binary, and names it; a
/// name standing inside a longer word is no switch.
#[test]
fn a_switch_in_neither_list_fails_the_guard() {
    let mut binary = b"\0\x01prefix CLAUDE_CODE_USE_BEDROCK\0".to_vec();
    binary.extend(b"i.CLAUDE_CODE_USE_X?1:0;\xff");
    binary.extend(b"XCLAUDE_CODE_USE_HIDDEN ");
    for (name, _) in KNOWN_NOT_A_ROUTE {
        binary.extend(name.as_bytes());
        binary.push(b',');
    }

    assert_eq!(switches_in_neither_list(&binary), ["CLAUDE_CODE_USE_X"]);
    assert!(names_in(&binary, SWITCH_PREFIX).contains("CLAUDE_CODE_USE_BEDROCK"));
    assert!(switches_in_neither_list(b"CLAUDE_CODE_USE_GATEWAY").is_empty());
}

/// R-248 (C-40): the Google Cloud scan fails on a member the floor doesn't list.
#[test]
fn a_google_cloud_variable_outside_the_floor_fails_the_scan() {
    let binary =
        b"ANTHROPIC_GOOGLE_CLOUD_PROJECT\0ANTHROPIC_GOOGLE_CLOUD_REGION\0ANTHROPIC_GOOGLE_CLOUD_AUTH";

    assert_eq!(
        google_cloud_names_not_in_the_floor(binary),
        ["ANTHROPIC_GOOGLE_CLOUD_REGION"]
    );
}

/// R-289: the spawned argv has one `--settings`, whose JSON holds the adapter's existing keys and
/// one `env` object carrying both the listing limit and the route's variables.
#[test]
fn adapter_writes_one_settings_flag_with_one_env_object() {
    let route = route_in(&[]);
    let (args, _) = armed(Some(&route));
    let (unrouted, _) = armed(None);

    let routed = settings_in(&args);
    let plain = settings_in(&unrouted);
    let keys = |settings: &serde_json::Value| {
        settings
            .as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&routed), keys(&plain), "the same keys, env among them");
    for (key, value) in plain.as_object().expect("an object") {
        if key != "env" {
            assert_eq!(&routed[key], value, "{key} changed");
        }
    }
    let env = routed["env"].as_object().expect("one env object");
    assert!(env.contains_key("SLASH_COMMAND_TOOL_CHAR_BUDGET"));
    assert!(env.contains_key(BASE_URL));
    assert_eq!(
        args.len(),
        unrouted.len(),
        "the route adds no flag: {args:?}"
    );
}

/// R-232 (R-293): `env.ANTHROPIC_DEFAULT_HAIKU_MODEL` is the built-in's `[models] background`.
#[test]
fn adapter_writes_the_background_model_into_the_haiku_slot() {
    let route = route_in(&[]);
    let env = settings_env_of(&armed(Some(&route)).0);

    let background = background().expect("the Anthropic built-in names a background model");
    assert_eq!(env.get(BACKGROUND_SLOT), Some(&background.into()));
}

/// R-252: a chat with no route has nothing of it written: a person's own cloud switch in its
/// environment stands, and its `--settings` `env` blanks none.
#[test]
fn adapter_keeps_an_unproxied_chats_cloud_switch() {
    let (args, env) = armed(None);
    let settings = settings_env_of(&args);

    assert!(!settings.contains_key("CLAUDE_CODE_USE_BEDROCK"));
    for name in STRIP_FLOOR.iter().chain(&[TOKEN, BACKGROUND_SLOT]) {
        assert!(!settings.contains_key(*name), "{name} written unproxied");
        assert!(!env.iter().any(|(n, _)| n == name), "{name} set unproxied");
    }
    assert_eq!(
        settings.keys().collect::<Vec<_>>(),
        ["SLASH_COMMAND_TOOL_CHAR_BUDGET"]
    );
}

/// R-252: a proxied chat blanks all seven switches.
#[test]
fn adapter_blanks_the_seven_switches_for_a_proxied_chat() {
    let route = route_in(&[("CLAUDE_CODE_USE_BEDROCK", "1")]);
    let env = settings_env_of(&armed(Some(&route)).0);

    for switch in SEVEN_SWITCHES {
        assert_eq!(env.get(switch), Some(&"".into()), "{switch}");
    }
}

/// R-301: the discovery variable is never written, proxied or not, and is on no list here.
#[test]
fn adapter_never_turns_discovery_on() {
    let route = route_in(&[(DISCOVERY, "1")]);
    for chat in [Some(&route), None] {
        let (args, env) = armed(chat);
        assert!(!settings_env_of(&args).contains_key(DISCOVERY));
        assert!(!env.iter().any(|(name, _)| name == DISCOVERY));
        assert!(!args.iter().any(|arg| arg.contains(DISCOVERY)));
    }
    assert!(!STRIP_FLOOR.contains(&DISCOVERY));
    assert!(!settings_env(&route).contains_key(DISCOVERY));
}

/// R-066, R-256: the token is in the environment as `ANTHROPIC_AUTH_TOKEN`, and nowhere in argv,
/// `--settings` among it.
#[test]
fn adapter_hands_the_token_in_the_environment_alone() {
    let route = route_in(&[]);
    let (args, env) = armed(Some(&route));

    assert!(env.contains(&(TOKEN.to_owned(), RUN_TOKEN.to_owned())));
    assert!(!args.iter().any(|arg| arg.contains(RUN_TOKEN)), "{args:?}");
    assert!(!format!("{route:?}").contains(RUN_TOKEN));
}
