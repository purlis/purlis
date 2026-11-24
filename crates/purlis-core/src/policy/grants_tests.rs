// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grants and nevers** (M75-20, #7029): a grant is used only at an ask and never lifts a deny; it
//! covers one rule, one action, one resource and the narrowing context it was given under; several
//! asks need a grant each; `@keep` and the scope lock cap the scope offered; a Never holds in every
//! mode, and a nevers store that does not read denies everything.
//!
//! The decisions that need no policy files are built by hand ([`asked`]); the ones that read a
//! `@keep` or a lock, and the ones under the `open` template, go through the real policy.

use std::collections::BTreeSet;

use super::*;
use crate::enforcement::Mode;
use crate::policy::decide::{self, Asker, Deciding, Decision, Outcome, Request, Sealed};
use crate::policy::force::tests::{HEADER, STRICT, commit, host, machine, plane};
use crate::policy::levels::Chat;
use crate::secrets::answers::{self, Entry, Standing, State};

const ULID: &str = "01M85WM43DT07ANNAQ6NQEJTXD";
const OTHER: &str = "01M85WM43DT07ANNAQ6NQEJTXE";

fn base() -> serde_json::Value {
    serde_json::json!({
        "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0
    })
}

fn request<'a>(
    action: &'a str,
    resource: (&'a str, &'a str),
    context: serde_json::Value,
) -> Request<'a> {
    Request {
        chat: "1",
        of: Chat {
            workspace: Some("web"),
            persona: Some("builder"),
        },
        action,
        resource,
        context,
    }
}

fn exec(command: &'static str) -> Request<'static> {
    request("exec", ("Command", command), base())
}

fn connect(port: i64) -> Request<'static> {
    let mut context = base();
    context["port"] = port.into();
    context["via"] = "hook".into();
    request("connect", ("Host", "api.example.com"), context)
}

fn rule(id: &str) -> Deciding {
    Deciding {
        id: id.to_owned(),
        level: "project".to_owned(),
        owner: "this project".to_owned(),
        file: "policy.cedar".to_owned(),
        line: 1,
        off: false,
    }
}

/// A decision of the asks `ids`, as the engine makes it: enforced, asking.
fn asked(ids: &[&str], keep: Option<&str>) -> Decision {
    Decision {
        outcome: Outcome::Ask,
        deciding: ids.iter().map(|id| rule(id)).collect(),
        family: "processes".to_owned(),
        mode_family: "processes".to_owned(),
        mode: Mode::Enforce,
        after_mode: Outcome::Ask,
        lifted: Vec::new(),
        asked: Vec::new(),
        lock: None,
        grant: None,
        unasked: None,
        keep: keep.map(str::to_owned),
        timed: None,
        deny_state: None,
    }
}

fn denied(ids: &[&str]) -> Decision {
    Decision {
        outcome: Outcome::Deny,
        after_mode: Outcome::Deny,
        ..asked(ids, None)
    }
}

/// An ask the `open` template lifted: Cedar said ask, the mode let it through.
fn lifted_by_open(ids: &[&str]) -> Decision {
    Decision {
        mode: Mode::Off,
        after_mode: Outcome::Allow,
        ..asked(ids, None)
    }
}

fn holding(entries: Vec<Entry>) -> Standing {
    Standing {
        state: State::Holds,
        entries,
        ..Standing::fresh()
    }
}

/// The decision after the answers, spending every Once grant it is asked to.
fn after(
    decision: Decision,
    request: &Request<'_>,
    chat: Option<&str>,
    standing: &Standing,
) -> Decision {
    apply(decision, request, chat, standing, &mut |_| true)
}

fn grant(rule: &str, request: &Request<'_>, scope: Scope, chat: Option<&str>) -> Entry {
    Grant::given(rule, request, scope, chat).entry()
}

fn never(request: &Request<'_>, scope: Scope) -> Entry {
    Never::of(request, scope).entry()
}

#[test]
fn a_grant_is_used_only_at_an_ask_and_never_lifts_a_deny() {
    let req = exec("ls");
    let standing = holding(vec![grant("processes-a", &req, Scope::Workspace, None)]);
    // At an ask it serves: the control that makes the refusals below mean something.
    let served = after(asked(&["processes-a"], None), &req, Some(ULID), &standing);
    assert_eq!(served.after_mode, Outcome::Allow, "{served:?}");
    assert!(served.grant.is_some(), "{served:?}");
    assert_eq!(
        served.would_have_blocked(),
        false,
        "a served ask is no would-have-blocked line"
    );
    // A deny stays a deny, though the same rule's grant is sealed.
    let deny = after(denied(&["processes-a"]), &req, Some(ULID), &standing);
    assert_eq!(deny.after_mode, Outcome::Deny, "{deny:?}");
    assert!(deny.grant.is_none(), "{deny:?}");
    // A deny that stands beside an ask is not lifted by the ask's grant either: the plain
    // forbid's own rule has none.
    let both = after(
        denied(&["processes-a", "processes-ban"]),
        &req,
        Some(ULID),
        &standing,
    );
    assert_eq!(both.after_mode, Outcome::Deny, "{both:?}");
    // An allow has no grant to use, and is left as it is.
    let mut allowed = asked(&["processes-a"], None);
    allowed.outcome = Outcome::Allow;
    allowed.after_mode = Outcome::Allow;
    assert_eq!(after(allowed.clone(), &req, Some(ULID), &standing), allowed);
}

#[test]
fn a_grant_stops_counting_when_its_rule_is_removed() {
    let req = exec("ls");
    let kept = grant("processes-a", &req, Scope::Everywhere, None);
    let gone = grant("processes-b", &req, Scope::Everywhere, None);
    let known: BTreeSet<String> = ["processes-a".to_owned()].into();
    // The grant of a rule the policy no longer holds is lost, and only that one.
    assert_eq!(
        lost(&[kept.clone(), gone.clone()], &known),
        vec![gone.clone()]
    );
    assert!(lost(std::slice::from_ref(&kept), &known).is_empty());
    // Anything but a grant is not this function's to take.
    let never = never(&req, Scope::Everywhere);
    assert!(lost(std::slice::from_ref(&never), &BTreeSet::new()).is_empty());
    // While it is there it serves; the rule's id coming back is no reason for it to.
    let standing = holding(vec![kept, gone]);
    let served = after(asked(&["processes-b"], None), &req, Some(ULID), &standing);
    assert_eq!(served.after_mode, Outcome::Allow, "{served:?}");
    let afterwards = holding(Vec::new());
    let again = after(asked(&["processes-b"], None), &req, Some(ULID), &afterwards);
    assert_eq!(again.after_mode, Outcome::Ask, "{again:?}");
}

#[test]
fn a_grant_does_not_match_a_different_narrowing_context() {
    let given = connect(443);
    let standing = holding(vec![grant("network-a", &given, Scope::Workspace, None)]);
    let same = after(asked(&["network-a"], None), &given, Some(ULID), &standing);
    assert_eq!(same.after_mode, Outcome::Allow, "{same:?}");
    // The port differs: not the context it was given under.
    let other = connect(80);
    let said = after(asked(&["network-a"], None), &other, Some(ULID), &standing);
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    // Neither do another action, another resource, nor another rule.
    let mut elsewhere = connect(443);
    elsewhere.resource = ("Host", "other.example.com");
    let said = after(asked(&["network-a"], None), &elsewhere, Some(ULID), &standing);
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    let said = after(asked(&["network-b"], None), &given, Some(ULID), &standing);
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    let mut dial = connect(443);
    dial.action = "dial";
    let said = after(asked(&["network-a"], None), &dial, Some(ULID), &standing);
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    // A context field the grant never saw is a different context too.
    let mut more = connect(443);
    more.context["via"] = "browser".into();
    let said = after(asked(&["network-a"], None), &more, Some(ULID), &standing);
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
}

#[test]
fn a_grant_given_under_force_false_does_not_serve_a_force_true_push() {
    let mut calm = base();
    calm["force"] = false.into();
    calm["skip-hooks"] = false.into();
    calm["brokered"] = true.into();
    calm["amend"] = false.into();
    let mut forced = calm.clone();
    forced["force"] = true.into();
    let given = request("push", ("Branch", "acme/web#main"), calm);
    let force = request("push", ("Branch", "acme/web#main"), forced);
    let standing = holding(vec![grant(
        "guards-git-protected-push",
        &given,
        Scope::Everywhere,
        None,
    )]);
    let served = after(
        asked(&["guards-git-protected-push"], None),
        &given,
        Some(ULID),
        &standing,
    );
    assert_eq!(served.after_mode, Outcome::Allow, "{served:?}");
    let said = after(
        asked(&["guards-git-protected-push"], None),
        &force,
        Some(ULID),
        &standing,
    );
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
}

#[test]
fn two_deciding_asks_each_need_their_own_grant() {
    let req = exec("ls");
    let a = grant("processes-a", &req, Scope::Workspace, None);
    let b = grant("processes-b", &req, Scope::Workspace, None);
    let both = asked(&["processes-a", "processes-b"], None);
    let one = after(both.clone(), &req, Some(ULID), &holding(vec![a.clone()]));
    assert_eq!(one.after_mode, Outcome::Ask, "{one:?}");
    let other = after(both.clone(), &req, Some(ULID), &holding(vec![b.clone()]));
    assert_eq!(other.after_mode, Outcome::Ask, "{other:?}");
    let each = after(both, &req, Some(ULID), &holding(vec![a, b]));
    assert_eq!(each.after_mode, Outcome::Allow, "{each:?}");
}

#[test]
fn the_scope_offered_is_the_narrowest_keep_among_the_deciding_asks() {
    let m = machine();
    let rules = format!(
        "{HEADER}\
         @id(\"processes-a\") @ask(\"you\") @keep(\"everywhere\")\n\
         forbid (principal, action == Action::\"exec\", resource);\n\
         @id(\"processes-b\") @ask(\"you\") @keep(\"this chat\")\n\
         forbid (principal, action == Action::\"exec\", resource);\n"
    );
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &rules)]);
    let said = host(&root, &exec("ls"));
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    assert_eq!(said.keep.as_deref(), Some("this chat"), "{said:?}");
    assert_eq!(offer(&said).scopes, [Scope::Once, Scope::Chat]);
    // Where neither writes one, the widest is offered. (A fresh plane: a `@keep` widened in a
    // later commit offers only the accepted scope until the press.)
    let m = machine();
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            (
                "policy.cedar",
                &format!(
                    "{HEADER}@id(\"processes-a\") @ask(\"you\")\n\
                     forbid (principal, action == Action::\"exec\", resource);\n"
                ),
            ),
        ],
    );
    let said = host(&root, &exec("ls"));
    assert_eq!(said.keep, None, "{said:?}");
    assert_eq!(
        offer(&said).scopes,
        [Scope::Once, Scope::Chat, Scope::Workspace, Scope::Everywhere]
    );
}

#[test]
fn keep_once_offers_no_wider_scope() {
    let said = asked(&["processes-a"], Some("once"));
    let offered = offer(&said);
    assert_eq!(offered.scopes, [Scope::Once]);
    // Don't allow and Never are offered at every ask, a once-only one too.
    assert!(offered.dont_allow);
    assert_eq!(offered.never, [Scope::Workspace, Scope::Everywhere]);
    // A grant wider than the cap now in force counts for nothing: a tightening applies at once.
    let req = exec("ls");
    let wide = holding(vec![grant("processes-a", &req, Scope::Everywhere, None)]);
    let once = after(said.clone(), &req, Some(ULID), &wide);
    assert_eq!(once.after_mode, Outcome::Ask, "{once:?}");
    let within = holding(vec![grant("processes-a", &req, Scope::Once, Some(ULID))]);
    let served = after(said, &req, Some(ULID), &within);
    assert_eq!(served.after_mode, Outcome::Allow, "{served:?}");
}

#[test]
fn a_project_lock_on_the_widest_scope_caps_every_ask() {
    let m = machine();
    let rules = format!(
        "{HEADER}\
         @id(\"processes-a\") @ask(\"you\") @keep(\"everywhere\")\n\
         forbid (principal, action == Action::\"exec\", resource) when {{ resource == Command::\"ls\" }};\n\
         @id(\"processes-b\") @ask(\"you\")\n\
         forbid (principal, action == Action::\"exec\", resource) when {{ resource == Command::\"cat\" }};\n"
    );
    let locked = format!("{STRICT}[policy.locks]\nkeep = \"this workspace\"\n");
    let root = plane(&m, &[("purlis.toml", &locked), ("policy.cedar", &rules)]);
    for command in ["ls", "cat"] {
        let said = host(&root, &exec(command));
        assert_eq!(said.after_mode, Outcome::Ask, "{command}: {said:?}");
        assert_eq!(
            said.keep.as_deref(),
            Some("this workspace"),
            "{command}: {said:?}"
        );
        assert_eq!(
            offer(&said).scopes,
            [Scope::Once, Scope::Chat, Scope::Workspace],
            "{command}"
        );
    }
    // A rule's own narrower `@keep` still wins over the lock.
    commit(
        &root,
        &[(
            "policy.cedar",
            &format!(
                "{HEADER}@id(\"processes-a\") @ask(\"you\") @keep(\"this chat\")\n\
                 forbid (principal, action == Action::\"exec\", resource);\n"
            ),
        )],
    );
    let said = host(&root, &exec("ls"));
    assert_eq!(said.keep.as_deref(), Some("this chat"), "{said:?}");
}

fn asker() -> Asker<'static> {
    Asker {
        chat: 1,
        of: Chat {
            workspace: Some("web"),
            persona: Some("builder"),
        },
        sandbox: "floor",
        attended: true,
        harness_mode: "default",
        depth: 0,
        via: "hook",
        start: None,
    }
}

/// A plane under the `open` template, and the app's view of it for chat 1.
fn open_plane_and_asker() -> (
    crate::policy::force::tests::Machine,
    std::path::PathBuf,
    Asker<'static>,
) {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", "[policy]\ntemplate = \"open\"\n")]);
    (m, root, asker())
}

#[test]
fn a_never_holds_under_the_open_template() {
    let (_m, root, asker) = open_plane_and_asker();
    let merge = request("merge", ("Branch", "acme/web#main"), {
        let mut context = base();
        context["via"] = "hook".into();
        context
    });
    let sealed_with = |entries: Vec<Entry>| Sealed {
        standing: holding(entries),
        machine: None,
        chat: Some(ULID.to_owned()),
    };
    let ask = |sealed: &Sealed, resource| {
        decide::in_plane(&root, &asker, "merge", ("Branch", resource), sealed)
    };
    // Under `open` the merge hold is off: the action goes through.
    let free = ask(&sealed_with(Vec::new()), "acme/web#main");
    assert_eq!(free.after_mode, Outcome::Allow, "{free:?}");
    // A This-workspace never on it holds all the same, and says it is the person's.
    let held = ask(
        &sealed_with(vec![never(&merge, Scope::Workspace)]),
        "acme/web#main",
    );
    assert_eq!(held.after_mode, Outcome::Deny, "{held:?}");
    assert_eq!(held.deciding[0].level, "you", "{held:?}");
    // Not another resource.
    let other = ask(
        &sealed_with(vec![never(&merge, Scope::Workspace)]),
        "acme/web#dev",
    );
    assert_eq!(other.after_mode, Outcome::Allow, "{other:?}");
    // A This-workspace never is that workspace's: another workspace's chat is not held by it.
    let mut theirs = Never::of(&merge, Scope::Workspace);
    theirs.workspace = Some("api".to_owned());
    let elsewhere = theirs.entry();
    let other = ask(&sealed_with(vec![elsewhere]), "acme/web#main");
    assert_eq!(other.after_mode, Outcome::Allow, "{other:?}");
    // An Everywhere never lives in the machine's own entry, and holds in every workspace.
    let mut standing = holding(Vec::new());
    standing.everywhere = vec![never(&merge, Scope::Everywhere)];
    let sealed = Sealed {
        standing,
        machine: None,
        chat: Some(ULID.to_owned()),
    };
    let everywhere = ask(&sealed, "acme/web#main");
    assert_eq!(everywhere.after_mode, Outcome::Deny, "{everywhere:?}");
    // And a mode that lifted an ask does not lift a never (decision built by hand).
    let lifted = after(
        lifted_by_open(&["guards-merge-hold"]),
        &merge,
        Some(ULID),
        &holding(vec![never(&merge, Scope::Workspace)]),
    );
    assert_eq!(lifted.after_mode, Outcome::Deny, "{lifted:?}");
}

#[test]
fn an_unreadable_nevers_store_denies_in_every_mode() {
    let (_m, root, asker) = open_plane_and_asker();
    let req = exec("ls");
    // A never of a shape this build cannot read: what it forbids is unknown.
    let broken = Entry::new(answers::NEVER, "{\"v\":99,\"what\":\"unknown\"}");
    let garbage = Entry::new(answers::NEVER, "not a never at all");
    for entries in [vec![broken.clone()], vec![garbage]] {
        let standing = holding(entries);
        // Even an allow by a permit, even an ask the mode lifted.
        let mut allow = asked(&[], None);
        allow.outcome = Outcome::Allow;
        allow.after_mode = Outcome::Allow;
        for decision in [allow, lifted_by_open(&["processes-a"])] {
            let said = after(decision, &req, Some(ULID), &standing);
            assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
            assert_eq!(said.deciding[0].level, "you", "{said:?}");
        }
    }
    // The Everywhere entry too.
    let mut standing = holding(Vec::new());
    standing.everywhere = vec![broken.clone()];
    let said = after(lifted_by_open(&["processes-a"]), &req, Some(ULID), &standing);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    // The nevers an earlier build kept (`asking -> target`) read fine and are not this store's
    // concern: they do not deny an unrelated action.
    let old = Entry::new(answers::NEVER, "qa -> prod");
    let fine = after(lifted_by_open(&["processes-a"]), &req, Some(ULID), &holding(vec![old]));
    assert_eq!(fine.after_mode, Outcome::Allow, "{fine:?}");
    // End to end, under `open`: the host's own decision denies.
    let sealed = Sealed {
        standing: holding(vec![broken]),
        machine: None,
        chat: Some(ULID.to_owned()),
    };
    let said = decide::in_plane(&root, &asker, "exec", ("Command", "ls"), &sealed);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}

#[test]
fn a_this_chat_grant_is_keyed_on_the_chats_ulid() {
    let req = exec("ls");
    let standing = holding(vec![grant(
        "processes-a",
        &req,
        Scope::Chat,
        Some(ULID),
    )]);
    let said = asked(&["processes-a"], None);
    let mine = after(said.clone(), &req, Some(ULID), &standing);
    assert_eq!(mine.after_mode, Outcome::Allow, "{mine:?}");
    // Another chat of the same workspace, a chat with no ULID, and a grant with none: no.
    let other = after(said.clone(), &req, Some(OTHER), &standing);
    assert_eq!(other.after_mode, Outcome::Ask, "{other:?}");
    let nobody = after(said.clone(), &req, None, &standing);
    assert_eq!(nobody.after_mode, Outcome::Ask, "{nobody:?}");
    let keyless = holding(vec![grant("processes-a", &req, Scope::Chat, None)]);
    let said_keyless = after(said, &req, Some(ULID), &keyless);
    assert_eq!(said_keyless.after_mode, Outcome::Ask, "{said_keyless:?}");
}

#[test]
fn a_once_grant_is_spent_by_the_one_request_it_serves() {
    let req = exec("ls");
    let standing = holding(vec![grant("processes-a", &req, Scope::Once, Some(ULID))]);
    let mut spent = Vec::new();
    let served = apply(
        asked(&["processes-a"], None),
        &req,
        Some(ULID),
        &standing,
        &mut |text| {
            spent.push(text.to_owned());
            true
        },
    );
    assert_eq!(served.after_mode, Outcome::Allow, "{served:?}");
    assert_eq!(spent.len(), 1, "{spent:?}");
    // A use that cannot be recorded is a refusal: the ask stays an ask.
    let refused = apply(
        asked(&["processes-a"], None),
        &req,
        Some(ULID),
        &standing,
        &mut |_| false,
    );
    assert_eq!(refused.after_mode, Outcome::Ask, "{refused:?}");
}

#[allow(clippy::disallowed_methods, reason = "a test plays the person's press")]
fn press() -> answers::Press {
    answers::Press::of_the_persons_press()
}

#[test]
fn a_removed_and_then_re_added_rule_does_not_revive_an_everywhere_grant() {
    let m = machine();
    let with_rule = format!(
        "{HEADER}@id(\"processes-a\") @ask(\"you\") @keep(\"everywhere\")\n\
         forbid (principal, action == Action::\"exec\", resource);\n"
    );
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &with_rule)]);
    answers::register(&root).expect("registered");
    let req = exec("ls");
    let given = grant("processes-a", &req, Scope::Everywhere, None);
    answers::commit(&root, |draft| draft.add(&press(), answers::GRANT, &given.text))
        .expect("sealed");
    let ask = || {
        let sealed = Sealed {
            chat: Some(ULID.to_owned()),
            ..Sealed::of(&root)
        };
        decide::in_plane(&root, &asker(), "exec", ("Command", "ls"), &sealed)
    };
    let holds_a_grant = || {
        answers::standing(&root)
            .entries
            .iter()
            .any(|entry| entry.kind == answers::GRANT)
    };
    // While the rule decides, the grant serves.
    assert_eq!(ask().after_mode, Outcome::Allow);
    // The rule is removed from the project, but the accepted floor still holds the version that
    // had it, so it still decides and its grant stays.
    commit(&root, &[("policy.cedar", HEADER)]);
    assert_eq!(ask().after_mode, Outcome::Allow);
    assert!(holds_a_grant());
    // The person's press starts the floor again: no side holds the rule, and its grant is taken
    // out of the store at the next decision.
    let kept: Vec<Entry> = answers::standing(&root)
        .entries
        .into_iter()
        .filter(|entry| entry.kind != answers::FLOOR)
        .collect();
    answers::recommit(&root, &press(), &kept).expect("pressed");
    assert!(holds_a_grant(), "the press keeps the person's grants");
    ask();
    assert!(!holds_a_grant(), "a grant of a rule in no side is lost");
    // The rule written again does not find it: the ask asks.
    commit(&root, &[("policy.cedar", &with_rule)]);
    let again = ask();
    assert_eq!(again.after_mode, Outcome::Ask, "{again:?}");
    assert!(again.grant.is_none(), "{again:?}");
}
