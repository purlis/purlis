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
//! assert_eq!(forge.fake.tokens_seen(), [CHATS_TOKEN]); // every token on every request
//! ```
//!
//! **What `tokens_seen()` proves, and what it does not.** It lists every credential any request
//! carried (every `Authorization` value, `PRIVATE-TOKEN`, `JOB-TOKEN`, and a token in the query
//! or body). A cell puts the chat's token in its command's own words, as a declared tool's value
//! would be put, so `[CHATS_TOKEN]` says the client sent that token and **no other: not the
//! decoy a cell plants for the person** (`person`: in the host process's environment, and in a
//! `gh` `hosts.yml` and a `glab` `config.yml` where a real home keeps them). It does **not** say
//! purlis's own route handed the chat that token: the scenario host has no vault and no declared
//! tool, and the App mint reaches github.com alone, so no cell here runs `purlis secret exec`.
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
                    scenario.names.origin(name, 443, at);
                }
            }
            Wire::Http => {
                let at = fake.http_addr();
                scenario.names.origin(SELF_HOSTED_GITLAB, 80, at);
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

    /// The environment a `gh` is run with, as `VAR=value ` words for a shell command, **with no
    /// token and no config folder of its own**: its host, the CA bundle, and settings that keep
    /// it from asking or updating. What a `gh` finds to sign in with is then what the chat's
    /// environment and home hold. The HTTPS wire alone.
    pub(crate) fn gh_route(&self) -> String {
        assert_eq!(
            self.wire,
            Wire::Https,
            "gh reaches the fake forge over HTTPS only"
        );
        format!(
            "GH_HOST={GITHUB_SHAPED_HOST} {} GH_NO_UPDATE_NOTIFIER=1 \
             GH_NO_EXTENSION_UPDATE_NOTIFIER=1 GH_PROMPT_DISABLED=1 NO_COLOR=1",
            self.trust(),
        )
    }

    /// [`Self::gh_route`], handed `token` the way a declared tool's value is (`gh` reads
    /// `GH_ENTERPRISE_TOKEN` for a host that is not github.com), and a config folder of its own.
    pub(crate) fn gh(&self, token: &str) -> String {
        format!(
            "GH_TOKEN={token} GH_ENTERPRISE_TOKEN={token} {} GH_CONFIG_DIR='{}/gh'",
            self.gh_route(),
            self.config.display()
        )
    }

    /// The environment a `glab` is run with for `which`, **with no token and no config folder of
    /// its own** ([`Self::gh_route`]'s kind). `telemetry_off` sets `GLAB_SEND_TELEMETRY=false`:
    /// without it `glab` posts usage data, token included, to its instance
    /// (`glab_posts_usage_data_with_the_token_unless_told_not_to`).
    pub(crate) fn glab_route(&self, which: Gitlab, telemetry_off: bool) -> String {
        let (name, _) = self.gitlab_name(which);
        let protocol = match (self.wire, &self.bundle) {
            (Wire::Http, _) => "GLAB_API_PROTOCOL=http".to_owned(),
            (Wire::Https, Some(bundle)) => format!("GLAB_CA_CERT='{}'", bundle.display()),
            (Wire::Https, None) => String::new(),
        };
        let telemetry = if telemetry_off {
            "GLAB_SEND_TELEMETRY=false"
        } else {
            ""
        };
        format!(
            "GITLAB_HOST={name} {protocol} {} GLAB_CHECK_UPDATE=false GLAB_NO_PROMPT=1 \
             {telemetry} NO_COLOR=1",
            self.trust(),
        )
    }

    /// [`Self::glab_route`] with telemetry off, handed `token` the way a declared tool's value
    /// is, and a config folder of its own.
    pub(crate) fn glab(&self, which: Gitlab, token: &str) -> String {
        format!(
            "GITLAB_TOKEN={token} {} GLAB_CONFIG_DIR='{}/glab'",
            self.glab_route(which, true),
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

/// The matrix row these cells' leg records carry: the ticket that built the fake forge. Its
/// rows, §3.8's holds and merges, are covered by ticket 16's cells, which use this one.
#[cfg(test)]
const ROW: &str = "T1.1-50";

/// The token a cell puts in a chat's `gh` command, as a declared tool or a mint would hand it.
#[cfg(test)]
const GH_TOKEN: &str = "chat-gh-token-3f9a";

/// The token a cell puts in a chat's `glab` command.
#[cfg(test)]
const GLAB_TOKEN: &str = "chat-glab-token-8c21";

/// Why the macOS `gh` cell is not run: the reason its `not_run` record carries and the gap
/// `coverage.json` declares for it (`the_macos_gh_gap_is_declared_with_its_reason`).
#[cfg(test)]
pub(crate) const GH_MACOS_GAP: &str = "gh is Go: on macOS it ignores the test CA, has no CA \
     setting, and speaks http only to github.localhost, which purlis refuses as this machine \
     (T1.1-50's fallback)";

/// **The person's own login, as a decoy** a chat's client must never present: where a real
/// machine keeps it, in the host process's environment and in `gh`'s `hosts.yml` and `glab`'s
/// `config.yml` under the person's home. Each value is its own string, so a request that
/// carries one says which place it was read from.
#[cfg(test)]
mod person {
    use std::path::Path;

    pub(super) const GH_ENV: &str = "person-gh-env-decoy-71aa";
    pub(super) const GH_FILE: &str = "person-gh-file-decoy-93cd";
    pub(super) const GITLAB_ENV: &str = "person-glab-env-decoy-52bb";
    pub(super) const GITLAB_FILE: &str = "person-glab-file-decoy-04ef";

    /// The variables of the host process that hold the person's tokens.
    pub(super) const ENV: [(&str, &str); 4] = [
        ("GH_TOKEN", GH_ENV),
        ("GITHUB_TOKEN", GH_ENV),
        ("GH_ENTERPRISE_TOKEN", GH_ENV),
        ("GITLAB_TOKEN", GITLAB_ENV),
    ];

    /// Every decoy, wherever it is kept.
    pub(super) const ALL: [&str; 4] = [GH_ENV, GH_FILE, GITLAB_ENV, GITLAB_FILE];

    /// Writes `gh`'s `hosts.yml` and `glab`'s `config.yml` under `home`, for the hosts the
    /// cells use.
    pub(super) fn plant(home: &Path) {
        let config = home.join(".config");
        std::fs::create_dir_all(config.join("gh")).expect("gh's folder");
        std::fs::create_dir_all(config.join("glab-cli")).expect("glab's folder");
        std::fs::write(
            config.join("gh/hosts.yml"),
            format!(
                "forge.test:\n    user: person\n    oauth_token: {GH_FILE}\n    \
                 git_protocol: https\ngithub.com:\n    user: person\n    oauth_token: {GH_FILE}\n"
            ),
        )
        .expect("the person's gh login");
        std::fs::write(
            config.join("glab-cli/config.yml"),
            format!(
                "hosts:\n    git.corp.test:\n        token: {GITLAB_FILE}\n        \
                 api_host: git.corp.test\n        api_protocol: https\n        \
                 git_protocol: https\n"
            ),
        )
        .expect("the person's glab login");
        // `glab` refuses a config that others can read, as it does a real one.
        use std::os::unix::fs::PermissionsExt as _;
        for file in ["gh/hosts.yml", "glab-cli/config.yml"] {
            std::fs::set_permissions(config.join(file), std::fs::Permissions::from_mode(0o600))
                .expect("the login file is private");
        }
    }
}

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

/// What `program args` prints, run in this process's own environment and home (the child a
/// scenario runs in, with the person's decoys), less the variables named in `without`.
#[cfg(test)]
fn run_here(program: &str, args: &[&str], without: &[&str], with: &[(&str, &str)]) -> String {
    let mut command = std::process::Command::new(program);
    command.args(args);
    for name in without {
        command.env_remove(name);
    }
    for (name, value) in with {
        command.env(name, value);
    }
    let out = purlis_core::forklock::output(&mut command)
        .unwrap_or_else(|why| panic!("{program} did not run: {why}"));
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A chat's turn that runs each `(step, command)`.
#[cfg(test)]
fn running(calls: &[(&str, String)]) -> serde_json::Value {
    serde_json::json!({"turns": [{"prompt": "go", "calls": calls.iter().map(|(step, run)| {
        serde_json::json!({"step": step, "tool": "bash", "run": run})
    }).collect::<Vec<_>>()}]})
}

/// This leg's record of `test`, for the report: a pass, or the not run of a declared gap.
#[cfg(test)]
fn record(test: &str, verdict: purlis_scenarios::model::ReportVerdict, reason: &str) {
    super::report::write(&[super::a_record_of(
        ROW,
        test,
        Some(purlis_scenarios::model::Dialect::Codex),
        Some(purlis_scenarios::model::Protection::Full),
        verdict,
        reason,
    )])
    .expect("the leg's report");
}

/// The distinct tokens of the requests whose path ends with `end`.
#[cfg(test)]
fn tokens_on(forge: &ForgeHere, end: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    for request in forge.fake.requests() {
        if request.path.ends_with(end) {
            for credential in request.credentials() {
                if !tokens.contains(&credential.token) {
                    tokens.push(credential.token);
                }
            }
        }
    }
    tokens
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

/// **A merge the guard refused before it ran**: the chat's result for `step` is the hook's
/// refusal (`pretooluse`), not the command's output, and the refusal is the merge hold's own
/// words (`mergehold::refusal`). Returns the refusal's text.
#[cfg(test)]
fn refused_as_a_merge(scenario: &Scenario, chat: u32, step: &str, what: &str) -> String {
    let result = scenario
        .result_of(chat, step)
        .unwrap_or_else(|| panic!("{what}: no result for {step}\n{}", scenario.said()));
    assert_eq!(
        result["hook"],
        "pretooluse",
        "{what}: the merge was not refused at the hook: {result}\n{}",
        scenario.said()
    );
    let reason = result["reason"].as_str().unwrap_or_default().to_owned();
    assert!(
        reason.contains("purlis holds merges") && reason.contains("Nothing was merged"),
        "{what}: the refusal is not the merge hold's: {result}"
    );
    reason
}

/// **`gh` reaches the fake forge from inside a sandboxed chat** (T1.1-50), under the OS wrap,
/// through the chat's proxy and the resolver seam: it reads a pull request with the token the
/// cell put in its command, over HTTPS under the test CA, and the forge saw that token on every
/// request and no other. Linux: on macOS Go ignores the test CA and `gh` has no other way in
/// (module docs), so there it is reported not run, the ticket's stated fallback, which
/// `coverage.json` declares as a gap.
#[cfg(unix)]
#[test]
fn gh_merge_reaches_the_fake_forge_under_the_wrap() {
    use super::host::{Dialect, Homes, Protection};
    use purlis_scenarios::model::ReportVerdict;
    const TEST: &str = "gh_merge_reaches_the_fake_forge_under_the_wrap";
    if cfg!(target_os = "macos") {
        record(TEST, ReportVerdict::NotRun, GH_MACOS_GAP);
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
        "the forge saw a token other than the one the cell put in the chat's command"
    );
    let seen = forge.fake.requests();
    let get = seen
        .iter()
        .find(|request| request.method == "GET")
        .unwrap_or_else(|| panic!("no read recorded: {seen:?}"));
    assert_eq!(get.path, "/api/v3/repos/o/r/pulls/7");
    assert_eq!(get.sni.as_deref(), Some(GITHUB_SHAPED_HOST));
    record(
        TEST,
        ReportVerdict::Pass,
        "gh read a pull request through the proxy",
    );
}

/// **`glab` reaches the fake forge from inside a sandboxed chat** (T1.1-50), as `gh` does: a
/// self-hosted GitLab on both wires, each read with the token the cell put in its command, and
/// no usage data sent (`GLAB_SEND_TELEMETRY=false`).
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
        forge.fake.add_merge_request("group/project", 42, 4, HEAD);
        let read = format!(
            "{} glab api projects/group%2Fproject/merge_requests/4 2>&1; echo \"glab exit $?\"",
            forge.glab(Gitlab::SelfHosted, GLAB_TOKEN)
        );
        let chat = scenario.starts_holding(&running(&[("read", read)]), forge.grants());
        scenario.until_played(chat, &["read"]);
        let wire_word = wire.word();
        let said = super::stdout_of(&scenario, chat, "read");
        assert!(
            said.contains("glab exit 0") && said.contains(HEAD),
            "{wire_word}: glab did not read !4: {said}\n{:?}\n{}",
            scenario.result_of(chat, "read"),
            scenario.said()
        );
        reached_through_the_proxy(&scenario, chat, forge.gitlab_name(Gitlab::SelfHosted));
        assert_eq!(
            forge.fake.tokens_seen(),
            vec![GLAB_TOKEN.to_owned()],
            "{wire_word}: the forge saw a token other than the one the cell put in the chat's command"
        );
        let requests = forge.fake.requests();
        assert!(
            requests.iter().any(|request| request
                .host
                .as_deref()
                .is_some_and(|host| host.starts_with(SELF_HOSTED_GITLAB))),
            "{wire_word}: no request carried the Host {SELF_HOSTED_GITLAB}: {requests:?}"
        );
        assert!(
            requests
                .iter()
                .all(|request| !request.path.contains("usage_data")),
            "{wire_word}: glab sent usage data with GLAB_SEND_TELEMETRY=false: {requests:?}"
        );
    }
    record(
        "glab_merge_reaches_the_fake_forge_under_the_wrap",
        purlis_scenarios::model::ReportVerdict::Pass,
        "glab read a merge request through the proxy on both wires",
    );
}

/// **A chat's `gh` and `glab` present the chat's token, never the person's** (T1.1-50), with
/// the person's login all around: decoy tokens in the host process's environment and in a `gh`
/// `hosts.yml` and a `glab` `config.yml` under the home, and a command that gives the client no
/// config folder of its own. Three things are asserted:
///
/// 1. **The decoys are readable** where purlis does not stand between (the controls): `glab`,
///    in this process's own environment and home, sends the environment's decoy to a fake
///    forge and prints the file's (`glab config get token`); for `gh` the control is only that
///    the file and the variables are there and readable here (`gh auth token` printed no token
///    for the planted `hosts.yml`, so its own reading is not shown). So a decoy missing from the
///    chat means purlis kept it away.
/// 2. **The chat sees none**: its environment and the login files print no decoy (the sandbox
///    denies `hosts.yml` and `config.yml`, `sandbox::CREDENTIAL_FILES`).
/// 3. **The forge saw the chat's token and no decoy**: a client given no token and no config
///    folder (the chat's HOME holds only the denied decoy files) sends the forge nothing, and a
///    client given the chat's token and a config folder of its own presents that token alone,
///    every credential of every request counted. (Finding: without a config folder of its own
///    a chat's `glab` stops at reading the person's denied `config.yml`, exit 2 "permission
///    denied", token or no token.)
///
/// What this does not prove (module docs): that purlis's own route handed the chat its token.
#[cfg(unix)]
#[test]
fn a_chats_gh_and_glab_present_the_chats_token_and_never_the_persons() {
    use super::host::{Dialect, Homes, Protection};
    let with_gh = !cfg!(target_os = "macos");
    if !available("glab") || (with_gh && !available("gh")) {
        return;
    }
    let Some(homes) = Homes::of_this_scenario_with(&person::ENV) else {
        return;
    };
    person::plant(&homes.user);

    // The controls: without purlis between, the clients read the person's login.
    let own_tokens: Vec<&str> = person::ENV.iter().map(|(name, _)| *name).collect();
    let stored = run_here(
        "glab",
        &["config", "get", "token", "--host", SELF_HOSTED_GITLAB],
        &own_tokens,
        &[],
    );
    assert!(
        stored.contains(person::GITLAB_FILE),
        "control: glab does not read the planted config.yml, so the cell proves nothing: {stored}"
    );
    if with_gh {
        // `gh auth token` printed "no oauth token found" for the planted `hosts.yml` on CI, so
        // gh's own reading of it is not shown: the control is that the file and the variables
        // are where the person's are, and readable by this process, which is not the chat's.
        let hosts = std::fs::read_to_string(homes.user.join(".config/gh/hosts.yml"))
            .expect("the planted hosts.yml is readable outside the chat");
        assert!(
            hosts.contains(person::GH_FILE),
            "control: hosts.yml does not hold the decoy: {hosts}"
        );
        assert_eq!(
            std::env::var("GH_TOKEN").as_deref(),
            Ok(person::GH_ENV),
            "control: the host process does not hold the person's GH_TOKEN"
        );
    }
    let outside = FakeForge::start(None).expect("a fake forge of the control's own");
    outside.add_merge_request("group/project", 42, 4, HEAD);
    let host = outside.http_addr().to_string();
    let sent = run_here(
        "glab",
        &["api", "projects/group%2Fproject/merge_requests/4"],
        &[],
        &[
            ("GITLAB_HOST", host.as_str()),
            ("GLAB_API_PROTOCOL", "http"),
            ("GLAB_SEND_TELEMETRY", "false"),
            ("GLAB_CHECK_UPDATE", "false"),
            ("GLAB_NO_PROMPT", "1"),
        ],
    );
    assert_eq!(
        outside.tokens_seen(),
        vec![person::GITLAB_ENV.to_owned()],
        "control: glab, in the host's own environment, does not present the environment's \
         token, so the cell proves nothing: {sent}"
    );

    for &wire in Wire::here() {
        let Some(scenario) =
            Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
        else {
            return;
        };
        let forge = ForgeHere::start(&scenario, wire);
        forge.fake.add_merge_request("group/project", 42, 4, HEAD);
        forge.fake.add_merge_request("group/project", 42, 5, HEAD);
        let gh_too = with_gh && wire == Wire::Https;
        if gh_too {
            forge.fake.add_pull_request("o/r", 6, HEAD);
            forge.fake.add_pull_request("o/r", 7, HEAD);
        }
        let mut calls: Vec<(&str, String)> = vec![
            (
                "env",
                "printenv GH_TOKEN GITHUB_TOKEN GH_ENTERPRISE_TOKEN GITLAB_TOKEN; echo env-done"
                    .to_owned(),
            ),
            (
                "files",
                "cat \"$HOME/.config/glab-cli/config.yml\" \"$HOME/.config/gh/hosts.yml\" 2>&1; \
                 echo files-done"
                    .to_owned(),
            ),
            (
                "glab-given-no-token",
                format!(
                    "{} glab api projects/group%2Fproject/merge_requests/4 2>&1; \
                     echo \"glab exit $?\"",
                    forge.glab_route(Gitlab::SelfHosted, true)
                ),
            ),
            (
                "glab-given-the-chats",
                format!(
                    "{} glab api projects/group%2Fproject/merge_requests/5 2>&1; \
                     echo \"glab exit $?\"",
                    forge.glab(Gitlab::SelfHosted, GLAB_TOKEN)
                ),
            ),
        ];
        if gh_too {
            calls.push((
                "gh-given-no-token",
                format!(
                    "{} gh api repos/o/r/pulls/6 2>&1; echo \"gh exit $?\"",
                    forge.gh_route()
                ),
            ));
            calls.push((
                "gh-given-the-chats",
                format!(
                    "{} gh api repos/o/r/pulls/7 2>&1; echo \"gh exit $?\"",
                    forge.gh(GH_TOKEN)
                ),
            ));
        }
        let chat = scenario.starts_holding(&running(&calls), forge.grants());
        let steps: Vec<&str> = calls.iter().map(|(step, _)| *step).collect();
        scenario.until_played(chat, &steps);
        let at = wire.word();
        for step in ["env", "files"] {
            let said = super::stdout_of(&scenario, chat, step);
            for decoy in person::ALL {
                assert!(
                    !said.contains(decoy),
                    "{at}: the chat's `{step}` shows the person's decoy {decoy}: {said}\n{}",
                    scenario.said()
                );
            }
            assert!(
                said.contains(&format!(
                    "{}-done",
                    if step == "env" { "env" } else { "files" }
                )),
                "{at}: the chat's `{step}` did not run: {said}"
            );
        }
        // A client given no token and no config folder finds only the person's login, which the
        // sandbox denies: `glab` stops at reading its config ("permission denied", exit 2), so
        // it neither reads the merge request nor sends the forge anything.
        let bare = super::stdout_of(&scenario, chat, "glab-given-no-token");
        assert!(
            !bare.contains(HEAD) && !bare.contains("glab exit 0"),
            "{at}: glab, given no token, read !4 with a credential of its own: {bare}\n{}",
            scenario.said()
        );
        assert!(
            tokens_on(&forge, "merge_requests/4").is_empty(),
            "{at}: a request of glab's with no token given carried one: {:?}",
            forge.fake.requests()
        );
        let chats = super::stdout_of(&scenario, chat, "glab-given-the-chats");
        assert!(
            chats.contains("glab exit 0") && chats.contains(HEAD),
            "{at}: glab, given the chat's token, did not read !5: {chats}\n{}",
            scenario.said()
        );
        reached_through_the_proxy(&scenario, chat, forge.gitlab_name(Gitlab::SelfHosted));
        let mut expected = vec![GLAB_TOKEN.to_owned()];
        assert_eq!(
            tokens_on(&forge, "merge_requests/5"),
            vec![GLAB_TOKEN.to_owned()],
            "{at}: the request glab made with the chat's token carried another as well"
        );
        if gh_too {
            let chats = super::stdout_of(&scenario, chat, "gh-given-the-chats");
            assert!(
                chats.contains("gh exit 0") && chats.contains(HEAD),
                "{at}: gh, given the chat's token, did not read #7: {chats}\n{}",
                scenario.said()
            );
            assert_eq!(
                tokens_on(&forge, "pulls/7"),
                vec![GH_TOKEN.to_owned()],
                "{at}: the request gh made with the chat's token carried another as well"
            );
            assert!(
                tokens_on(&forge, "pulls/6").is_empty(),
                "{at}: a request of gh's with no token given carried one: {:?}",
                forge.fake.requests()
            );
            expected.push(GH_TOKEN.to_owned());
        }
        let mut seen = forge.fake.tokens_seen();
        for decoy in person::ALL {
            assert!(
                !seen.iter().any(|token| token.contains(decoy)),
                "{at}: the forge was presented the person's decoy {decoy}: {seen:?}\n{}",
                scenario.said()
            );
        }
        seen.sort();
        expected.sort();
        assert_eq!(
            seen, expected,
            "{at}: a token reached the forge that the cell did not put in the chat's command"
        );
    }
    record(
        "a_chats_gh_and_glab_present_the_chats_token_and_never_the_persons",
        purlis_scenarios::model::ReportVerdict::Pass,
        "no decoy of the person's login reached the chat or the forge; the controls read them",
    );
}

/// **`glab` posts usage data, the chat's token with it, unless told not to** (T1.1-50): a
/// finding, pinned. Run without `GLAB_SEND_TELEMETRY=false`, a chat's `glab` sends
/// `POST /api/v4/usage_data/track_event` to its instance with its token. It is the instance's
/// own, so it is not a leak to a third party, but a route that hands a chat a token must switch
/// it off (#7230 sets it); every other cell sets it, and asserts none arrived.
#[cfg(unix)]
#[test]
fn glab_posts_usage_data_with_the_token_unless_told_not_to() {
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
        let read = format!(
            "GITLAB_TOKEN={GLAB_TOKEN} {} GLAB_CONFIG_DIR='{}/glab' glab api \
             projects/group%2Fproject/merge_requests/4 2>&1; echo \"glab exit $?\"",
            forge.glab_route(Gitlab::SelfHosted, false),
            forge.config.display()
        );
        let chat = scenario.starts_holding(&running(&[("read", read)]), forge.grants());
        scenario.until_played(chat, &["read"]);
        let at = wire.word();
        assert!(
            super::stdout_of(&scenario, chat, "read").contains("glab exit 0"),
            "{at}: the control read failed: {:?}\n{}",
            scenario.result_of(chat, "read"),
            scenario.said()
        );
        let usage: Vec<_> = forge
            .fake
            .requests()
            .into_iter()
            .filter(|request| request.path.contains("usage_data"))
            .collect();
        assert!(
            !usage.is_empty(),
            "{at}: glab sent no usage data without GLAB_SEND_TELEMETRY=false, so the flag the \
             other cells set suppresses nothing: {:?}",
            forge.fake.requests()
        );
        for request in &usage {
            assert_eq!(request.method, "POST", "{at}: {request:?}");
            assert!(
                request
                    .credentials()
                    .iter()
                    .any(|credential| credential.token == GLAB_TOKEN),
                "{at}: the usage data carried no token: {request:?}"
            );
        }
    }
    record(
        "glab_posts_usage_data_with_the_token_unless_told_not_to",
        purlis_scenarios::model::ReportVerdict::Pass,
        "without GLAB_SEND_TELEMETRY=false glab posts usage data with the token (finding for #7230)",
    );
}

/// **A merge a chat tries is held before it reaches the fake forge** (T1.1-50): the same chat
/// reads a merge request (the control: a recorded `200` for the chat's token, through its
/// proxy) and then tries to merge it by each route, `glab api … merge` and `glab mr merge`, and
/// each is refused at the hook with the merge hold's own words, so the forge records the read
/// and no merge. On every wire this OS runs. (`glab mr merge 4 2>&1` is read as a merge of two
/// words and so as unreadable: the `mr merge` commands here carry no redirect.)
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
    let mut observed: Vec<String> = Vec::new();
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
        let api = format!(
            "{env} glab api projects/group%2Fproject/merge_requests/4/merge --method PUT \
             -f sha={HEAD} 2>&1; echo \"glab exit $?\""
        );
        let mr = format!("{env} glab mr merge 4 -R group/project --yes; echo \"glab exit $?\"");
        let chat = scenario.starts_holding(
            &running(&[("read", read), ("merge-api", api), ("merge-mr", mr)]),
            forge.grants(),
        );
        scenario.until_played(chat, &["read", "merge-api", "merge-mr"]);
        let at = wire.word();
        assert!(
            super::stdout_of(&scenario, chat, "read").contains("glab exit 0"),
            "{at}: the control read failed: {:?}\n{}",
            scenario.result_of(chat, "read"),
            scenario.said()
        );
        reached_through_the_proxy(&scenario, chat, forge.gitlab_name(Gitlab::SelfHosted));
        let requests = forge.fake.requests();
        let read_seen = requests
            .iter()
            .find(|request| request.method == "GET" && request.status == 200)
            .unwrap_or_else(|| panic!("{at}: no 200 read recorded: {requests:?}"));
        assert!(
            read_seen.path.ends_with("merge_requests/4")
                && read_seen
                    .credentials()
                    .iter()
                    .any(|credential| credential.token == GLAB_TOKEN),
            "{at}: the read was not the chat's, with its token: {read_seen:?}"
        );
        // The merge by the API cannot be read for its request: no ask.
        let unreadable = refused_as_a_merge(&scenario, chat, "merge-api", at);
        assert!(
            unreadable.contains("cannot tell which request"),
            "{at}: `glab api … merge` was not the merge-unreadable refusal: {unreadable}"
        );
        // The merge by `glab mr merge 4` names its request: the hold reads it, and asks the
        // person where the app can read the forge (no app login on the fake, so it may not).
        let named = refused_as_a_merge(&scenario, chat, "merge-mr", at);
        assert!(
            !named.contains("cannot tell which request"),
            "{at}: `glab mr merge 4` was read as unreadable: {named}"
        );
        observed.push(format!(
            "{at}: glab mr merge refused as {}",
            if named.contains("Nobody was asked") {
                "merge-not-asked (no ask raised)"
            } else {
                "merge-hold (an ask was raised)"
            }
        ));
        for step in ["merge-api", "merge-mr"] {
            assert!(
                !super::stdout_of(&scenario, chat, step).contains("glab exit"),
                "{at}: the {step} command ran"
            );
        }
        let after = forge.fake.requests();
        assert_eq!(
            after.len(),
            requests.len(),
            "{at}: a refused merge sent the forge something: {after:?}"
        );
        assert!(
            after.iter().all(|request| request.method == "GET"),
            "{at}: a non-read reached the forge: {after:?}"
        );
        assert_eq!(
            forge
                .fake
                .merged(purlis_scenarios::net::Forge::GitLab, "group/project", 4),
            None,
            "{at}: a chat's merge merged"
        );
    }
    record(
        "a_chats_merge_is_held_before_it_reaches_the_fake_forge",
        purlis_scenarios::model::ReportVerdict::Pass,
        &format!(
            "glab api merge and glab mr merge were refused at the hook; the forge saw reads \
             only ({})",
            observed.join("; ")
        ),
    );
}

/// **A chat's `gh` merge is held before it reaches the fake forge** (T1.1-50), as `glab`'s is:
/// `gh api -X PUT …/merge` and `gh pr merge 7` are each refused at the hook. Linux: `gh` is not
/// run on macOS (module docs). The refused `gh pr merge` never reaches the forge, so the fake
/// answers no GraphQL: a merge that did would be a `404` naming `POST /api/graphql`.
#[cfg(unix)]
#[test]
fn a_chats_gh_merge_is_held_before_it_reaches_the_fake_forge() {
    use super::host::{Dialect, Homes, Protection};
    if cfg!(target_os = "macos") || !available("gh") {
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
    let env = forge.gh(GH_TOKEN);
    let read = format!("{env} gh api repos/o/r/pulls/7 2>&1; echo \"gh exit $?\"");
    let api = format!(
        "{env} gh api -X PUT repos/o/r/pulls/7/merge -f sha={HEAD} 2>&1; echo \"gh exit $?\""
    );
    let pr = format!("{env} gh pr merge 7 -R o/r --squash; echo \"gh exit $?\"");
    let chat = scenario.starts_holding(
        &running(&[("read", read), ("merge-api", api), ("merge-pr", pr)]),
        forge.grants(),
    );
    scenario.until_played(chat, &["read", "merge-api", "merge-pr"]);
    assert!(
        super::stdout_of(&scenario, chat, "read").contains("gh exit 0"),
        "the control read failed: {:?}\n{}",
        scenario.result_of(chat, "read"),
        scenario.said()
    );
    reached_through_the_proxy(&scenario, chat, forge.github_name());
    let requests = forge.fake.requests();
    assert!(
        requests.iter().any(|request| request.method == "GET"
            && request.status == 200
            && request.path.ends_with("pulls/7")
            && request
                .credentials()
                .iter()
                .any(|credential| credential.token == GH_TOKEN)),
        "no 200 read with the chat's token was recorded: {requests:?}"
    );
    let unreadable = refused_as_a_merge(&scenario, chat, "merge-api", "gh api");
    assert!(
        unreadable.contains("cannot tell which request"),
        "`gh api -X PUT … merge` was not the merge-unreadable refusal: {unreadable}"
    );
    let named = refused_as_a_merge(&scenario, chat, "merge-pr", "gh pr merge");
    assert!(
        !named.contains("cannot tell which request"),
        "`gh pr merge 7` was read as unreadable: {named}"
    );
    for step in ["merge-api", "merge-pr"] {
        assert!(
            !super::stdout_of(&scenario, chat, step).contains("gh exit"),
            "the {step} command ran"
        );
    }
    let after = forge.fake.requests();
    assert!(
        after.iter().all(|request| request.method == "GET"),
        "a non-read reached the forge: {after:?}"
    );
    assert_eq!(
        forge
            .fake
            .merged(purlis_scenarios::net::Forge::GitHub, "o/r", 7),
        None,
        "a chat's merge merged"
    );
    record(
        "a_chats_gh_merge_is_held_before_it_reaches_the_fake_forge",
        purlis_scenarios::model::ReportVerdict::Pass,
        "gh api merge and gh pr merge were refused at the hook; the forge saw reads only",
    );
}

/// **The macOS `gh` gap is declared, with its reason** (T1.1-50): `coverage.json` lists a gap
/// for this ticket on macOS whose reason is the very text the not-run record carries, so the
/// leg's `not_run` is a pinned fallback and a change to one without the other fails here.
#[test]
fn the_macos_gh_gap_is_declared_with_its_reason() {
    let list: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(purlis_scenarios::root().join("coverage.json"))
            .expect("coverage.json"),
    )
    .expect("coverage.json parses");
    let gaps = list["gaps"].as_array().expect("the gaps");
    assert!(
        gaps.iter().any(|gap| gap["os"] == "macos"
            && gap["rows"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row == ROW))
            && gap["reason"] == GH_MACOS_GAP),
        "coverage.json declares no macOS gap for {ROW} with the reason {GH_MACOS_GAP:?}: {gaps:?}"
    );
}
