// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A persona's own sandbox grants (#1362): the hosts its chats reach, and only its chats.

use super::hosts::{Granted, Host, Level, Locks, in_force};
use super::{Compiled, Machine, Os, Plane, Policy, Refusal};

const DEVOPS: &str = "[sandbox]\nmode = \"on\"\negress = []\n\n[sandbox.personas.devops]\n\
                      hosts = [\"10.100.39.145:6443\", \"*.internal.example\"]\n";

fn policy(text: &str) -> Policy {
    Plane::of(Some(text)).said().policy.expect("on")
}

fn machine() -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(std::path::PathBuf::from("/home/op")),
        os: Os::MacOs,
    }
}

/// What a chat running as `persona` reaches, compiled from `text`, on a machine where the
/// person allowed every persona's hosts as `text` lists them (D-1362-7).
fn reached(text: &str, persona: Option<&str>) -> Vec<String> {
    let root = tempfile::tempdir().expect("a project");
    super::persona::allow_every_as_listed(root.path(), &policy(text));
    compiled_at(root.path(), text, persona)
}

/// What a chat at `root` running as `persona` reaches, compiled from `text`, with whatever the
/// person at `root` allowed.
fn compiled_at(root: &std::path::Path, text: &str, persona: Option<&str>) -> Vec<String> {
    Compiled::of(
        &policy(text),
        &Plane::of(Some(text)),
        root,
        &machine(),
        persona,
    )
    .hosts
}

/// The person at `root` allows `persona`'s hosts as `hosts` lists them.
fn allow(root: &std::path::Path, persona: &str, hosts: &[&str]) {
    let hosts: Vec<Host> = hosts.iter().map(|one| Host::parse(one).unwrap()).collect();
    super::local::allow_persona_hosts(root, persona, &super::persona::digest(&hosts, false))
        .expect("kept");
}

#[test]
fn a_chat_running_as_the_persona_reaches_its_hosts_private_address_and_port_included() {
    assert_eq!(
        reached(DEVOPS, Some("devops")),
        ["10.100.39.145:6443", "*.internal.example"]
    );
}

#[test]
fn a_chat_on_another_persona_or_on_none_does_not() {
    assert_eq!(reached(DEVOPS, Some("qa")), Vec::<String>::new());
    assert_eq!(reached(DEVOPS, None), Vec::<String>::new());
}

#[test]
fn a_persona_host_goes_through_the_same_parser_and_a_bad_one_grants_nothing() {
    let said = Plane::of(Some(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"127.0.0.1\", \
         \"10.0.0.5:6443\"]\nrun = 3\n[sandbox.personas.Bad]\nhosts = [\"a.example\"]\n",
    ))
    .said();
    let policy = said.policy.expect("on");
    assert_eq!(
        policy.personas["devops"].hosts,
        [Host::parse("10.0.0.5:6443").unwrap()]
    );
    assert!(!policy.personas.contains_key("Bad"));
    let refused: Vec<String> = said.refused.iter().map(ToString::to_string).collect();
    assert_eq!(refused.len(), 3, "{refused:?}");
    assert!(
        refused[2].starts_with("sandbox.personas.Bad in charter.toml is not a persona's name"),
        "{refused:?}"
    );
    assert!(
        refused[0]
            .starts_with("sandbox.personas.devops.run in charter.toml is not a key purlis reads"),
        "{refused:?}"
    );
    assert!(
        refused[1].starts_with(
            "sandbox.personas.devops.hosts in charter.toml names \"127.0.0.1\", which no chat \
             is let reach: 127.0.0.1 is this machine."
        ),
        "{refused:?}"
    );
    assert!(
        said.refused
            .iter()
            .all(|one| matches!(one, Refusal::Persona(_)))
    );
}

#[test]
fn personas_that_are_not_a_table_grant_nothing() {
    let said = Plane::of(Some("[sandbox]\nmode = \"on\"\npersonas = [\"devops\"]\n")).said();
    assert!(said.policy.expect("on").personas.is_empty());
    assert_eq!(said.refused.len(), 1);
}

#[test]
fn a_persona_host_the_project_already_grants_is_the_project_s() {
    let one = |typed: &str| Host::parse(typed).unwrap();
    assert_eq!(
        in_force(
            &[one("a.example")],
            &[],
            &[one("a.example"), one("10.0.0.5:6443")],
            &Locks::none()
        ),
        [
            Granted {
                host: one("a.example"),
                level: Level::Project
            },
            Granted {
                host: one("10.0.0.5:6443"),
                level: Level::Persona
            },
        ]
    );
}

#[test]
fn policy_can_forbid_persona_hosts_and_leaves_the_project_s() {
    let one = |typed: &str| Host::parse(typed).unwrap();
    let locks = Locks::parse(
        r#"{"sandbox": {"persona-hosts": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(
        in_force(&[one("a.example")], &[], &[one("10.0.0.5:6443")], &locks),
        [Granted {
            host: one("a.example"),
            level: Level::Project
        }]
    );
    let why = locks
        .refuses(&Granted {
            host: one("10.0.0.5:6443"),
            level: Level::Persona,
        })
        .expect("locked");
    assert!(why.contains("Locked by policy"), "{why}");
}

#[test]
fn a_persona_s_hosts_are_not_told_with_the_project_s_since_they_are_asked_for() {
    // D-1362-7: the project's hosts apply and are told; a persona's are asked for apart.
    assert_eq!(
        Plane::of(Some(
            "[sandbox]\nmode = \"on\"\nhosts = [\"a.example\"]\n\
             [sandbox.personas.devops]\nhosts = [\"10.100.39.145:6443\"]\n"
        ))
        .granted_hosts(&crate::sandbox::policy::Locks::none()),
        ["a.example"]
    );
}

// ---- allowed on each machine, bound to what was shown (D-1362-7) ----------------------------

#[test]
fn a_persona_s_committed_hosts_reach_nothing_until_the_person_here_allows_them() {
    let root = tempfile::tempdir().expect("a project");
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("devops")),
        Vec::<String>::new(),
        "a teammate's commit alone widens nothing"
    );
    allow(
        root.path(),
        "devops",
        &["10.100.39.145:6443", "*.internal.example"],
    );
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("devops")),
        ["10.100.39.145:6443", "*.internal.example"]
    );
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("qa")),
        Vec::<String>::new(),
        "an Allow for devops grants another persona nothing"
    );
}

#[test]
fn an_allow_is_bound_to_the_list_it_was_shown_and_a_changed_list_waits_for_a_new_one() {
    let root = tempfile::tempdir().expect("a project");
    allow(root.path(), "devops", &["10.100.39.145:6443"]);
    // A host added after the Allow: nothing of the list is granted, the old part included.
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("devops")),
        Vec::<String>::new()
    );
    // A host taken away is a change too.
    allow(
        root.path(),
        "devops",
        &["10.100.39.145:6443", "*.internal.example", "c.example"],
    );
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("devops")),
        Vec::<String>::new()
    );
    // The order a list is written in is no change.
    allow(
        root.path(),
        "devops",
        &["*.internal.example", "10.100.39.145:6443"],
    );
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("devops")),
        ["10.100.39.145:6443", "*.internal.example"]
    );
    // A Revoke takes it back.
    assert!(super::local::revoke_persona_hosts(root.path(), "devops").expect("kept"));
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("devops")),
        Vec::<String>::new()
    );
    assert!(!super::local::revoke_persona_hosts(root.path(), "devops").expect("kept"));
}

#[test]
fn the_digest_is_of_the_hosts_as_spelled_and_not_their_order() {
    let hosts =
        |list: &[&str]| -> Vec<Host> { list.iter().map(|one| Host::parse(one).unwrap()).collect() };
    let digest = |list: &[Host]| super::persona::digest(list, false);
    assert_eq!(
        digest(&hosts(&["a.example", "10.0.0.5:6443"])),
        digest(&hosts(&["10.0.0.5:6443", "A.Example."]))
    );
    assert_ne!(
        digest(&hosts(&["a.example", "10.0.0.5:6443"])),
        digest(&hosts(&["a.example", "10.0.0.5:6444"]))
    );
    assert_ne!(digest(&hosts(&["a.example"])), digest(&[]));
    assert_eq!(digest(&[]).len(), 64);
    // Being the default persona is part of what is allowed (D-1362-12).
    assert_ne!(
        super::persona::digest(&hosts(&["a.example"]), true),
        digest(&hosts(&["a.example"]))
    );
}

#[test]
fn an_allow_is_kept_only_for_the_list_as_it_stands_when_it_is_pressed() {
    use super::persona::{Shown, Standing, pick};
    let devops = |digest: &str| Shown {
        persona: "devops".to_owned(),
        listed: vec![Host::parse("10.0.0.5:6443").unwrap()],
        hosts: vec![Host::parse("10.0.0.5:6443").unwrap()],
        default: false,
        digest: digest.to_owned(),
        standing: Standing::NotAllowed,
        tools: Vec::new(),
        refused: Vec::new(),
    };
    assert_eq!(
        pick(vec![devops("now")], "devops", "now").map(|one| one.persona),
        Ok("devops".to_owned())
    );
    let changed = pick(vec![devops("now")], "devops", "shown").expect_err("changed since");
    assert!(
        changed.contains("changed after they were shown"),
        "{changed}"
    );
    let none = pick(vec![devops("now")], "qa", "now").expect_err("no hosts");
    assert!(
        none.contains("have no hosts or tools of their own"),
        "{none}"
    );
    // A persona whose every host policy locks out has nothing to allow.
    let locked = Shown {
        hosts: Vec::new(),
        ..devops("now")
    };
    assert!(pick(vec![locked], "devops", "now").is_err());
}

#[test]
fn the_notice_asks_only_for_hosts_a_chat_would_reach_and_nothing_where_policy_forbids_them() {
    let root = tempfile::tempdir().expect("a project");
    let plane = Plane::of(Some(DEVOPS));
    let shown = super::persona::shown_in(root.path(), &plane, &Locks::none());
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].hosts, shown[0].listed);
    assert!(!shown[0].allowed());
    let forbidden = Locks::parse(
        r#"{"sandbox": {"persona-hosts": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    );
    let locked = super::persona::shown_in(root.path(), &plane, &forbidden);
    assert_eq!(locked[0].hosts, Vec::<Host>::new());
    assert_eq!(locked[0].listed.len(), 2, "still said, as locked out");
    // An Allow made before the policy opens nothing while it stands.
    super::persona::allow_every_as_listed(root.path(), &policy(DEVOPS));
    assert!(!super::persona::shown_in(root.path(), &plane, &forbidden)[0].allowed());
    // The policy, not the list, changed: lifted, the Allow is in force again.
    assert!(super::persona::shown_in(root.path(), &plane, &Locks::none())[0].allowed());
    // Not sandboxed: nothing to ask.
    assert!(super::persona::shown_in(root.path(), &Plane::of(None), &Locks::none()).is_empty());
}

#[test]
fn an_allow_whose_list_changed_never_returns_to_force_even_when_the_list_comes_back() {
    use super::persona::{Standing, shown_in};
    let root = tempfile::tempdir().expect("a project");
    let first = "[sandbox]\nmode = \"on\"\negress = []\n[sandbox.personas.devops]\n\
                 hosts = [\"10.0.0.5:6443\"]\n";
    allow(root.path(), "devops", &["10.0.0.5:6443"]);
    assert_eq!(
        compiled_at(root.path(), first, Some("devops")),
        ["10.0.0.5:6443"]
    );
    // A teammate widens the list: it is seen as changed, and grants nothing.
    let wider = "[sandbox]\nmode = \"on\"\negress = []\n[sandbox.personas.devops]\n\
                 hosts = [\"10.0.0.5:6443\", \"10.0.0.6:22\"]\n";
    assert_eq!(
        compiled_at(root.path(), wider, Some("devops")),
        Vec::<String>::new()
    );
    // Back to the list that was allowed: still nothing until it is allowed anew.
    assert_eq!(
        compiled_at(root.path(), first, Some("devops")),
        Vec::<String>::new()
    );
    let shown = shown_in(root.path(), &Plane::of(Some(first)), &Locks::none());
    assert_eq!(shown[0].standing, Standing::Waiting);
    assert!(!shown[0].allowed());
    // A new Allow puts it back in force.
    allow(root.path(), "devops", &["10.0.0.5:6443"]);
    assert_eq!(
        compiled_at(root.path(), first, Some("devops")),
        ["10.0.0.5:6443"]
    );
}

/// M1 of the #1362 review: a persona that becomes the project's default reaches every chat that
/// names no persona, so an Allow made while it was not grants nothing until it is asked again.
#[test]
fn an_allow_is_bound_to_whether_the_persona_is_the_default() {
    use super::persona::{digest, shown_in};
    let root = tempfile::tempdir().expect("a project");
    let hosts = [
        Host::parse("10.100.39.145:6443").unwrap(),
        Host::parse("*.internal.example").unwrap(),
    ];
    super::local::allow_persona_hosts(root.path(), "devops", &digest(&hosts, false)).expect("kept");
    assert_eq!(compiled_at(root.path(), DEVOPS, Some("devops")).len(), 2);
    // A teammate makes devops the default persona.
    std::fs::write(
        crate::names::manifest(root.path()),
        "[persona]\ndefault = \"devops\"\n",
    )
    .expect("the project file");
    assert!(super::persona::is_default(root.path(), "devops"));
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("devops")),
        Vec::<String>::new(),
        "an Allow for devops chats is not one for every chat that names no persona"
    );
    let shown = shown_in(root.path(), &Plane::of(Some(DEVOPS)), &Locks::none());
    assert!(shown[0].default);
    assert!(!shown[0].allowed());
    // Allowed again as the default's, it reaches.
    super::local::allow_persona_hosts(root.path(), "devops", &digest(&hosts, true)).expect("kept");
    assert_eq!(compiled_at(root.path(), DEVOPS, Some("devops")).len(), 2);
    // And no longer the default: asked again.
    std::fs::write(
        crate::names::manifest(root.path()),
        "[persona]\ndefault = \"qa\"\n",
    )
    .expect("the project file");
    assert_eq!(
        compiled_at(root.path(), DEVOPS, Some("devops")),
        Vec::<String>::new()
    );
}

#[test]
fn a_private_address_is_taken_only_as_one_exact_address_and_port() {
    let said = Plane::of(Some(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"10.0.0.5:6443\", \
         \"[fd12::5]:443\", \"10.0.0.0/8\", \"10.0.0.*\", \"*.0.0.10\", \"*.10\", \
         \"10.0.0.5:6000-7000\", \"10.0.0.5:*\", \"0x0a.0.0.5\"]\n",
    ))
    .said();
    assert_eq!(
        said.policy.expect("on").personas["devops"].hosts,
        [
            Host::parse("10.0.0.5:6443").unwrap(),
            Host::parse("[fd12::5]:443").unwrap()
        ]
    );
    assert_eq!(said.refused.len(), 7, "{:?}", said.refused);
}

#[test]
fn a_change_under_a_persona_s_grants_is_a_sandbox_key_no_brokered_write_makes() {
    assert!(super::changes_a_sandbox_key(
        Some("[sandbox]\nmode = \"on\"\n"),
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"10.0.0.5\"]\n"
    ));
}

// ---- a handoff never launders a persona's hosts (D-1362-5) ----------------------------------

const TWO: &str = "[sandbox]\nmode = \"on\"\negress = []\n[sandbox.personas.devops]\nhosts = [\"10.100.39.145:6443\", \
                   \"a.example\"]\n[sandbox.personas.qa]\nhosts = [\"a.example\"]\n";

#[test]
fn a_handoff_from_a_chat_to_a_wider_persona_holds_the_asking_chat_s_grants() {
    let policy = policy(TWO);
    let held = super::persona::held_unless_within(Some(&policy), Some("qa"), Some("devops"))
        .expect("held");
    assert_eq!(held.persona.as_deref(), Some("qa"));
    // Compiled as the asking chat's persona, the new devops chat reaches only qa's hosts.
    let as_held = crate::start::grants_persona(Some(&held), Some("devops"), || None);
    assert_eq!(as_held.as_deref(), Some("qa"));
    assert_eq!(reached(TWO, as_held.as_deref()), ["a.example"]);
    // From a chat on no persona, it holds none at all.
    let none =
        super::persona::held_unless_within(Some(&policy), None, Some("devops")).expect("held");
    assert_eq!(none.persona, None);
    assert_eq!(
        reached(
            TWO,
            crate::start::grants_persona(Some(&none), Some("devops"), || None).as_deref()
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_handoff_to_a_persona_whose_hosts_the_asker_already_reaches_holds_its_own() {
    let policy = policy(TWO);
    assert_eq!(
        super::persona::held_unless_within(Some(&policy), Some("devops"), Some("qa")),
        None
    );
    assert_eq!(
        super::persona::held_unless_within(Some(&policy), Some("devops"), Some("devops")),
        None
    );
    // A persona with no hosts of its own widens nothing either.
    assert_eq!(
        super::persona::held_unless_within(Some(&policy), None, Some("writer")),
        None
    );
    // A project that does not sandbox its chats has nothing to hold.
    assert_eq!(
        super::persona::held_unless_within(None, None, Some("devops")),
        None
    );
}

/// #1407: a Resume holds a wider persona's grants whether or not the project sandboxes its
/// chats as it starts. A chat started unsandboxed keeps its hold, so turning the sandbox on later
/// never hands it what a record it could write named.
#[test]
fn a_resume_while_the_sandbox_is_off_holds_a_wider_persona_s_grants_as_one_while_it_is_on() {
    use super::persona::{for_holds, held_unless_within};
    let none = Locks::none();
    let off_text = TWO.replace("mode = \"on\"\n", "");
    let off = Plane::of(Some(&off_text));
    assert_eq!(off.in_force(&none), None, "the sandbox is off");
    let held = held_unless_within(for_holds(&off, &none).as_ref(), Some("qa"), Some("devops"))
        .expect("held while off");
    assert_eq!(held.persona.as_deref(), Some("qa"));
    let on = Plane::of(Some(TWO));
    assert_eq!(
        held_unless_within(for_holds(&on, &none).as_ref(), Some("qa"), Some("devops")),
        Some(held)
    );
    // Within the trusted persona's, it holds its own either way.
    assert_eq!(
        held_unless_within(for_holds(&off, &none).as_ref(), Some("devops"), Some("qa")),
        None
    );
    // A project that grants no persona anything has nothing to hold.
    for text in ["", "[persona]\ndefault = \"qa\"\n"] {
        assert_eq!(for_holds(&Plane::of(Some(text)), &none), None, "{text}");
    }
}

#[test]
fn a_chat_the_person_starts_holds_its_own_persona_s_grants_and_one_on_none_the_default_s() {
    assert_eq!(
        crate::start::grants_persona(None, Some("devops"), || Some("qa".to_owned())).as_deref(),
        Some("devops")
    );
    assert_eq!(
        reached(
            TWO,
            crate::start::grants_persona(None, Some("devops"), || None).as_deref()
        ),
        ["10.100.39.145:6443", "a.example"]
    );
    // No persona named: the one a new chat adopts by default, whose grants its briefing takes.
    assert_eq!(
        crate::start::grants_persona(None, None, || Some("devops".to_owned())).as_deref(),
        Some("devops")
    );
}

#[test]
fn a_chat_that_holds_another_persona_s_grants_runs_with_them_and_hands_off_holding_them() {
    // qa hands off to devops (B, held at qa); B hands off to devops again (C).
    let policy = policy(TWO);
    let b = crate::reopen::Chat {
        persona: Some("devops".to_owned()),
        held: Some(crate::reopen::HeldGrants {
            persona: Some("qa".to_owned()),
        }),
        ..Default::default()
    };
    let root = tempfile::tempdir().expect("a project");
    let b_runs_with = crate::start::runs_with(&b, root.path());
    assert_eq!(b_runs_with.as_deref(), Some("qa"));
    let c =
        super::persona::held_unless_within(Some(&policy), b_runs_with.as_deref(), Some("devops"))
            .expect("C stays held");
    assert_eq!(c.persona.as_deref(), Some("qa"));
    // Once allowed, B runs with its own, and a handoff to devops widens nothing.
    let allowed = crate::reopen::Chat { held: None, ..b };
    let runs = crate::start::runs_with(&allowed, root.path());
    assert_eq!(runs.as_deref(), Some("devops"));
    assert_eq!(
        super::persona::held_unless_within(Some(&policy), runs.as_deref(), Some("devops")),
        None
    );
}

#[test]
fn a_resume_as_a_persona_wider_than_the_default_holds_the_default_s_grants() {
    // D-1362-6: the record says devops; the default persona is qa.
    let policy = policy(TWO);
    let held = super::persona::held_unless_within(Some(&policy), Some("qa"), Some("devops"))
        .expect("held");
    assert_eq!(held.persona.as_deref(), Some("qa"));
    // A record as the default persona, or one within it, holds its own.
    assert_eq!(
        super::persona::held_unless_within(Some(&policy), Some("devops"), Some("qa")),
        None
    );
}

/// #1407: a grant under `[sandbox.personas.<name>]` for a persona the project does not define
/// is refused with a sentence, and grants nothing; one it defines is taken as before.
#[test]
fn a_grant_for_a_persona_the_project_does_not_define_is_refused() {
    let known = ["devops".to_owned()];
    let table: toml::Table =
        "[qa]\nhosts = [\"10.0.0.9:443\"]\n\n[devops]\nhosts = [\"10.0.0.5:6443\"]\n"
            .parse()
            .expect("TOML");
    let value = toml::Value::Table(table);
    let (grants, refused) = super::persona::read(Some(&value), "charter.toml", Some(&known));
    assert_eq!(grants.keys().collect::<Vec<_>>(), ["devops"]);
    assert_eq!(
        refused,
        [
            "sandbox.personas.qa in charter.toml names no persona of this project, so it grants \
          nothing — make the persona first, or take its table out"
        ]
    );
    // Where the personas are not known, nothing is refused for it.
    let (grants, refused) = super::persona::read(Some(&value), "charter.toml", None);
    assert_eq!(grants.len(), 2);
    assert!(refused.is_empty(), "{refused:?}");
}

/// #1407: a Settings save is refused the grant, in the reader's words, and is let through once
/// the persona is made, at the next read.
#[test]
fn a_settings_save_refuses_a_grant_for_a_persona_that_is_not_there_until_it_is() {
    let project = tempfile::tempdir().expect("a project");
    let text = "[sandbox]\nmode = \"on\"\n\n[sandbox.personas.qa]\nhosts = [\"10.0.0.9:443\"]\n";
    let refused = super::refusals_at(project.path(), text, "charter.toml");
    assert!(
        refused
            .iter()
            .any(|why| why.starts_with("sandbox.personas.qa in charter.toml names no persona")),
        "{refused:?}"
    );
    let qa = project.path().join("personas/qa");
    std::fs::create_dir_all(&qa).expect("a persona folder");
    std::fs::write(qa.join("persona.md"), "---\nrole: qa\n---\n\n# qa\n").expect("persona.md");
    assert_eq!(
        super::refusals_at(project.path(), text, "charter.toml"),
        Vec::<String>::new()
    );
}

// ---- its tools (#1407) ---------------------------------------------------------------------

#[test]
fn a_persona_s_tools_are_read_beside_its_hosts_and_a_bad_one_is_refused_alone() {
    let said = Plane::of(Some(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"10.0.0.5:6443\"]\n\
         tools = { kubectl = { vault = \"devops\", as = \"KUBECONFIG file\", key = \"kube\" }, \
         sh = { vault = \"devops\", as = \"X file\", key = \"k\" } }\n",
    ))
    .said();
    let policy = said.policy.expect("on");
    let devops = &policy.personas["devops"];
    assert_eq!(devops.hosts, [Host::parse("10.0.0.5:6443").unwrap()]);
    assert_eq!(
        devops
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["kubectl"]
    );
    let refused: Vec<String> = said.refused.iter().map(ToString::to_string).collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(
        refused[0].starts_with("sandbox.personas.devops.tools.sh in charter.toml runs nothing"),
        "{refused:?}"
    );
}

// ---- its tools wait for the same Allow (D-PE-ruling-1) -----------------------------------------

const DEVOPS_TOOLS: &str = "[sandbox]\nmode = \"on\"\negress = []\n\n[sandbox.personas.devops]\n\
                            hosts = [\"10.0.0.5:6443\"]\ntools = { kubectl = { vault = \"devops\", \
                            as = \"KUBECONFIG file\", key = \"kubeconfig\" } }\n";

const ONLY_TOOLS: &str = "[sandbox]\nmode = \"on\"\negress = []\n\n[sandbox.personas.devops]\n\
                          tools = { kubectl = { vault = \"devops\", as = \"KUBECONFIG file\", \
                          key = \"kubeconfig\" } }\n";

fn tools_here(root: &std::path::Path, text: &str, locks: &Locks) -> Vec<String> {
    super::persona::tools_in_force_here(root, &policy(text).personas, Some("devops"), locks)
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

#[test]
fn a_persona_without_tools_keeps_the_digest_an_allow_was_made_with() {
    let hosts = [Host::parse("10.0.0.5:6443").unwrap()];
    for default in [false, true] {
        assert_eq!(
            super::persona::digest_of(&hosts, &[], default),
            super::persona::digest(&hosts, default)
        );
    }
    let tools = &policy(DEVOPS_TOOLS).personas["devops"].tools;
    assert_ne!(
        super::persona::digest_of(&hosts, tools, false),
        super::persona::digest(&hosts, false),
        "a tool is part of what is allowed"
    );
}

#[test]
fn a_runner_arms_a_persona_s_tools_on_its_desktops_digest_and_never_its_hosts() {
    use super::local::desktop_allows_tools;
    use super::persona::{digest_allowed_here, digest_of, in_force_here};
    let desktop = tempfile::tempdir().expect("a project");
    let runner = tempfile::tempdir().expect("the same project there");
    let personas = policy(DEVOPS_TOOLS).personas;
    let hosts = [Host::parse("10.0.0.5:6443").unwrap()];
    let digest = digest_of(&hosts, &personas["devops"].tools, false);
    let none = Locks::none();

    assert_eq!(
        digest_allowed_here(desktop.path(), &personas, "devops", &none),
        None,
        "nothing allowed there, nothing said"
    );
    super::local::allow_persona_hosts(desktop.path(), "devops", &digest).unwrap();
    assert_eq!(
        digest_allowed_here(desktop.path(), &personas, "devops", &none).as_deref(),
        Some(digest.as_str())
    );

    // Another list than this runner reads arms nothing.
    desktop_allows_tools(runner.path(), "devops", Some("another")).unwrap();
    assert_eq!(
        tools_here(runner.path(), DEVOPS_TOOLS, &none),
        Vec::<String>::new()
    );
    // The same list arms its tools, and its hosts stay off.
    desktop_allows_tools(runner.path(), "devops", Some(&digest)).unwrap();
    assert_eq!(tools_here(runner.path(), DEVOPS_TOOLS, &none).len(), 1);
    assert_eq!(
        in_force_here(runner.path(), &personas, Some("devops"), &none),
        Vec::new(),
        "the persona's hosts are this machine's Allow alone"
    );
    // Another persona is not armed by it.
    assert_eq!(
        super::persona::tools_in_force_here(runner.path(), &personas, Some("reviewer"), &none),
        Vec::new()
    );
    // Taken back there, the next start says so.
    desktop_allows_tools(runner.path(), "devops", None).unwrap();
    assert_eq!(
        tools_here(runner.path(), DEVOPS_TOOLS, &none),
        Vec::<String>::new()
    );
}

#[test]
fn a_persona_s_tools_reach_no_chat_until_this_machine_allows_them_with_its_hosts() {
    use super::persona::{Standing, digest_of, shown_in};
    let root = tempfile::tempdir().expect("a project");
    let plane = Plane::of(Some(DEVOPS_TOOLS));
    let shown = shown_in(root.path(), &plane, &Locks::none());
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].tools.len(), 1, "the Notice shows the tool");
    assert!(!shown[0].allowed());
    assert_eq!(
        tools_here(root.path(), DEVOPS_TOOLS, &Locks::none()),
        Vec::<String>::new()
    );
    assert_eq!(
        compiled_at(root.path(), DEVOPS_TOOLS, Some("devops")),
        Vec::<String>::new()
    );
    // An Allow of the hosts alone, as one made before tools were read, allows neither.
    allow(root.path(), "devops", &["10.0.0.5:6443"]);
    assert_eq!(
        tools_here(root.path(), DEVOPS_TOOLS, &Locks::none()),
        Vec::<String>::new()
    );
    assert_eq!(
        compiled_at(root.path(), DEVOPS_TOOLS, Some("devops")),
        Vec::<String>::new()
    );
    assert_eq!(
        shown_in(root.path(), &plane, &Locks::none())[0].standing,
        Standing::Waiting
    );
    // The Allow of what the Notice shows: both.
    let now = shown_in(root.path(), &plane, &Locks::none()).remove(0);
    super::local::allow_persona_hosts(root.path(), "devops", &now.digest).expect("kept");
    assert_eq!(
        tools_here(root.path(), DEVOPS_TOOLS, &Locks::none()),
        ["kubectl"]
    );
    assert_eq!(
        compiled_at(root.path(), DEVOPS_TOOLS, Some("devops")),
        ["10.0.0.5:6443"]
    );
    assert_eq!(
        now.digest,
        digest_of(
            &now.hosts,
            &policy(DEVOPS_TOOLS).personas["devops"].tools,
            false
        )
    );
    // A teammate points the tool at another key: neither stands until it is allowed again.
    let other_key = DEVOPS_TOOLS.replace("key = \"kubeconfig\"", "key = \"admin\"");
    assert_eq!(
        tools_here(root.path(), &other_key, &Locks::none()),
        Vec::<String>::new()
    );
    assert_eq!(
        compiled_at(root.path(), &other_key, Some("devops")),
        Vec::<String>::new()
    );
    // And a chat on another persona never gets them.
    assert!(
        super::persona::tools_in_force_here(
            root.path(),
            &policy(DEVOPS_TOOLS).personas,
            Some("qa"),
            &Locks::none()
        )
        .is_empty()
    );
}

#[test]
fn a_persona_with_tools_and_no_hosts_is_asked_about_and_allowed_the_same_way() {
    use super::persona::{pick, shown_in};
    let root = tempfile::tempdir().expect("a project");
    let plane = Plane::of(Some(ONLY_TOOLS));
    let shown = shown_in(root.path(), &plane, &Locks::none());
    assert_eq!(shown.len(), 1);
    assert!(shown[0].hosts.is_empty());
    assert!(shown[0].asks());
    assert!(!shown[0].allowed());
    let digest = shown[0].digest.clone();
    let picked = pick(shown, "devops", &digest).expect("allowable");
    super::local::allow_persona_hosts(root.path(), "devops", &picked.digest).expect("kept");
    assert!(shown_in(root.path(), &plane, &Locks::none())[0].allowed());
    assert_eq!(
        tools_here(root.path(), ONLY_TOOLS, &Locks::none()),
        ["kubectl"]
    );
}

#[test]
fn policy_forbidding_persona_tools_asks_for_none_and_arms_none_even_when_allowed() {
    use super::persona::shown_in;
    let root = tempfile::tempdir().expect("a project");
    let forbidden = Locks::parse(
        r#"{"sandbox": {"persona-tools": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    );
    // Allowed while the policy was not there.
    super::persona::allow_every_as_listed(root.path(), &policy(ONLY_TOOLS));
    assert_eq!(
        tools_here(root.path(), ONLY_TOOLS, &Locks::none()),
        ["kubectl"]
    );
    let locked = shown_in(root.path(), &Plane::of(Some(ONLY_TOOLS)), &forbidden);
    assert!(locked[0].tools.is_empty());
    assert!(!locked[0].asks(), "nothing to ask: no chat would get it");
    assert_eq!(
        tools_here(root.path(), ONLY_TOOLS, &forbidden),
        Vec::<String>::new()
    );
    // The policy, not the list, changed: lifted, the Allow is in force again.
    assert_eq!(
        tools_here(root.path(), ONLY_TOOLS, &Locks::none()),
        ["kubectl"]
    );
    // Hosts and tools: the hosts are still asked about, the tool is not.
    let both = shown_in(root.path(), &Plane::of(Some(DEVOPS_TOOLS)), &forbidden);
    assert_eq!(both[0].hosts.len(), 1);
    assert!(both[0].tools.is_empty());
}

#[test]
fn a_resume_whose_persona_declares_a_tool_the_trusted_one_does_not_is_held() {
    let text = format!("{DEVOPS_TOOLS}\n[sandbox.personas.steward]\nhosts = [\"10.0.0.5:6443\"]\n");
    let policy = policy(&text);
    let held = super::persona::held_unless_within(Some(&policy), Some("steward"), Some("devops"));
    assert_eq!(
        held.and_then(|held| held.persona),
        Some("steward".to_owned()),
        "same hosts, one more tool: held"
    );
    assert!(
        super::persona::held_unless_within(Some(&policy), Some("devops"), Some("devops")).is_none()
    );
}

/// A tool whose program, as found on this machine and as its links resolve, is one no tool may
/// be is left out of what is shown and allowed, and says why.
#[test]
fn a_tool_whose_program_is_one_no_tool_may_be_is_not_shown_or_allowed_and_says_why() {
    use super::persona::{digest_of, shown_in};
    let root = tempfile::tempdir().expect("a project");
    let base = tempfile::tempdir().expect("a machine");
    let system = base.path().join("system");
    std::fs::create_dir_all(&system).expect("dir");
    let elsewhere = base.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("dir");
    let real = elsewhere.join("zsh");
    std::fs::write(&real, "#!/bin/sh\n").expect("written");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).expect("mode");
    }
    std::os::unix::fs::symlink(&real, system.join("fetch")).expect("link");
    super::tools::SEARCH_HERE.with(|dirs| *dirs.borrow_mut() = Some(vec![system.clone()]));
    super::tools::TEMPS_HERE.with(|temps| *temps.borrow_mut() = Some(Vec::new()));

    let text = DEVOPS_TOOLS.replace(
        "tools = { kubectl = {",
        "tools = { fetch = { vault = \"devops\", as = \"TOKEN env\", key = \"token\" }, kubectl = {",
    );
    let declared = &policy(&text).personas["devops"].tools;
    assert_eq!(declared.len(), 2, "the name itself is taken");
    let shown = shown_in(root.path(), &Plane::of(Some(&text)), &Locks::none()).remove(0);
    let names: Vec<&str> = shown.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(names, ["kubectl"]);
    assert_eq!(shown.refused.len(), 1, "{:?}", shown.refused);
    assert!(
        shown.refused[0].starts_with("devops's tool fetch runs nothing: fetch runs whatever"),
        "{:?}",
        shown.refused
    );
    // An Allow of the list with it in grants nothing; one of the list as shown, never it.
    super::local::allow_persona_hosts(
        root.path(),
        "devops",
        &digest_of(&shown.hosts, declared, false),
    )
    .expect("kept");
    assert_eq!(
        tools_here(root.path(), &text, &Locks::none()),
        Vec::<String>::new()
    );
    super::local::allow_persona_hosts(root.path(), "devops", &shown.digest).expect("kept");
    assert_eq!(tools_here(root.path(), &text, &Locks::none()), ["kubectl"]);

    super::tools::SEARCH_HERE.with(|dirs| *dirs.borrow_mut() = None);
    super::tools::TEMPS_HERE.with(|temps| *temps.borrow_mut() = None);
}

/// #2241: a persona declares its browser in its sandbox section, `browser = "project"` or
/// `"open-web"`, in the committed file, whether or not the project sandboxes its chats. The
/// declaration grants nothing: no host, no tool, and nothing to allow on this machine.
#[test]
fn a_persona_declares_its_browser_in_its_sandbox_section_and_is_granted_nothing_by_it() {
    use super::persona::{Browser, declared_browser};
    let on =
        "[sandbox]\nmode = \"on\"\negress = []\n\n[sandbox.personas.qa]\nbrowser = \"open-web\"\n";
    let plane = Plane::of(Some(on));
    assert_eq!(declared_browser(&plane, "qa"), Some(Browser::OpenWeb));
    assert_eq!(declared_browser(&plane, "devops"), None);
    assert!(plane.said().refused.is_empty());
    let unset = "[sandbox.personas.qa]\nbrowser = \"project\"\n";
    assert_eq!(
        declared_browser(&Plane::of(Some(unset)), "qa"),
        Some(Browser::Project)
    );
    // Nothing to reach or allow for it.
    assert_eq!(reached(on, Some("qa")), Vec::<String>::new());
    let root = tempfile::tempdir().expect("a project");
    assert!(
        super::persona::shown_in(root.path(), &plane, &Locks::none()).is_empty(),
        "a declared browser asks nothing of the person's hosts Notice"
    );
}

/// #2241: a declaration that is neither word is refused with a sentence, and declares nothing.
#[test]
fn a_browser_declaration_that_is_neither_word_is_refused_and_declares_nothing() {
    use super::persona::declared_browser;
    let text = "[sandbox]\nmode = \"on\"\n\n[sandbox.personas.qa]\nbrowser = \"everything\"\n";
    let plane = Plane::of(Some(text));
    assert_eq!(declared_browser(&plane, "qa"), None);
    let refused = super::refusals(text, "charter.toml");
    assert_eq!(
        refused,
        [
            "sandbox.personas.qa.browser in charter.toml is not \"project\" or \"open-web\", so \
             qa declares no browser — write browser = \"project\" for this project's sites and \
             its dev servers, or \"open-web\" for the open web too"
        ]
    );
}

/// #2241 security: a persona declares its browser only in the project's root `charter.toml`,
/// which no sandboxed chat writes: what its `persona.md` says (a file a chat may edit) and what the
/// person's local file says declare nothing.
#[test]
fn a_browser_written_where_a_chat_may_write_declares_nothing() {
    use super::persona::declared_browser;
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    std::fs::write(
        root.join(crate::plane::MANIFEST),
        "schema = 1\n\n[sandbox]\nmode = \"on\"\n",
    )
    .expect("the committed file");
    let qa = root.join("personas/qa");
    std::fs::create_dir_all(&qa).expect("a persona folder");
    std::fs::write(
        qa.join("persona.md"),
        "---\nrole: qa\nbrowser: open-web\nsandbox:\n  browser: open-web\n---\n\n# qa\n",
    )
    .expect("persona.md");
    std::fs::write(
        root.join(crate::profiles::LOCAL_FILE),
        "[sandbox.personas.qa]\nbrowser = \"open-web\"\n",
    )
    .expect("the local file");

    assert_eq!(declared_browser(&Plane::read(root), "qa"), None);
}
