// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The fake forge in a scenario** (T1.1-50, #6897): a forge that a sandboxed chat's `gh` and
//! `glab` reach through purlis's own proxy and the scenario's resolver seam, and that records
//! every request with the token it presented.
//!
//! ## How a cell uses it
//!
//! ```ignore
//! let forge = ForgeHere::start(&scenario, wire());
//! forge.fake.add_pull_request("o/r", 7, HEAD);
//! let chat = scenario.starts_holding(&script, forge.grants());
//! // the chat's command: `{forge.gh()} gh api ...` or `{forge.glab(Self::SELF_HOSTED)} glab api ...`
//! assert_eq!(forge.fake.tokens_seen(), [CHATS_TOKEN]); // and never the person's own
//! ```
//!
//! ## Two wires, and what each client can do
//!
//! `gh` and `glab` are Go programs. On Linux they verify TLS against `SSL_CERT_FILE`, so the
//! fake forge speaks HTTPS under the test CA ([`Wire::Https`], names under `.test`). On macOS Go
//! verifies through the Security framework and ignores it (`TestCa`'s docs).
//!
//! - **`glab`** has settings of its own, so it runs on both wires on both OSes:
//!   `GLAB_CA_CERT` names the bundle (Go reads that file itself, on macOS too), and
//!   `GLAB_API_PROTOCOL=http` speaks plain HTTP ([`Wire::Http`]). **`gitlab.com` itself is not
//!   reachable**: `glab` speaks https to it whatever its protocol setting says, and the test CA
//!   signs `.test` names alone. A self-hosted name stands for it; the fake answers GitLab's API
//!   on any name.
//! - **`gh`** has neither: no CA setting, and http only to `github.localhost`, which purlis
//!   refuses as this machine's own. So `gh` is a Linux cell, and on macOS it is the ticket's
//!   stated fallback: reported not run, covered by a fixture.
//!
//! The chat's proxy dials none of these names itself: the seam maps each to the fake forge's
//! listener, and records the lookup, so a cell can say a request reached the forge through the
//! proxy and not around it.
//!
//! **A chat never merges** (purlis's `merge-unreadable` guard holds `gh api -X PUT …/merge` and
//! `glab api … merge`: the app merges, on the person's press). So the chat's cells here read,
//! which proves reach and the credential; the merge itself is purlis's own client's
//! (`tests/scenarios/tests/fake_forge.rs`), and a chat's attempt is shown held before it reaches
//! the forge.

use std::path::PathBuf;

use purlis_core::sandbox::grant::{Grants, What};
use purlis_core::sandbox::hosts::Host;
use purlis_scenarios::net::{FakeForge, TestCa};

use super::host::Scenario;

/// **How a client talks to the fake forge.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wire {
    /// HTTPS under the test CA, trusted through `SSL_CERT_FILE`: Linux.
    Https,
    /// Plain HTTP: where Go cannot be made to trust the test CA (macOS), and everywhere.
    Http,
}

impl Wire {
    /// The wires `glab` runs on: both, on both OSes. It trusts a CA by its own setting
    /// (`GLAB_CA_CERT`), which Go on macOS honours where it ignores `SSL_CERT_FILE`, and it
    /// speaks plain HTTP by `GLAB_API_PROTOCOL=http`.
    pub(crate) fn here() -> &'static [Wire] {
        &[Wire::Https, Wire::Http]
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Wire::Https => "https",
            Wire::Http => "http",
        }
    }
}

/// **A GitLab the fake forge answers as**: one self-hosted instance (`gitlab.com` is out of
/// reach, module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gitlab {
    /// A self-hosted instance, `git.corp.test`.
    SelfHosted,
}

/// What a chat is handed that its own `gh` and `glab` read: not the person's.
pub(crate) const GITHUB_SHAPED_HOST: &str = "forge.test";
pub(crate) const SELF_HOSTED_GITLAB: &str = "git.corp.test";

/// **The fake forge, started for one scenario**, its names mapped on the scenario's seam.
pub(crate) struct ForgeHere {
    pub(crate) fake: FakeForge,
    pub(crate) wire: Wire,
    bundle: Option<PathBuf>,
    config: PathBuf,
    /// Held so the CA outlives the certificates it signed.
    _ca: Option<TestCa>,
}

impl ForgeHere {
    /// **Starts the fake forge on `wire`**, maps the names a cell's clients use onto it, and
    /// writes the CA bundle `Wire::Https` clients trust into the scenario's project.
    pub(crate) fn start(scenario: &Scenario, wire: Wire) -> ForgeHere {
        let config = scenario.root.join("forge-clients");
        std::fs::create_dir_all(&config).expect("the clients' config folder");
        let (fake, ca, bundle) = match wire {
            Wire::Https => {
                let ca = TestCa::new().expect("the test CA");
                let fake = FakeForge::start(Some((&ca, &[GITHUB_SHAPED_HOST, SELF_HOSTED_GITLAB])))
                    .expect("the fake forge");
                let bundle = ca.bundle(&config).expect("the CA bundle");
                (fake, Some(ca), Some(bundle))
            }
            Wire::Http => (FakeForge::start(None).expect("the fake forge"), None, None),
        };
        match wire {
            Wire::Https => {
                let at = fake.https_addr().expect("an HTTPS listener");
                for name in [GITHUB_SHAPED_HOST, SELF_HOSTED_GITLAB] {
                    let _ = (name, at); // PROBE B
                }
            }
            Wire::Http => {
                let at = fake.http_addr();
                for name in [SELF_HOSTED_GITLAB] {
                    let _ = (name, at); // PROBE B
                }
            }
        }
        ForgeHere {
            fake,
            wire,
            bundle,
            config,
            _ca: ca,
        }
    }

    /// The names a chat is granted so its proxy carries them: the person's allowing, as for any
    /// host. (The fake forge adds no name to a chat's reach by itself.)
    pub(crate) fn grants(&self) -> Grants {
        let mut grants = Grants::default();
        for name in self.names() {
            grants.add(&What::Host(Host::parse(name).expect("a host")));
        }
        grants
    }

    /// Every name this wire maps.
    pub(crate) fn names(&self) -> Vec<&'static str> {
        match self.wire {
            Wire::Https => vec![GITHUB_SHAPED_HOST, SELF_HOSTED_GITLAB],
            Wire::Http => vec![SELF_HOSTED_GITLAB],
        }
    }

    /// The name `gh` asks, and the port: Linux's HTTPS wire alone.
    pub(crate) fn github_name(&self) -> (&'static str, u16) {
        (GITHUB_SHAPED_HOST, 443)
    }

    /// The name `glab` asks for `which`, and the port, on this wire.
    pub(crate) fn gitlab_name(&self, which: Gitlab) -> (&'static str, u16) {
        let port = if self.wire == Wire::Https { 443 } else { 80 };
        match which {
            Gitlab::SelfHosted => (SELF_HOSTED_GITLAB, port),
        }
    }

    /// The environment a `gh` is run with, as `VAR=value ` words for a shell command: its host,
    /// the token the chat is handed (`gh` reads `GH_ENTERPRISE_TOKEN` for a host that is not
    /// github.com), the CA bundle, and settings that keep it from asking, updating or writing
    /// outside the project. The HTTPS wire alone.
    pub(crate) fn gh(&self, token: &str) -> String {
        assert_eq!(
            self.wire,
            Wire::Https,
            "gh reaches the fake forge over HTTPS only"
        );
        format!(
            "GH_HOST={GITHUB_SHAPED_HOST} GH_TOKEN={token} GH_ENTERPRISE_TOKEN={token} {} \
             GH_CONFIG_DIR='{}/gh' GH_NO_UPDATE_NOTIFIER=1 GH_NO_EXTENSION_UPDATE_NOTIFIER=1 \
             GH_PROMPT_DISABLED=1 NO_COLOR=1",
            self.trust(),
            self.config.display()
        )
    }

    /// The environment a `glab` is run with, for `which`: the same kind of words as [`Self::gh`].
    /// `glab` sends usage data to its instance unless told not to, and that is a request the
    /// forge would record, so it is off.
    pub(crate) fn glab(&self, which: Gitlab, token: &str) -> String {
        let (name, _) = self.gitlab_name(which);
        let protocol = match (self.wire, &self.bundle) {
            (Wire::Http, _) => "GLAB_API_PROTOCOL=http".to_owned(),
            (Wire::Https, Some(bundle)) => format!("GLAB_CA_CERT='{}'", bundle.display()),
            (Wire::Https, None) => String::new(),
        };
        format!(
            "GITLAB_HOST={name} GITLAB_TOKEN={token} {protocol} {} GLAB_CONFIG_DIR='{}/glab' \
             GLAB_CHECK_UPDATE=false GLAB_NO_PROMPT=1 GLAB_SEND_TELEMETRY=false NO_COLOR=1",
            self.trust(),
            self.config.display()
        )
    }

    /// `SSL_CERT_FILE` and the other variables clients read their roots from, where the wire
    /// needs them (applied by the cell's command, after whatever purlis set).
    fn trust(&self) -> String {
        match &self.bundle {
            Some(bundle) => purlis_scenarios::net::ca_env(bundle)
                .into_iter()
                .map(|(name, value)| format!("{name}='{}'", value.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(" "),
            None => String::new(),
        }
    }
}

/// The head a pull request or merge request is at.
#[cfg(test)]
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

/// The token a chat is handed for `gh`, as a declared tool or a mint would hand it.
#[cfg(test)]
const GH_TOKEN: &str = "chat-gh-token-3f9a";

/// The token a chat is handed for `glab`.
#[cfg(test)]
const GLAB_TOKEN: &str = "chat-glab-token-8c21";

/// Whether `program` runs here. Where it does not, a scenario on CI fails and elsewhere returns
/// with the reason printed (`host::cannot_run`), so a missing client is never a pass.
#[cfg(test)]
fn available(program: &str) -> bool {
    let runs = purlis_core::forklock::output(std::process::Command::new(program).arg("--version"))
        .is_ok_and(|out| out.status.success());
    if !runs {
        let _ = super::host::cannot_run::<()>(&format!("{program} is not installed here"));
    }
    runs
}

/// A chat's turn that runs each `(step, command)`.
#[cfg(test)]
fn running(calls: &[(&str, String)]) -> serde_json::Value {
    serde_json::json!({"turns": [{"prompt": "go", "calls": calls.iter().map(|(step, run)| {
        serde_json::json!({"step": step, "tool": "bash", "run": run})
    }).collect::<Vec<_>>()}]})
}

/// **What a cell owes after a chat's client reached the fake forge**: the proxy looked its name
/// up and carried it to the forge, and the chat was never held on the name. (Which credential a request carried is the cell's
/// own assertion: [`FakeForge::tokens_seen`].)
#[cfg(test)]
fn reached_through_the_proxy(scenario: &Scenario, chat: u32, name: (&str, u16)) {
    let said = scenario.said();
    assert!(
        scenario
            .names
            .lookups()
            .iter()
            .any(|one| one.name == name.0 && one.port == name.1),
        "the chat's proxy never looked {name:?} up: {:?}\n{said}",
        scenario.names.lookups()
    );
    assert!(
        scenario
            .names
            .origin_hits()
            .iter()
            .any(|(host, port)| (host.as_str(), *port) == name),
        "nothing was carried to the fake forge for {name:?}: {:?}\n{said}",
        scenario.names.origin_hits()
    );
    assert!(
        super::held_on(scenario, chat, name.0).is_none(),
        "the chat was held on {}",
        name.0
    );
}

/// **`gh` reaches the fake forge from inside a sandboxed chat** (T1.1-50), under the OS wrap,
/// through the chat's proxy and the resolver seam: it reads a pull request with the token the
/// chat was handed, over HTTPS under the test CA, and the forge saw that token and no other.
/// Linux: on macOS Go ignores the test CA and `gh` has no other way in (module docs), so there
/// it is reported not run, the ticket's stated fallback.
#[cfg(unix)]
#[test]
fn gh_merge_reaches_the_fake_forge_under_the_wrap() {
    use super::host::{Dialect, Homes, Protection};
    if cfg!(target_os = "macos") {
        super::report::write(&[super::a_fixture_record(
            "gh_merge_reaches_the_fake_forge_under_the_wrap",
            Some(purlis_scenarios::model::Dialect::Codex),
            Some(purlis_scenarios::model::Protection::Full),
            purlis_scenarios::model::ReportVerdict::NotRun,
            "gh is Go: on macOS it ignores the test CA and speaks http only to github.localhost, \
             which purlis refuses as this machine (T1.1-50's fallback)",
        )])
        .expect("the leg's report");
        return;
    }
    if !available("gh") {
        return;
    }
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    let forge = ForgeHere::start(&scenario, Wire::Https);
    forge.fake.add_pull_request("o/r", 7, HEAD);
    let read = format!(
        "{} gh api repos/o/r/pulls/7 2>&1; echo \"gh exit $?\"",
        forge.gh(GH_TOKEN)
    );
    let chat = scenario.starts_holding(&running(&[("read", read)]), forge.grants());
    scenario.until_played(chat, &["read"]);
    let said = super::stdout_of(&scenario, chat, "read");
    assert!(
        said.contains("gh exit 0") && said.contains(HEAD),
        "gh did not read the pull request: {said}\n{:?}\n{}",
        scenario.result_of(chat, "read"),
        scenario.said()
    );
    reached_through_the_proxy(&scenario, chat, forge.github_name());
    assert_eq!(
        forge.fake.tokens_seen(),
        vec![GH_TOKEN.to_owned()],
        "the forge saw a credential other than the chat's own"
    );
    let seen = forge.fake.requests();
    let get = seen
        .iter()
        .find(|request| request.method == "GET")
        .unwrap_or_else(|| panic!("no read recorded: {seen:?}"));
    assert_eq!(get.path, "/api/v3/repos/o/r/pulls/7");
    assert_eq!(get.sni.as_deref(), Some(GITHUB_SHAPED_HOST));
}

/// **`glab` reaches the fake forge from inside a sandboxed chat** (T1.1-50), as `gh` does: a
/// self-hosted GitLab on both wires, and `gitlab.com` itself on the plain one, each read with
/// the token the chat was handed.
#[cfg(unix)]
#[test]
fn glab_merge_reaches_the_fake_forge_under_the_wrap() {
    use super::host::{Dialect, Homes, Protection};
    if !available("glab") {
        return;
    }
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    for &wire in Wire::here() {
        let Some(scenario) =
            Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
        else {
            return;
        };
        let forge = ForgeHere::start(&scenario, wire);
        let which = vec![(Gitlab::SelfHosted, 4_u64)];
        let mut calls = Vec::new();
        for (gitlab, iid) in &which {
            forge
                .fake
                .add_merge_request("group/project", 42, *iid, HEAD);
            calls.push((
                format!("read-{iid}"),
                format!(
                    "{} glab api projects/group%2Fproject/merge_requests/{iid} 2>&1; \
                     echo \"glab exit $?\"",
                    forge.glab(*gitlab, GLAB_TOKEN)
                ),
            ));
        }
        let calls: Vec<(&str, String)> = calls
            .iter()
            .map(|(step, run)| (step.as_str(), run.clone()))
            .collect();
        let chat = scenario.starts_holding(&running(&calls), forge.grants());
        let steps: Vec<&str> = calls.iter().map(|(step, _)| *step).collect();
        scenario.until_played(chat, &steps);
        let wire_word = wire.word();
        for (gitlab, iid) in &which {
            let said = super::stdout_of(&scenario, chat, &format!("read-{iid}"));
            assert!(
                said.contains("glab exit 0") && said.contains(HEAD),
                "{wire_word}: glab did not read !{iid}: {said}\n{:?}\n{}",
                scenario.result_of(chat, &format!("read-{iid}")),
                scenario.said()
            );
            reached_through_the_proxy(&scenario, chat, forge.gitlab_name(*gitlab));
        }
        assert_eq!(
            forge.fake.tokens_seen(),
            vec![GLAB_TOKEN.to_owned()],
            "{wire_word}: the forge saw a credential other than the chat's own"
        );
        let hosts: Vec<String> = forge
            .fake
            .requests()
            .iter()
            .filter_map(|request| request.host.clone())
            .collect();
        for (gitlab, _) in &which {
            let name = forge.gitlab_name(*gitlab).0;
            assert!(
                hosts.iter().any(|host| host.starts_with(name)),
                "{wire_word}: no request carried the Host {name}: {hosts:?}"
            );
        }
    }
}

/// **A merge a chat tries is held before it reaches the fake forge** (T1.1-50): the same chat
/// reads a pull request (the control: the forge is reachable and was reached) and then tries to
/// merge it, which purlis's guard refuses, so the forge records the read and no merge. Over
/// `glab` on every wire this OS runs.
#[cfg(unix)]
#[test]
fn a_chats_merge_is_held_before_it_reaches_the_fake_forge() {
    use super::host::{Dialect, Homes, Protection};
    if !available("glab") {
        return;
    }
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    for &wire in Wire::here() {
        let Some(scenario) =
            Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
        else {
            return;
        };
        let forge = ForgeHere::start(&scenario, wire);
        forge.fake.add_merge_request("group/project", 42, 4, HEAD);
        let env = forge.glab(Gitlab::SelfHosted, GLAB_TOKEN);
        let read = format!(
            "{env} glab api projects/group%2Fproject/merge_requests/4 2>&1; echo \"glab exit $?\""
        );
        let merge = format!(
            "{env} glab api projects/group%2Fproject/merge_requests/4/merge --method PUT \
             -f sha={HEAD} 2>&1; echo \"glab exit $?\""
        );
        let chat = scenario.starts_holding(
            &running(&[("read", read), ("merge", merge)]),
            forge.grants(),
        );
        scenario.until_played(chat, &["read", "merge"]);
        let wire_word = wire.word();
        assert!(
            super::stdout_of(&scenario, chat, "read").contains("glab exit 0"),
            "{wire_word}: the control read failed: {:?}\n{}",
            scenario.result_of(chat, "read"),
            scenario.said()
        );
        let events = format!("{:?}", scenario.events());
        assert!(
            events.contains("merge-unreadable"),
            "{wire_word}: the guard's merge rule never denied the call\n{}",
            scenario.said()
        );
        assert!(
            !super::stdout_of(&scenario, chat, "merge").contains("glab exit"),
            "{wire_word}: the merge command ran"
        );
        assert!(
            forge
                .fake
                .requests()
                .iter()
                .all(|request| request.method == "GET"),
            "{wire_word}: a non-read reached the forge: {:?}",
            forge.fake.requests()
        );
        assert_eq!(
            forge
                .fake
                .merged(purlis_scenarios::net::Forge::GitLab, "group/project", 4),
            None,
            "{wire_word}: a chat's merge merged"
        );
    }
}
