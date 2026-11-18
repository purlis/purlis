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
//! ## Two wires, because Go on macOS ignores the test CA
//!
//! `gh` and `glab` are Go programs. On Linux they verify TLS against `SSL_CERT_FILE`, so the
//! fake forge speaks HTTPS under the test CA ([`Wire::Https`], names under `.test`). On macOS Go
//! verifies through the Security framework and ignores it (`TestCa`'s docs), so a cell speaks
//! plain HTTP ([`Wire::Http`]): `gh` to `github.localhost`, which it reaches as
//! `http://api.github.localhost/` (it treats that one host as a local GitHub), and `glab` to a
//! `GITLAB_HOST` that names the `http://` scheme. gitlab.com itself is only reachable on the
//! plain wire: the test CA signs `.test` names alone.
//!
//! The chat's proxy dials none of these names itself: the seam maps each to the fake forge's
//! listener, and records the lookup, so a cell can say a request reached the forge through the
//! proxy and not around it.

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
    /// The wires a cell runs on this OS: both on Linux, the plain one where Go ignores the CA.
    pub(crate) fn here() -> &'static [Wire] {
        if cfg!(target_os = "macos") {
            &[Wire::Http]
        } else {
            &[Wire::Https, Wire::Http]
        }
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Wire::Https => "https",
            Wire::Http => "http",
        }
    }
}

/// **A GitLab the fake forge answers as.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gitlab {
    /// `gitlab.com`: the plain wire only.
    Com,
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
                    scenario.names.origin(name, 443, at);
                }
            }
            Wire::Http => {
                let at = fake.http_addr();
                for name in ["api.github.localhost", "gitlab.com", SELF_HOSTED_GITLAB] {
                    scenario.names.origin(name, 80, at);
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
            Wire::Http => vec!["api.github.localhost", "gitlab.com", SELF_HOSTED_GITLAB],
        }
    }

    /// The API path prefix a GitHub client's requests carry on this wire: `gh` asks a GHES-shaped
    /// host under `/api/v3`, and `github.localhost` at the root.
    pub(crate) fn github_prefix(&self) -> &'static str {
        match self.wire {
            Wire::Https => "/api/v3",
            Wire::Http => "",
        }
    }

    /// The name `gh` asks, and the port, on this wire.
    pub(crate) fn github_name(&self) -> (&'static str, u16) {
        match self.wire {
            Wire::Https => (GITHUB_SHAPED_HOST, 443),
            Wire::Http => ("api.github.localhost", 80),
        }
    }

    /// The name `glab` asks for `which`, and the port, on this wire.
    pub(crate) fn gitlab_name(&self, which: Gitlab) -> (&'static str, u16) {
        let port = if self.wire == Wire::Https { 443 } else { 80 };
        match which {
            Gitlab::Com => ("gitlab.com", port),
            Gitlab::SelfHosted => (SELF_HOSTED_GITLAB, port),
        }
    }

    /// The environment a `gh` is run with, as `VAR=value ` words for a shell command: its host,
    /// the token the chat is handed, the CA bundle where the wire needs it, and settings that
    /// keep it from asking, updating or writing outside the project.
    pub(crate) fn gh(&self, token: &str) -> String {
        let host = match self.wire {
            Wire::Https => GITHUB_SHAPED_HOST,
            Wire::Http => "github.localhost",
        };
        // `gh` reads `GH_ENTERPRISE_TOKEN` for every host but github.com's and `github.localhost`'s.
        format!(
            "GH_HOST={host} GH_TOKEN={token} GH_ENTERPRISE_TOKEN={token} {} \
             GH_CONFIG_DIR='{}/gh' GH_NO_UPDATE_NOTIFIER=1 GH_NO_EXTENSION_UPDATE_NOTIFIER=1 \
             GH_PROMPT_DISABLED=1 NO_COLOR=1",
            self.trust(),
            self.config.display()
        )
    }

    /// The environment a `glab` is run with, for `which`: the same kind of words as [`Self::gh`].
    pub(crate) fn glab(&self, which: Gitlab, token: &str) -> String {
        let (name, _) = self.gitlab_name(which);
        let host = match self.wire {
            Wire::Https => name.to_owned(),
            Wire::Http => format!("http://{name}"),
        };
        format!(
            "GITLAB_HOST={host} GITLAB_TOKEN={token} {} GLAB_CONFIG_DIR='{}/glab' \
             GLAB_CHECK_UPDATE=false NO_PROMPT=1 NO_COLOR=1",
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
/// through the chat's proxy and the resolver seam, and merges a pull request with the token the
/// chat was handed: the forge saw that token and no other. On Linux over HTTPS under the test
/// CA, and over plain HTTP; on macOS over plain HTTP, where Go ignores the CA.
#[cfg(unix)]
#[test]
fn gh_merge_reaches_the_fake_forge_under_the_wrap() {
    use super::host::{Dialect, Homes, Protection};
    if !available("gh") {
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
        forge.fake.add_pull_request("o/r", 7, HEAD);
        let merge = format!(
            "{} gh api -X PUT repos/o/r/pulls/7/merge -f merge_method=squash -f sha={HEAD} 2>&1; \
             echo \"gh exit $?\"",
            forge.gh(GH_TOKEN)
        );
        let chat = scenario.starts_holding(&running(&[("merge", merge)]), forge.grants());
        scenario.until_played(chat, &["merge"]);
        let said = super::stdout_of(&scenario, chat, "merge");
        let wire = wire.word();
        assert!(
            said.contains("gh exit 0") && said.contains("\"merged\":true"),
            "{wire}: gh did not merge: {said}\n{}",
            scenario.said()
        );
        reached_through_the_proxy(&scenario, chat, forge.github_name());

        assert_eq!(
            forge
                .fake
                .merged(purlis_scenarios::net::Forge::GitHub, "o/r", 7),
            Some(purlis_scenarios::net::MERGE_SHA.to_owned()),
            "{wire}: the fake forge did not merge"
        );
        assert_eq!(
            forge.fake.tokens_seen(),
            vec![GH_TOKEN.to_owned()],
            "{wire}: the forge saw a credential other than the chat's own"
        );
        let seen = forge.fake.requests();
        let put = seen
            .iter()
            .find(|request| request.method == "PUT")
            .unwrap_or_else(|| panic!("{wire}: no merge request recorded: {seen:?}"));
        assert_eq!(
            put.path,
            format!("{}/repos/o/r/pulls/7/merge", forge.github_prefix()),
            "{wire}"
        );
        assert!(
            put.body.contains(HEAD),
            "{wire}: the merge named no head: {}",
            put.body
        );
        if wire == "https" {
            assert_eq!(put.sni.as_deref(), Some(GITHUB_SHAPED_HOST), "{wire}");
        }
    }
}

/// **`glab` reaches the fake forge from inside a sandboxed chat** (T1.1-50), as `gh` does: a
/// self-hosted GitLab on both wires, and `gitlab.com` itself on the plain one, each merged with
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
        let mut which = vec![(Gitlab::SelfHosted, 4_u64)];
        if wire == Wire::Http {
            which.push((Gitlab::Com, 3));
        }
        let mut calls = Vec::new();
        for (gitlab, iid) in &which {
            forge
                .fake
                .add_merge_request("group/project", 42, *iid, HEAD);
            calls.push((
                format!("merge-{iid}"),
                format!(
                    "{} glab api projects/group%2Fproject/merge_requests/{iid}/merge \
                     --method PUT -f sha={HEAD} 2>&1; echo \"glab exit $?\"",
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
            let said = super::stdout_of(&scenario, chat, &format!("merge-{iid}"));
            assert!(
                said.contains("glab exit 0") && said.contains("\"merged\"")
                    || said.contains("glab exit 0") && said.contains("\"state\":\"merged\""),
                "{wire_word}: glab did not merge !{iid}: {said}\n{}",
                scenario.said()
            );
            reached_through_the_proxy(&scenario, chat, forge.gitlab_name(*gitlab));
            assert_eq!(
                forge
                    .fake
                    .merged(purlis_scenarios::net::Forge::GitLab, "group/project", *iid),
                Some(purlis_scenarios::net::MERGE_SHA.to_owned()),
                "{wire_word}: the fake forge did not merge !{iid}"
            );
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

/// **A refusal the fake forge is asked for reaches the chat in the forge's own words**, and
/// nothing is merged (T1.1-50): the ruleset's `405` through `gh`, on every wire this OS runs.
#[cfg(unix)]
#[test]
fn a_ruleset_refusal_reaches_a_chats_gh_in_the_forges_own_words() {
    use super::host::{Dialect, Homes, Protection};
    if !available("gh") {
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
        forge.fake.add_pull_request("o/r", 7, HEAD);
        forge.fake.refuse_merge(
            purlis_scenarios::net::Forge::GitHub,
            "o/r",
            7,
            purlis_scenarios::net::Refusal::Ruleset,
        );
        let merge = format!(
            "{} gh api -X PUT repos/o/r/pulls/7/merge -f sha={HEAD} 2>&1; echo \"gh exit $?\"",
            forge.gh(GH_TOKEN)
        );
        let chat = scenario.starts_holding(&running(&[("merge", merge)]), forge.grants());
        scenario.until_played(chat, &["merge"]);
        let said = super::stdout_of(&scenario, chat, "merge");
        assert!(
            !said.contains("gh exit 0") && said.contains("Repository rule violations"),
            "{}: gh was not refused as a ruleset refuses: {said}\n{}",
            wire.word(),
            scenario.said()
        );
        assert_eq!(
            forge
                .fake
                .merged(purlis_scenarios::net::Forge::GitHub, "o/r", 7),
            None,
            "{}: a refused merge merged",
            wire.word()
        );
        assert!(
            forge
                .fake
                .requests()
                .iter()
                .any(|request| request.status == 405),
            "{}: the fake forge never refused",
            wire.word()
        );
    }
}
