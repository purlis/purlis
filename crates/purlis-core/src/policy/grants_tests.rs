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
use crate::policy::force::judge;
use crate::policy::force::tests::{
    HEADER, STRICT, accepted as accepted_entry, commit, host, machine, plane, sources, version,
};
use crate::policy::levels::Chat;
use crate::secrets::answers::{self, Deny, Entry, Standing, State};

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
        lifted_scopes: None,
        asked_scopes: None,
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
    assert!(
        !served.would_have_blocked(),
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
    let said = after(
        asked(&["network-a"], None),
        &elsewhere,
        Some(ULID),
        &standing,
    );
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
        [
            Scope::Once,
            Scope::Chat,
            Scope::Workspace,
            Scope::Everywhere
        ]
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
    let said = after(
        lifted_by_open(&["processes-a"]),
        &req,
        Some(ULID),
        &standing,
    );
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    // The nevers an earlier build kept (`asking -> target`) read fine and are not this store's
    // concern: they do not deny an unrelated action.
    let old = Entry::new(answers::NEVER, "qa -> prod");
    let fine = after(
        lifted_by_open(&["processes-a"]),
        &req,
        Some(ULID),
        &holding(vec![old]),
    );
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
    let standing = holding(vec![grant("processes-a", &req, Scope::Chat, Some(ULID))]);
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
        &mut |texts| {
            spent.extend(texts.iter().map(|text| (*text).to_owned()));
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
    answers::commit(&root, |draft| {
        draft.add(&press(), answers::GRANT, &given.text)
    })
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

// ---- review round 2 ---------------------------------------------------------------------------

/// An ask on `exec`, by `id`, where `when` holds.
fn exec_ask(id: &str, when: &str) -> String {
    format!(
        "@id(\"{id}\") @ask(\"you\")\nforbid (principal, action == Action::\"exec\", resource){when};\n"
    )
}

#[test]
fn an_ask_the_accepted_copy_holds_and_a_commit_dropped_still_needs_its_own_grant() {
    // F1: HEAD asks by X alone, the accepted copy by X and Y (Y where nobody is at the chat). The
    // tie picks HEAD's side, and X's grant must not serve what Y still asks.
    let m = machine();
    let x = exec_ask("processes-x", "");
    let y = exec_ask("processes-y", " when { context.attended == false }");
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &format!("{HEADER}{x}")),
        ],
    );
    let accepted = accepted_entry(&version(STRICT, Some(&format!("{HEADER}{x}{y}"))));
    let mut context = base();
    context["attended"] = false.into();
    let req = request("exec", ("Command", "ls"), context);
    let judged = |entries: Vec<Entry>| {
        let standing = holding(entries);
        let decision = judge(&root, &sources(&root), &standing, &req, None).0;
        apply(decision, &req, Some(ULID), &standing, &mut |_| true)
    };
    let gx = grant("processes-x", &req, Scope::Workspace, None);
    let gy = grant("processes-y", &req, Scope::Workspace, None);
    let only_x = judged(vec![accepted.clone(), gx.clone()]);
    assert_eq!(only_x.after_mode, Outcome::Ask, "{only_x:?}");
    assert!(only_x.grant.is_none(), "{only_x:?}");
    let mut ids = only_x.ids();
    ids.sort_unstable();
    assert_eq!(ids, ["processes-x", "processes-y"], "{only_x:?}");
    // Control: a grant for each of them serves.
    let both = judged(vec![accepted, gx, gy]);
    assert_eq!(both.after_mode, Outcome::Allow, "{both:?}");
}

#[test]
fn a_scope_lock_caps_an_ask_a_family_mode_left_enforced_beside_a_lifted_plain_forbid() {
    // F2: the family logs, so the plain forbid P is lifted; the ask A is enforced by its own
    // entry. Cedar's outcome is a deny and what is enforced is an ask, and the lock must cap it.
    let m = machine();
    let rules = format!(
        "{HEADER}@id(\"processes-p\")\nforbid (principal, action == Action::\"exec\", resource);\n{}",
        exec_ask("processes-a", "")
    );
    let settings = "[policy]\ntemplate = \"strict\"\nprocesses = \"log\"\n\n\
         [policy.rules]\n\"processes-a\" = \"enforce\"\n\n[policy.locks]\nkeep = \"once\"\n";
    let root = plane(&m, &[("purlis.toml", settings), ("policy.cedar", &rules)]);
    let req = exec("ls");
    let standing = holding(vec![grant("processes-a", &req, Scope::Everywhere, None)]);
    let decision = judge(&root, &sources(&root), &standing, &req, None).0;
    assert_eq!(
        (decision.outcome, decision.after_mode),
        (Outcome::Deny, Outcome::Ask),
        "{decision:?}"
    );
    assert_eq!(decision.keep.as_deref(), Some("once"), "{decision:?}");
    let said = apply(decision, &req, Some(ULID), &standing, &mut |_| true);
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
}

#[test]
fn a_never_on_a_start_holds_where_the_deny_state_set_the_stores_answers_aside() {
    // F6: past the floor's cap the verified entries are in `kept` and `entries` is empty.
    let start = request("start", ("Profile", "claude"), base());
    let denied = Standing {
        state: State::Deny(vec![Deny::PastTheFloorCap]),
        kept: vec![never(&start, Scope::Workspace)],
        ..Standing::fresh()
    };
    let said = after(
        lifted_by_open(&["processes-a"]),
        &start,
        Some(ULID),
        &denied,
    );
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.deciding[0].level, "you", "{said:?}");
    // A kept never that does not read denies too.
    let broken = Standing {
        kept: vec![Entry::new(answers::NEVER, "{\"v\":99}")],
        ..denied
    };
    let said = after(
        lifted_by_open(&["processes-a"]),
        &start,
        Some(ULID),
        &broken,
    );
    assert_eq!(said.deciding[0].id, UNREADABLE_ID, "{said:?}");
}

#[test]
fn an_unreadable_everywhere_store_denies_a_start() {
    // F6: its nevers are emptied from the standing, which must not read as "no nevers".
    let start = request("start", ("Profile", "claude"), base());
    let denied = Standing {
        state: State::Deny(vec![Deny::PastTheFloorCap]),
        everywhere_unreadable: true,
        ..Standing::fresh()
    };
    let said = after(
        lifted_by_open(&["processes-a"]),
        &start,
        Some(ULID),
        &denied,
    );
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.deciding[0].id, UNREADABLE_ID, "{said:?}");
}

/// A plane of two asks on `exec`, registered, and the grants in the real store.
fn two_asks(m: &crate::policy::force::tests::Machine) -> std::path::PathBuf {
    let rules = format!(
        "{HEADER}{}{}",
        exec_ask("processes-a", ""),
        exec_ask("processes-b", "")
    );
    let root = plane(m, &[("purlis.toml", STRICT), ("policy.cedar", &rules)]);
    answers::register(&root).expect("registered");
    root
}

#[test]
fn two_once_grants_are_spent_together_or_not_at_all() {
    // F7: the store holds the Once grant of A and the standing the host read holds A's and B's
    // (B's was spent by another decision a moment ago). A's must not be lost to an ask.
    let m = machine();
    let root = two_asks(&m);
    let req = exec("ls");
    let ga = grant("processes-a", &req, Scope::Once, Some(ULID));
    let gb = grant("processes-b", &req, Scope::Once, Some(ULID));
    answers::commit(&root, |draft| draft.add(&press(), answers::GRANT, &ga.text)).expect("sealed");
    let sealed = Sealed {
        standing: holding(vec![ga.clone(), gb]),
        machine: None,
        chat: Some(ULID.to_owned()),
    };
    let said = decide::in_plane(&root, &asker(), "exec", ("Command", "ls"), &sealed);
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    let kept = answers::standing(&root);
    assert!(
        kept.entries.iter().any(|entry| entry.text == ga.text),
        "A's grant was lost though the ask was refused: {:?}",
        kept.entries
    );
}

#[test]
fn a_once_grant_serves_only_one_of_two_decisions_that_read_the_store_before_either_spent() {
    // The race of two decisions: the real `use_once_grant` lets one through.
    let m = machine();
    let root = two_asks(&m);
    let req = exec("ls");
    let once_a = grant("processes-a", &req, Scope::Once, Some(ULID));
    let once_b = grant("processes-b", &req, Scope::Once, Some(ULID));
    answers::commit(&root, |draft| {
        draft.add(&press(), answers::GRANT, &once_a.text);
        draft.add(&press(), answers::GRANT, &once_b.text);
    })
    .expect("sealed");
    let read = || Sealed {
        chat: Some(ULID.to_owned()),
        ..Sealed::of(&root)
    };
    let (first, second) = (read(), read());
    let a = decide::in_plane(&root, &asker(), "exec", ("Command", "ls"), &first);
    let b = decide::in_plane(&root, &asker(), "exec", ("Command", "ls"), &second);
    assert_eq!(a.after_mode, Outcome::Allow, "{a:?}");
    assert_eq!(b.after_mode, Outcome::Ask, "{b:?}");
    assert!(
        answers::standing(&root)
            .entries
            .iter()
            .all(|entry| entry.kind != answers::GRANT),
        "both were spent by the one decision"
    );
}

#[test]
fn a_grant_given_for_the_merge_hold_serves_a_chat_the_floor_enforces_it_for() {
    // F5 (b) and (c): the floor enforces `guards.merge`; the grant is applied after the floor, so
    // it neither is undone by it nor spent for nothing.
    use crate::enforcement::Resolved;
    use crate::policy::decide::ChatFacts;
    let facts = ChatFacts {
        workspace: Some("web".to_owned()),
        persona: None,
        sandboxed: false,
        level: None,
        attended: true,
        harness_mode: None,
    };
    let floor = Resolved::parse("guards.merge=enforce");
    let mut context = base();
    context["via"] = "hook".into();
    let merge = request("merge", ("Branch", "acme/web#main"), context);
    for scope in [Scope::Once, Scope::Workspace, Scope::Chat] {
        let m = machine();
        let root = plane(&m, &[("purlis.toml", STRICT)]);
        answers::register(&root).expect("registered");
        let given = grant("guards-merge-hold", &merge, scope, Some(ULID));
        answers::commit(&root, |draft| {
            draft.add(&press(), answers::GRANT, &given.text)
        })
        .expect("sealed");
        let sealed = Sealed {
            chat: Some(ULID.to_owned()),
            ..Sealed::of(&root)
        };
        let said = decide::for_chat(
            &root,
            4,
            Some(&facts),
            &floor,
            &sealed,
            "merge",
            ("Branch", "acme/web#main"),
        )
        .expect("a chat that is open");
        // The shipped hold is `@keep("once")`: only a Once grant serves, and it is spent by
        // the decision it serves; a wider grant is no answer and stays where it is.
        let served = scope == Scope::Once;
        let expected = if served { Outcome::Allow } else { Outcome::Ask };
        assert_eq!(said.after_mode, expected, "{scope:?}: {said:?}");
        assert_eq!(said.grant.is_some(), served, "{scope:?}: {said:?}");
        let held = answers::standing(&root)
            .entries
            .iter()
            .any(|entry| entry.text == given.text);
        assert_eq!(
            held, !served,
            "{scope:?}: only a served Once grant is spent"
        );
    }
}

#[test]
fn a_grant_of_a_rule_an_uncommitted_edit_took_from_the_tree_is_not_taken_out_of_the_store() {
    // F8: the purge reads the versions a decision reads. A workspace folder removed from the tree
    // (uncommitted) breaks the tree's reading of the project, while HEAD still holds the rule.
    use crate::policy::force::in_force;
    let m = machine();
    let rules = format!("{HEADER}{}", exec_ask("processes-a", ""));
    let tables = "[policy]\ntemplate = \"strict\"\n[policy.workspace.web]\n";
    let root = plane(
        &m,
        &[
            ("purlis.toml", tables),
            ("policy.cedar", &rules),
            ("workspaces/web/README.md", "web\n"),
        ],
    );
    answers::register(&root).expect("registered");
    let req = exec("ls");
    let given = grant("processes-a", &req, Scope::Everywhere, None);
    answers::commit(&root, |draft| {
        draft.add(&press(), answers::GRANT, &given.text)
    })
    .expect("sealed");
    std::fs::remove_dir_all(root.join("workspaces/web")).expect("removed, uncommitted");
    let no_workspaces: [&str; 0] = [];
    let sources = crate::policy::levels::Sources {
        root: &root,
        workspaces: &no_workspaces,
        personas: &no_workspaces,
        organisation: None,
        accepted: None,
    };
    in_force(&root, &sources, &answers::standing(&root), &req, None);
    assert!(
        answers::standing(&root)
            .entries
            .iter()
            .any(|entry| entry.text == given.text),
        "HEAD still holds the rule, so its grant stays"
    );
}

// ---- review round 3 ---------------------------------------------------------------------------

fn merge_asked_of_a_hook() -> Request<'static> {
    let mut context = base();
    context["via"] = "hook".into();
    request("merge", ("Branch", "acme/web#main"), context)
}

fn a_chat() -> decide::ChatFacts {
    decide::ChatFacts {
        workspace: Some("web".to_owned()),
        persona: None,
        sandboxed: false,
        level: None,
        attended: true,
        harness_mode: None,
    }
}

/// A merge hold set `off`, as a project writes it.
const HOLD_OFF: &str =
    "[policy]\ntemplate = \"strict\"\n\n[policy.rules]\n\"guards-merge-hold\" = \"off\"\n";

fn for_the_merge(
    root: &std::path::Path,
    floor: &crate::enforcement::Resolved,
    sealed: &Sealed,
) -> Decision {
    decide::for_chat(
        root,
        4,
        Some(&a_chat()),
        floor,
        sealed,
        "merge",
        ("Branch", "acme/web#main"),
    )
    .expect("a chat that is open")
}

#[test]
fn a_grant_for_the_projects_own_merge_ask_does_not_serve_while_the_floor_holds_the_hold() {
    // N1: the project sets the hold off and asks by its own rule X, both accepted, and the floor
    // enforces the merge family: the decision is Ask[X], and the shipped hold the floor keeps
    // must be asked beside it, so X's grant alone never lets the merge through.
    use crate::enforcement::Resolved;
    let m = machine();
    let x = "@id(\"guards-merge-x\") @ask(\"you\") @keep(\"everywhere\")\n\
             forbid (principal, action == Action::\"merge\", resource);\n";
    let rules = format!("{HEADER}{x}");
    let root = plane(&m, &[("purlis.toml", HOLD_OFF), ("policy.cedar", &rules)]);
    let merge = merge_asked_of_a_hook();
    let accepted = accepted_entry(&version(HOLD_OFF, Some(&rules)));
    let floor = Resolved::parse("guards.merge=enforce");
    let given_x = grant("guards-merge-x", &merge, Scope::Everywhere, None);
    let sealed = |entries: Vec<Entry>| Sealed {
        standing: holding(entries),
        machine: None,
        chat: Some(ULID.to_owned()),
    };
    let said = for_the_merge(
        &root,
        &floor,
        &sealed(vec![accepted.clone(), given_x.clone()]),
    );
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    assert!(
        said.grant.is_none(),
        "X's grant is not the hold's: {said:?}"
    );
    let mut ids = said.ids();
    ids.sort_unstable();
    assert_eq!(ids, ["guards-merge-hold", "guards-merge-x"], "{said:?}");
    // The scope offered is the narrowest of the two: the hold's once.
    assert_eq!(said.keep.as_deref(), Some("once"), "{said:?}");
    // Control: a floor that does not enforce the merge family asks only by X, whose grant serves.
    let loose = Resolved::parse("guards.merge=off");
    let served = for_the_merge(&root, &loose, &sealed(vec![accepted, given_x]));
    assert_eq!(served.after_mode, Outcome::Allow, "{served:?}");
}

/// A plane whose project, and the accepted copy, have the merge hold off, with the app's store
/// registered and `entries` committed to it.
fn hold_off_everywhere(
    m: &crate::policy::force::tests::Machine,
    entries: &[&Entry],
) -> std::path::PathBuf {
    let root = plane(m, &[("purlis.toml", HOLD_OFF)]);
    answers::register(&root).expect("registered");
    let accepted = accepted_entry(&version(HOLD_OFF, None));
    answers::commit(&root, |draft| {
        draft.add(&press(), answers::ACCEPTED, &accepted.text);
        for entry in entries {
            draft.add(&press(), &entry.kind, &entry.text);
        }
    })
    .expect("sealed");
    root
}

#[test]
fn a_once_grant_for_the_shipped_hold_serves_where_every_side_has_it_off_and_the_floor_enforces_it()
{
    // N1 (the override with a grant) and N5: the hold the floor keeps is the shipped rule, which
    // no side holds, so its grant must not be purged before it is spent.
    use crate::enforcement::Resolved;
    let m = machine();
    let merge = merge_asked_of_a_hook();
    let given = grant("guards-merge-hold", &merge, Scope::Once, Some(ULID));
    let root = hold_off_everywhere(&m, &[&given]);
    let sealed = Sealed {
        chat: Some(ULID.to_owned()),
        ..Sealed::of(&root)
    };
    let said = for_the_merge(&root, &Resolved::parse("guards.merge=enforce"), &sealed);
    assert_eq!(said.after_mode, Outcome::Allow, "{said:?}");
    assert!(said.grant.is_some(), "{said:?}");
    assert!(
        answers::standing(&root)
            .entries
            .iter()
            .all(|e| e.kind != answers::GRANT),
        "the Once grant was spent by the decision it served"
    );
}

#[test]
fn a_grant_for_the_shipped_hold_is_kept_only_while_the_floor_enforces_the_hold() {
    use crate::enforcement::Resolved;
    let merge = merge_asked_of_a_hook();
    // A This-workspace grant is wider than the hold's once, so it serves nothing and stays.
    let wide = grant("guards-merge-hold", &merge, Scope::Workspace, None);
    for (floor, kept) in [("guards.merge=enforce", true), ("guards.merge=off", false)] {
        let m = machine();
        let root = hold_off_everywhere(&m, &[&wide]);
        let sealed = Sealed {
            chat: Some(ULID.to_owned()),
            ..Sealed::of(&root)
        };
        for_the_merge(&root, &Resolved::parse(floor), &sealed);
        let held = answers::standing(&root)
            .entries
            .iter()
            .any(|e| e.text == wide.text);
        assert_eq!(held, kept, "{floor}");
    }
}

#[test]
fn an_ask_held_by_the_floor_version_and_the_accepted_copy_needs_a_grant_for_each_at_the_narrowest_keep()
 {
    // F1 with a floor-version side, and `keep` narrowed across the union: HEAD asks by X
    // (everywhere); the accepted copy by X and Y (this chat); a floor version by Z (once). The
    // grants needed are all three, offered at the narrowest scope any of them writes.
    use crate::policy::force::tests::floor as floor_entry;
    let m = machine();
    let ask = |id: &str, keep: &str, when: &str| {
        format!(
            "@id(\"{id}\") @ask(\"you\") @keep(\"{keep}\")\n\
             forbid (principal, action == Action::\"exec\", resource){when};\n"
        )
    };
    let unattended = " when { context.attended == false }";
    let x = ask("processes-x", "everywhere", "");
    let y = ask("processes-y", "this chat", unattended);
    let z = ask("processes-z", "once", unattended);
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &format!("{HEADER}{x}")),
        ],
    );
    let accepted = accepted_entry(&version(STRICT, Some(&format!("{HEADER}{x}{y}"))));
    let floored = floor_entry(&version(STRICT, Some(&format!("{HEADER}{x}{z}"))));
    let mut context = base();
    context["attended"] = false.into();
    let req = request("exec", ("Command", "ls"), context);
    let judged = |grants: Vec<Entry>| {
        let mut entries = vec![accepted.clone(), floored.clone()];
        entries.extend(grants);
        let standing = holding(entries);
        let decision = judge(&root, &sources(&root), &standing, &req, None).0;
        apply(decision, &req, Some(ULID), &standing, &mut |_| true)
    };
    let once = |rule: &str| grant(rule, &req, Scope::Once, Some(ULID));
    let wide = |rule: &str| grant(rule, &req, Scope::Everywhere, None);
    // X's own grant, at the widest scope, is not enough and not offered the scope.
    let only_x = judged(vec![wide("processes-x")]);
    assert_eq!(only_x.after_mode, Outcome::Ask, "{only_x:?}");
    let mut ids = only_x.ids();
    ids.sort_unstable();
    assert_eq!(
        ids,
        ["processes-x", "processes-y", "processes-z"],
        "{only_x:?}"
    );
    assert_eq!(only_x.keep.as_deref(), Some("once"), "{only_x:?}");
    // Every rule has a grant, but a wider one than the narrowest keep serves none of them.
    let too_wide = judged(vec![
        wide("processes-x"),
        wide("processes-y"),
        wide("processes-z"),
    ]);
    assert_eq!(too_wide.after_mode, Outcome::Ask, "{too_wide:?}");
    // A Once grant for each serves.
    let all = judged(vec![
        once("processes-x"),
        once("processes-y"),
        once("processes-z"),
    ]);
    assert_eq!(all.after_mode, Outcome::Allow, "{all:?}");
}

#[test]
fn a_workspace_never_on_a_start_is_not_applied_where_the_sealed_store_cannot_be_read() {
    // KNOWN GAP, by the person's ruling Q323 (2026-11-24): M75-07b stands, so the deny state never
    // refuses a start. Where the sealed store cannot be read (nothing it holds counts, and nothing
    // was set aside as verified), a Never the person gave on `start` is not applied, and the
    // chat starts. Pinned so a change here is a decision, not a drift.
    let start = request("start", ("Profile", "claude"), base());
    let given = never(&start, Scope::Workspace);
    let unreadable = Standing {
        state: State::Deny(vec![Deny::CloneKeySwapped]),
        // What the host read of the store, none of which counts in the deny state.
        entries: vec![given.clone()],
        kept: Vec::new(),
        ..Standing::fresh()
    };
    let said = after(
        lifted_by_open(&["processes-a"]),
        &start,
        Some(ULID),
        &unreadable,
    );
    assert_eq!(said.after_mode, Outcome::Allow, "{said:?}");
    assert!(said.deny_state.is_none(), "{said:?}");
    // Control: the same Never, in what a verified store set aside, does hold (F6).
    let verified = Standing {
        kept: vec![given],
        ..unreadable
    };
    let said = after(
        lifted_by_open(&["processes-a"]),
        &start,
        Some(ULID),
        &verified,
    );
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}
