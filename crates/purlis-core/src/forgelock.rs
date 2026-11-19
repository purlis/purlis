// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No path from a chat to the person's forge login** (#7205, slice 1 of #7133; design
//! `docs/research/forge-credential-lock.md` §6.1; ADR 0067's amendment of 2026-11-19).
//!
//! The merge and release lock is the forge's own rule, with the person's repository-admin role as
//! the one bypass. A chat that holds the person's login is that bypass. A chat on a repo that
//! claims the lock ([`claimed`]) therefore gets, from its environment and the config purlis
//! compiles for it, none of the routes in [`Route::ALL`] that [`Route::closed_by`] says it closes,
//! and what a chat has no mint for it gets no credential at all. Not every route is closed, and
//! [`PROBED`] says per harness which are not: no version is [`Status::Locked`] today.
//!
//! **What is closed here, by route** ([`Route::closed_by`]):
//!
//! - `gh` with no minted token, and gh by its absolute path: `GH_CONFIG_DIR` is an empty folder
//!   ([`env`]), so the real `gh` finds no login in a file either.
//! - git over https: `GIT_CONFIG_GLOBAL` is a file purlis writes, which includes the person's own
//!   (names, signing) and then empties `credential.helper`; `GIT_CONFIG_SYSTEM` is nothing, so no
//!   system helper (Apple's `osxkeychain`) is read; `GIT_CONFIG_COUNT` empties `credential.helper`
//!   once more at the command scope, which outranks a repository's own `.git/config`; and
//!   `GIT_TERMINAL_PROMPT=0`.
//! - ssh agents: `SSH_AUTH_SOCK` is empty and git's ssh is told no agent and only configured
//!   identities ([`ssh_command`]); in Claude Code, whose own sandbox sets git's ssh per command, only the
//!   environment's empty socket applies and its own ssh stays open ([`Route::SshAgent`]).
//! - the purlis browser: `browser_login` to a forge host is refused ([`is_a_forge_host`]).
//! - a harness's forge connector, by the names in [`CONNECTORS`]: Claude Code's tools of each are
//!   denied (`mcp__<name>`, [`claude_denied`]; the server still starts, its tool calls are
//!   refused), Codex's is told not enabled where its config has one ([`connectors`]), opencode's
//!   is `enabled: false` ([`opencode_config`]). A forge server under a name not there is a
//!   residual.
//!
//! **The routes this does not close** are recorded open, per harness, in [`PROBED`], and no harness
//! version is [`Status::Locked`] while one is ([`Probed::open`]): gh's macOS keychain item
//! ([`Route::GhKeychain`]), a credential helper the chat names itself, per command
//! (`git -c credential.helper=…`, `GIT_CONFIG_COUNT=0 git …`, `env -u GIT_CONFIG_COUNT …`;
//! [`Route::GitKeychain`]), a socket the person granted ([`Route::GrantedAgent`]), bare `ssh`
//! reading `~/.ssh/config` ([`Route::BareSsh`]), a claim made by a mint alone, which denies no
//! file ([`Route::MintOnly`]), and, in Claude Code, its own ssh ([`Route::SshAgent`]).
//!
//! **A probe per harness** ([`probe`]) runs each route's command in the environment a chat of
//! that harness is composed with, against a person made for the test, and passes when none returns
//! their identity. Its result is recorded with the harness version ([`PROBED`]); a version not in
//! the record is [`Status::NotLocked`], so the lock fails closed.

use std::path::Path;

use crate::harness::Harness;

/// The ways a chat could reach the person's own forge login (design §6.1), each of which a locked
/// chat has closed or is recorded open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// `gh` with no minted token: the fallback to the person's login.
    GhNoMint,
    /// The real `gh` binary by its absolute path, past purlis's shim.
    GhAbsolutePath,
    /// git over https, with the person's `credential.helper`.
    GitHttps,
    /// gh's macOS keychain item, which is no file.
    GhKeychain,
    /// An ssh agent, named by the environment or by `~/.ssh/config`'s `IdentityAgent`.
    SshAgent,
    /// The purlis browser tools, with a profile logged in to the forge.
    BrowserSession,
    /// A harness's forge connector, which carries the person's OAuth.
    HarnessConnector,
    /// git's own `osxkeychain` helper, named on a command line (`git -c credential.helper=…`),
    /// which outranks every config file and reads the keychain.
    GitKeychain,
    /// An ssh agent socket the person granted (`ssh-agent.persons`), which design §6.1 would have
    /// refused on a locked repo.
    GrantedAgent,
    /// A bare `ssh` the chat runs itself: purlis's route (Codex, opencode) includes the person's
    /// `~/.ssh/config`, whose `IdentityAgent` names their agent.
    BareSsh,
    /// A claim made by a mint alone: it sets this environment but denies no file, so the person's
    /// `gh` config, `~/.ssh` and the rest stay readable.
    MintOnly,
}

/// How a route is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closure {
    /// By the chat's environment ([`env`]), which a probe runs.
    Environment,
    /// By config purlis compiles for the chat or by a tool's refusal, which its own test holds.
    Refused,
    /// Not closed: the reason it is not.
    Open(&'static str),
}

impl Route {
    /// Every route, in the order the design lists them.
    pub const ALL: [Route; 11] = [
        Self::GhNoMint,
        Self::GhAbsolutePath,
        Self::GitHttps,
        Self::GhKeychain,
        Self::SshAgent,
        Self::BrowserSession,
        Self::HarnessConnector,
        Self::GitKeychain,
        Self::GrantedAgent,
        Self::BareSsh,
        Self::MintOnly,
    ];

    /// The word it is recorded under.
    pub fn word(self) -> &'static str {
        match self {
            Self::GhNoMint => "gh-no-mint",
            Self::GhAbsolutePath => "gh-absolute-path",
            Self::GitHttps => "git-https",
            Self::GhKeychain => "gh-keychain",
            Self::SshAgent => "ssh-agent",
            Self::BrowserSession => "browser-session",
            Self::HarnessConnector => "harness-connector",
            Self::GitKeychain => "git-keychain",
            Self::GrantedAgent => "granted-agent",
            Self::BareSsh => "bare-ssh",
            Self::MintOnly => "mint-only",
        }
    }

    /// How a locked chat has this route closed.
    pub fn closed_by(self) -> Closure {
        match self {
            // `SshAgent` is closed by the environment where purlis sets git's ssh; Claude Code's
            // own sandbox sets git's ssh per command, so there its ssh stays open ([`PROBED`]).
            Self::GhNoMint | Self::GhAbsolutePath | Self::GitHttps | Self::SshAgent => {
                Closure::Environment
            }
            Self::BrowserSession | Self::HarnessConnector => Closure::Refused,
            Self::GhKeychain => Closure::Open(
                "gh reads the keychain item through the system's keychain service, which no \
                 variable turns off, and a sandbox denial of that service is not verified to leave \
                 a chat's harness signed in",
            ),
            Self::GitKeychain => Closure::Open(
                "a helper the chat names itself outranks the lock's config and reads the keychain \
                 (git's osxkeychain, or any other): `git -c credential.helper=<any>`, \
                 `GIT_CONFIG_COUNT=0 git …` and `env -u GIT_CONFIG_COUNT …` each re-admit one, \
                 and no variable turns them off",
            ),
            Self::GrantedAgent => Closure::Open(
                "an ssh agent socket the person granted (ssh-agent.persons) is not refused on a \
                 locked repo yet (design 6.1)",
            ),
            Self::BareSsh => Closure::Open(
                "a bare ssh in a Codex or opencode chat goes through purlis's route, which \
                 includes the person's ~/.ssh/config and so its IdentityAgent",
            ),
            Self::MintOnly => Closure::Open(
                "a claim made by a mint alone sets the environment and denies no file: the \
                 person's gh config and ~/.ssh stay readable",
            ),
        }
    }
}

/// **Whether a chat is on a repo that claims the lock**, as its sandbox compiled: it holds the
/// person's own credentials back (ADR 0067 §5 class 6), or one of its tools mints it a token.
///
/// Slice 3 (#7207) reads the forge's own rulesets; until it does, a chat that has a mint or holds
/// the credentials class is one for which the person has asked purlis to keep their login away.
pub fn claimed(holds_credentials: bool, mints: bool) -> bool {
    holds_credentials || mints
}

/// What `GIT_SSH_COMMAND` is for a locked chat: git's ssh told no agent, and only the identities
/// configured. `route` is the ssh purlis's own route already points git at ([`crate::sandbox`],
/// #1667), which carries its own config; with none, `ssh` reads no config file at all.
pub fn ssh_command(route: Option<&str>) -> String {
    const NO_AGENT: &str = "-o IdentityAgent=none -o IdentitiesOnly=yes";
    match route {
        Some(route) => format!("{route} {NO_AGENT}"),
        None => format!("ssh -F /dev/null {NO_AGENT}"),
    }
}

/// The text of the git config a locked chat's git reads as its global one: the person's own
/// (names, signing, aliases), then every credential helper emptied, then `own`, purlis's own, where
/// the chat has one. `include` is the person's own file, where there is one.
pub fn git_config(include: Option<&Path>, own: Option<&str>) -> String {
    let mut text = String::from(
        "# purlis: this chat's git config (#7205). The helper list is emptied after everything\n\
         # the person's own config names, so no helper of theirs is asked for a credential.\n",
    );
    if let Some(include) = include {
        text.push_str(&format!("[include]\n\tpath = {}\n", include.display()));
    }
    text.push_str("[credential]\n\thelper =\n");
    if let Some(own) = own {
        text.push_str(&format!("\thelper = {own}\n"));
    }
    text
}

/// **The variable a locked chat's line carries so its composition adds the lock's config pair**
/// ([`arm`]): the pair goes after every other, and only where the chat's whole environment is
/// known (a profile's pairs, a `[chat_env] pass` one, purlis's hooks) is the next index known too.
pub const MARKER: &str = "PURLIS_FORGE_LOCK";

/// `env`, a chat's whole environment as it will be started, with the lock's command-scope pair
/// (`credential.helper` emptied, which outranks a repository's own `.git/config` as the global
/// file cannot) as the next `GIT_CONFIG_KEY_<n>`/`GIT_CONFIG_VALUE_<n>` after the pairs it already
/// has, and `GIT_CONFIG_COUNT` raised to include it. The last of a name is the one read, as for
/// [`crate::githooks::GitHooks::arm`]; every pair already there stays.
pub fn arm(
    env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    const COUNT: &str = "GIT_CONFIG_COUNT";
    let n: usize = env
        .iter()
        .rev()
        .find(|(name, _)| name == COUNT)
        .and_then(|(_, value)| value.to_str()?.trim().parse().ok())
        .unwrap_or(0);
    let key = format!("GIT_CONFIG_KEY_{n}");
    let value = format!("GIT_CONFIG_VALUE_{n}");
    let mut out: Vec<_> = env
        .into_iter()
        .filter(|(name, _)| name != COUNT && *name != *key && *name != *value)
        .collect();
    out.push((COUNT.into(), (n + 1).to_string().into()));
    out.push((key.into(), "credential.helper".into()));
    out.push((value.into(), "".into()));
    out
}

/// The folder in `scratch` the lock's files are written in.
const FOLDER: &str = "forge-lock";

/// **A locked chat's environment** (design §6.1): what is set for it, on top of what
/// [`crate::chatenv`] keeps. `scratch` is a folder no other chat shares (its own temp directory),
/// where the empty gh folder and the git config are made; none, and each is `/dev/null`, which
/// gh and git read as nothing at all (fail closed). `home` is the person's home, whose own git
/// config the chat's is built on. `ssh` is the ssh the chat's git already goes through, if any.
pub fn env(
    scratch: Option<&Path>,
    home: Option<&Path>,
    ssh: Option<&str>,
) -> std::io::Result<Vec<(String, String)>> {
    // The marker, not the pair: the pair is added after every other, where the chat's environment
    // is composed ([`arm`], [`crate::chatenv::compose`]).
    let (gh, glab, git) = match scratch {
        Some(scratch) => {
            let folder = scratch.join(FOLDER);
            let gh = folder.join("gh");
            let glab = folder.join("glab");
            std::fs::create_dir_all(&gh)?;
            std::fs::create_dir_all(&glab)?;
            let own = home
                .map(|home| home.join(".gitconfig"))
                .filter(|path| path.is_file());
            let git = folder.join("gitconfig");
            std::fs::write(&git, git_config(own.as_deref(), None))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                for dir in [&folder, &gh, &glab] {
                    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
                }
                std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o600))?;
            }
            (shown(&gh), shown(&glab), shown(&git))
        }
        None => (
            "/dev/null".to_owned(),
            "/dev/null".to_owned(),
            "/dev/null".to_owned(),
        ),
    };
    Ok(vec![
        ("GH_CONFIG_DIR".to_owned(), gh),
        ("GLAB_CONFIG_DIR".to_owned(), glab),
        ("GIT_CONFIG_GLOBAL".to_owned(), git),
        ("GIT_CONFIG_SYSTEM".to_owned(), "/dev/null".to_owned()),
        ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()),
        // The helper list is emptied at the command scope too ([`arm`]), which the composition
        // does where the marker is.
        (MARKER.to_owned(), "1".to_owned()),
        // Empty, not unset: a chat's variables are added to a keep-list, and an empty socket is
        // read by ssh and by git as no agent.
        ("SSH_AUTH_SOCK".to_owned(), String::new()),
        ("GIT_SSH_COMMAND".to_owned(), ssh_command(ssh)),
    ])
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// `env`, a chat's environment, with the locked chat's variables ([`env`]) set over it: one of a
/// name replaces the chat's own, and git's ssh keeps the route a chat already had.
pub fn apply(
    mut chat: Vec<(String, String)>,
    scratch: Option<&Path>,
    home: Option<&Path>,
) -> std::io::Result<Vec<(String, String)>> {
    // Git's ssh as the chat has it: purlis's own route, quoted as a word, or none.
    let route = chat
        .iter()
        .find(|(name, _)| name == "GIT_SSH_COMMAND")
        .map(|(_, value)| value.clone());
    let locked = env(scratch, home, route.as_deref())?;
    chat.retain(|(name, _)| !locked.iter().any(|(set, _)| set == name));
    chat.extend(locked);
    Ok(chat)
}

/// **[`apply`] as a harness's line takes it** (`sandbox::Applied::line`): a Claude Code chat's own
/// sandbox sets git's ssh for each command (#1667), so where its line named none, the lock names
/// none either, and ssh there is its own (recorded open in [`PROBED`]).
pub fn apply_to_line(
    chat: Vec<(String, String)>,
    scratch: Option<&Path>,
    home: Option<&Path>,
    claude_code: bool,
) -> std::io::Result<Vec<(String, String)>> {
    let sets_ssh = chat.iter().any(|(key, _)| key == "GIT_SSH_COMMAND");
    let mut locked = apply(chat, scratch, home)?;
    if claude_code && !sets_ssh {
        locked.retain(|(key, _)| key != "GIT_SSH_COMMAND");
    }
    Ok(locked)
}

/// The hosts whose own pages sign a person in to a forge: a login there is the person's session.
const FORGE_HOSTS: [&str; 2] = ["github.com", "gitlab.com"];

/// **Whether `host` is a forge's**: a host of [`FORGE_HOSTS`] or any name under one, or one of
/// `known`, the hosts the project's remotes are on ([`crate::forge::agentidentity`]).
pub fn is_a_forge_host(host: &str, known: &[String]) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let under = |forge: &str| host == forge || host.ends_with(&format!(".{forge}"));
    FORGE_HOSTS.iter().any(|forge| under(forge)) || known.iter().any(|forge| under(forge))
}

/// **The host of `origin`** (a URL or `host[:port]`): scheme, userinfo, port and path taken off, so
/// a URL with a user, a port and a path is its host alone.
pub fn host_of(origin: &str) -> String {
    let rest = origin.split_once("://").map_or(origin, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if let Some(inner) = authority.strip_prefix('[') {
        return inner.split(']').next().unwrap_or_default().to_owned();
    }
    authority.split(':').next().unwrap_or_default().to_owned()
}

/// What a refused `browser_login` says (#7205): a login to a forge is the person's own session.
pub fn browser_login_refused(host: &str) -> String {
    format!(
        "a browser login to {host} is refused in a chat on a repo whose lock purlis keeps: it \
         would sign this chat in as you. Use the token purlis mints for the chat (declare the \
         tool with its vault, or set up Agent identity in Settings › Agent identity)."
    )
}

/// The names a harness lists a forge connector under, which a locked chat has switched off. A
/// connector purlis cannot name is a residual ([`Status`]).
pub const CONNECTORS: [&str; 6] = [
    "github",
    "gitlab",
    "claude_ai_GitHub",
    "claude_ai_GitLab",
    "gh",
    "glab",
];

/// Names Claude Code alone is given, beyond [`CONNECTORS`]: its plugin servers are
/// `mcp__plugin_<plugin>_<server>`, and its rule matcher is exact-case, so the case variants a
/// forge's own servers are spelled with are named too.
const CLAUDE_ONLY: [&str; 8] = [
    "plugin_github_github",
    "plugin_gitlab_gitlab",
    "GitHub",
    "GitLab",
    "Github",
    "Gitlab",
    "claude_ai_Github",
    "claude_ai_Gitlab",
];

/// **Claude Code's rules that deny every tool of each forge connector** ([`CONNECTORS`] and
/// [`CLAUDE_ONLY`]): a rule `mcp__<name>` matches all of that server's tools. Claude Code has no flag that turns one server
/// off and keeps the rest (`--strict-mcp-config` drops them all, purlis's own and the person's
/// non-forge ones too), so the rule is how the forge's are the only ones lost. The server may
/// still start; a call to it is refused.
pub fn claude_denied() -> Vec<String> {
    CONNECTORS
        .iter()
        .chain(CLAUDE_ONLY.iter())
        .map(|name| format!("mcp__{name}"))
        .collect()
}

/// **The forge connectors of `harness` the chat's config holds, switched off on its line**: only
/// Codex has any. Codex is told `mcp_servers.<name>.enabled=false` for each name of [`CONNECTORS`]
/// that `files` (its config files, [`crate::chattools::codex_config_files`]) define as a server,
/// and for no other: an `enabled=false` for a server with no transport fails Codex's config
/// (`invalid transport`, measured on 0.169.0). Claude Code's are [`claude_denied`], opencode's
/// [`opencode_config`].
pub fn connectors(harness: Harness, files: &[std::path::PathBuf]) -> Vec<String> {
    if harness != Harness::Codex {
        return Vec::new();
    }
    let mut named: Vec<String> = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(config) = text.parse::<toml::Table>() else {
            continue;
        };
        let Some(servers) = config.get("mcp_servers").and_then(toml::Value::as_table) else {
            continue;
        };
        for name in servers.keys() {
            let is_one = CONNECTORS
                .iter()
                .any(|connector| connector.eq_ignore_ascii_case(name));
            let plain = name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if is_one && plain && !named.contains(name) {
                named.push(name.clone());
            }
        }
    }
    named
        .into_iter()
        .flat_map(|name| ["-c".to_owned(), format!("mcp_servers.{name}.enabled=false")])
        .collect()
}

/// **opencode's session config for a locked chat** (#7205): `config`, the JSON
/// [`crate::opencode::session_config`] wrote, with each [`CONNECTORS`] name in its `mcp` table
/// disabled. purlis's own server, which that table names too, is left as it is.
pub fn opencode_config(config: &str) -> String {
    let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(config) else {
        return config.to_owned();
    };
    let Some(root) = parsed.as_object_mut() else {
        return config.to_owned();
    };
    let table = root
        .entry("mcp")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if let Some(table) = table.as_object_mut() {
        for name in CONNECTORS {
            table
                .entry(name)
                .or_insert_with(|| serde_json::json!({ "enabled": false }));
        }
    }
    parsed.to_string()
}

/// One harness version's probe, as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probed {
    pub harness: Harness,
    /// The version its program answers `--version` with (the number alone).
    pub version: &'static str,
    /// The routes the probe found open on it, which keep it from being locked.
    pub open: &'static [Route],
}

/// **Every harness version probed**, each with the routes found open on it. A version not here
/// has not been probed, and a chat on it is not locked. The lists differ by harness, and are not
/// the same list kept in step.
///
/// Recorded 2026-11-19 on macOS, with the fixtures of `a_chat_with_no_mint_has_no_forge_credential`
/// run against each harness's composed environment (a test holds that a route probed here is
/// recorded open exactly where it leaked). Open on all three: gh's keychain item
/// ([`Route::GhKeychain`]), git's own keychain helper on a command line ([`Route::GitKeychain`]),
/// a granted agent socket ([`Route::GrantedAgent`]) and a claim by a mint alone
/// ([`Route::MintOnly`]), none of which a fixture person can stand for. Claude Code adds its own
/// ssh ([`Route::SshAgent`]): its line has no `GIT_SSH_COMMAND`, so `~/.ssh/config`'s
/// `IdentityAgent` applies, which the probe does reach. Codex and opencode add a bare `ssh`
/// ([`Route::BareSsh`]).
pub const PROBED: [Probed; 3] = [
    Probed {
        harness: Harness::ClaudeCode,
        version: "2.1.328",
        open: &[
            Route::GhKeychain,
            Route::SshAgent,
            Route::GitKeychain,
            Route::GrantedAgent,
            Route::MintOnly,
        ],
    },
    Probed {
        harness: Harness::Codex,
        version: "0.169.0",
        open: &[
            Route::GhKeychain,
            Route::GitKeychain,
            Route::GrantedAgent,
            Route::BareSsh,
            Route::MintOnly,
        ],
    },
    Probed {
        harness: Harness::Opencode,
        version: "1.19.5",
        open: &[
            Route::GhKeychain,
            Route::GitKeychain,
            Route::GrantedAgent,
            Route::BareSsh,
            Route::MintOnly,
        ],
    },
];

/// What purlis says of a chat's lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Probed, with no route open: locked for the routes purlis knows.
    Locked,
    /// Not locked, and why: the version was never probed, or a route is open on it.
    NotLocked(String),
}

/// **Whether a chat of `harness` at `version` is locked**: only a probed version with no route
/// open. An unprobed one, or one whose version is unknown, is not (it fails closed).
pub fn status(harness: Harness, version: Option<&str>) -> Status {
    let Some(version) = version.map(str::trim).filter(|version| !version.is_empty()) else {
        return Status::NotLocked(format!(
            "the version of {} is not known, so it has not been probed",
            harness.name()
        ));
    };
    match PROBED
        .iter()
        .find(|probed| probed.harness == harness && probed.version == version)
    {
        None => Status::NotLocked(format!(
            "{} {version} has not been probed for a path to your forge login",
            harness.name()
        )),
        Some(probed) if probed.open.is_empty() => Status::Locked,
        Some(probed) => Status::NotLocked(format!(
            "{} {version} was probed and these routes are open: {}",
            harness.name(),
            probed
                .open
                .iter()
                .map(|route| route.word())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The marker a probe's fixtures put in everything that is the person's own, and that no route's
/// answer may hold.
pub const PERSON: &str = "PERSON-FORGE-LOGIN-7205";

/// The result of one route's probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub route: Route,
    /// What the route answered.
    pub said: String,
    /// Whether it answered with the person's identity.
    pub leaked: bool,
}

/// **The probe**: each route [`Closure::Environment`] closes, tried in the environment `env` a
/// chat of its harness is composed with, run from `cwd`, with `PATH` as that environment has it.
/// The commands are the design's: `gh api user` (named by `gh`, and by the real `gh`'s absolute
/// path `real_gh`), git's credential fill for the forge's host, and ssh's own resolved
/// configuration for it. Each is run for its answer and nothing is sent to a forge.
///
/// A route passes when [`PERSON`] is nowhere in what it said.
pub fn probe(env: &[(String, String)], cwd: &Path, real_gh: &Path) -> Vec<Finding> {
    let tries: [(Route, String); 5] = [
        (Route::GhNoMint, "gh api user".to_owned()),
        (
            Route::GhAbsolutePath,
            format!("'{}' api user", real_gh.display()),
        ),
        (
            Route::GitHttps,
            "printf 'protocol=https\\nhost=github.com\\n\\n' | git credential fill".to_owned(),
        ),
        (
            Route::SshAgent,
            // `-F` the home's config: ssh reads the account's home, never `$HOME`, so without it a
            // fixture person's config is not read and the machine's own is. A chat's own
            // `-F` (git's route, `/dev/null`) comes first and wins.
            "eval \"${GIT_SSH_COMMAND:-ssh} -F \\\"\\$HOME/.ssh/config\\\" -G github.com\""
                .to_owned(),
        ),
        // The agent's own socket, as a program that reads the variable finds it.
        (
            Route::SshAgent,
            "printf '%s' \"${SSH_AUTH_SOCK:-}\"".to_owned(),
        ),
    ];
    tries
        .into_iter()
        .map(|(route, command)| {
            let mut run = std::process::Command::new("/bin/sh");
            run.arg("-c")
                .arg(&command)
                .current_dir(cwd)
                .env_clear()
                // Never the machine's own system git config, whose helper is the person's keychain.
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .envs(env.iter().map(|(name, value)| (name, value)));
            let out = crate::forklock::output(&mut run);
            let said = match out {
                Ok(out) => format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                ),
                Err(why) => format!("not run: {why}"),
            };
            Finding {
                route,
                leaked: said.contains(PERSON),
                said,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
