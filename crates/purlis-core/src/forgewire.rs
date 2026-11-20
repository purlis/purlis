// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a chat's `gh`, `git`, `glab` and `cargo` are pointed at purlis's forge proxy** (ADR 0128
//! D3b and D3c, "One login", slice B).
//!
//! A chat holds no token. Its tools carry a **sentinel** where each already sends a credential,
//! and are routed, by environment and by config files in the chat's own folder, to the proxy that
//! acts for the chat's recorded repos. This module composes that wiring and writes nothing but
//! the chat's own folder: it never touches the person's clone `.git/config`, `~/.config/gh`, the
//! keychain, or the system or user trust store.
//!
//! **What each tool gets** (every one is spike E's, on Linux and macOS):
//!
//! - **gh**: `GH_HOST=github.localhost`, `GH_CONFIG_DIR` the chat's own folder with a `config.yml`
//!   whose only setting is `http_unix_socket` (the chat's socket), `GH_TOKEN` and
//!   `GH_ENTERPRISE_TOKEN` the sentinel, and a `gh` wrapper first on `PATH` ([`GH_WRAPPER`]) that
//!   passes `-R github.localhost/<owner>/<repo>` from the recorded repo of the working directory,
//!   because with that host `gh` matches no remote of `github.com`.
//! - **git**: environment only, as `GIT_CONFIG_*` pairs appended after every pair already there:
//!   an empty `credential.helper` (so `osxkeychain` and the person's helpers are dropped and
//!   cannot be asked to store the sentinel), then purlis's own helper scoped to each recorded
//!   host ([`gitcredential::serve_sentinel`]), and `http.https://<host>/.sslCAInfo` naming the
//!   chat's CA **for that host alone**. `HTTPS_PROXY` is the chat's egress port. **Never**
//!   `GIT_SSL_CAINFO` (it would replace git's whole bundle) and never the system trust.
//! - **glab**: `HTTPS_PROXY`, `GLAB_CA_CERT` and `GITLAB_TOKEN` the sentinel; `GITLAB_HOST` is
//!   left to `glab`'s own inference from the remotes.
//! - **cargo**: `CARGO_NET_GIT_FETCH_WITH_CLI=true`, so git dependencies go through `git`.
//!   libgit2 is a named gap ([`libgit2_gap`]).
//! - **npm, pip, curl**: nothing. The download-only hosts ([`DOWNLOAD_ONLY`]) are tunnelled with
//!   their real certificates, so neither `NODE_EXTRA_CA_CERTS` nor `PIP_CERT` is set.
//!
//! **The composition is the same for every harness and both systems.** What differs by system is
//! the version of each tool purlis has run ([`PROVEN`]); another version still gets the route and
//! a non-blocking [`Advice`].
//!
//! **Where it gets its inputs.** The chat's unix socket, its egress port, its CA and its sentinel
//! are slice A's (`forge/proxy`, the `ChatEndpoint` trait). They arrive here as plain values in a
//! [`Route`], so this module has no dependency on that slice, and the sandbox calls it only when
//! the `forge-wiring` feature is on (the train turns it on once a proxy is there to answer).

use std::io;
use std::path::{Path, PathBuf};

use secrecy::ExposeSecret;
pub use secrecy::SecretString;

use crate::forge::gitcredential::{SENTINEL_ENV, SENTINEL_HELPER_ARG};

/// The host name `gh` is told is its forge. It is never resolved: `gh` reaches it over the chat's
/// unix socket, and the proxy builds the upstream from the chat's recorded host (D3b).
pub const GH_HOST: &str = "github.localhost";

/// The hosts that serve downloads and never take a credential: tunnelled with their own
/// certificates, never terminated, never named by the chat's CA, never denied (D3i).
pub const DOWNLOAD_ONLY: [&str; 3] = [
    "codeload.github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];

/// The forge hosts every chat's egress proxy denies as a direct tunnel, besides the hosts of its
/// recorded repos (D3g): the only route to them is the forge route.
pub const KNOWN_FORGE_HOSTS: [&str; 4] = [
    "github.com",
    "api.github.com",
    "uploads.github.com",
    "gitlab.com",
];

/// A tool whose version purlis has run the route with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Gh,
    Glab,
    Git,
}

impl Tool {
    /// The program's name.
    pub fn program(self) -> &'static str {
        match self {
            Self::Gh => "gh",
            Self::Glab => "glab",
            Self::Git => "git",
        }
    }
}

/// The systems a version was run on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOs,
}

impl Os {
    /// The system this build runs on, where it is one of the two.
    pub fn this() -> Option<Self> {
        if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else {
            None
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Linux => "Linux",
            Self::MacOs => "macOS",
        }
    }
}

/// One tool version the route was run with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Proven {
    pub tool: Tool,
    pub os: Os,
    /// The number alone, as [`parse_version`] reads it.
    pub version: &'static str,
}

/// **Every version the route was run with, per system and tool** (spike E, 2026-11-20, PR
/// #7270), recorded as [`crate::forgelock::PROBED`] records a harness: a version not here has not
/// been run, and its chat gets an [`Advice`]. `GH_HOST=github.localhost` and `http_unix_socket`
/// are lightly documented, so a `gh` bump re-runs the spike's job before this list changes.
pub const PROVEN: [Proven; 6] = [
    Proven {
        tool: Tool::Gh,
        os: Os::Linux,
        version: "2.103.3",
    },
    Proven {
        tool: Tool::Gh,
        os: Os::MacOs,
        version: "2.105.0",
    },
    Proven {
        tool: Tool::Glab,
        os: Os::Linux,
        version: "1.119.0",
    },
    Proven {
        tool: Tool::Glab,
        os: Os::MacOs,
        version: "1.131.0",
    },
    Proven {
        tool: Tool::Git,
        os: Os::Linux,
        version: "2.56.1",
    },
    Proven {
        tool: Tool::Git,
        os: Os::MacOs,
        version: "2.56.1",
    },
];

/// The version of `tool` run on `os`.
pub fn proven(tool: Tool, os: Os) -> Option<&'static str> {
    PROVEN
        .iter()
        .find(|one| one.tool == tool && one.os == os)
        .map(|one| one.version)
}

/// The version a tool's `--version` answer names: the first word of its first line that is a
/// dotted number (`gh version 2.105.0 (2026-…)`, `glab 1.131.0 (…)`, `git version 2.56.1`).
pub fn parse_version(text: &str) -> Option<String> {
    text.lines().next()?.split_whitespace().find_map(|word| {
        let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        let numeric = word.chars().all(|c| c.is_ascii_digit() || c == '.');
        (numeric && word.contains('.') && word.starts_with(|c: char| c.is_ascii_digit()))
            .then(|| word.to_owned())
    })
}

/// What a person is told, in the window, without anything being stopped: a **non-blocking**
/// Notice's content (`app/src/Notice.tsx` wants a way out and a cause key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advice {
    /// The cause key the Notice is kept under.
    pub cause: String,
    /// One sentence that says what happened and what to do.
    pub sentence: String,
    /// The way out.
    pub fix: Fix,
}

/// The way out an [`Advice`] offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// Install the version purlis has run, or Dismiss.
    Install { tool: Tool, version: &'static str },
    /// Set this cargo setting.
    Setting(&'static str),
}

/// The cargo setting that sends git dependencies through `git`.
pub const FETCH_WITH_CLI: &str = "net.git-fetch-with-cli";

/// The variable cargo reads that setting from.
const FETCH_WITH_CLI_ENV: &str = "CARGO_NET_GIT_FETCH_WITH_CLI";

/// **The advice for a tool whose version purlis has not run**, or none where `found` is the
/// version run on `os`. The route is set up all the same: the advice says it is not one purlis
/// has run, and never stops the chat.
pub fn check_version(tool: Tool, os: Os, found: Option<&str>) -> Option<Advice> {
    let ran = proven(tool, os)?;
    let found = found.map(str::trim).filter(|found| !found.is_empty());
    if found == Some(ran) {
        return None;
    }
    let program = tool.program();
    let said = match found {
        Some(found) => format!("{program} {found}"),
        None => format!("this {program}"),
    };
    Some(Advice {
        cause: format!("forge-route-unproven-{program}"),
        sentence: format!(
            "{said} is not a version purlis has run its forge route with on {} ({ran}), so \
             {program} may fail on it. Install {program} {ran}, or Dismiss to carry on.",
            os.name()
        ),
        fix: Fix::Install { tool, version: ran },
    })
}

/// **The named gap: cargo's own git client** (libgit2) got 401 through the route on both systems
/// in every variant tried (D3c). `net.git-fetch-with-cli=true` is set for every chat; a chat that
/// has unset it gets this advice. None where `env` leaves it on.
pub fn libgit2_gap(env: &[(String, String)]) -> Option<Advice> {
    let on = env
        .iter()
        .rev()
        .find(|(name, _)| name == FETCH_WITH_CLI_ENV)
        .is_some_and(|(_, value)| value == "true");
    (!on).then(|| Advice {
        cause: "forge-route-cargo-libgit2".to_owned(),
        sentence: format!(
            "Cargo's own git client cannot sign in through purlis, so a git dependency will fail \
             with a 401. Set `{FETCH_WITH_CLI}` to true."
        ),
        fix: Fix::Setting(FETCH_WITH_CLI),
    })
}

/// Which kind of forge a repo is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeKind {
    GitHub,
    GitLab,
}

/// Why a repo is not one the wiring can name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadRepo(pub String);

impl std::fmt::Display for BadRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BadRepo {}

/// One of the chat's recorded repos (D4): the host it is on, and its owner and name.
///
/// Every part is checked to be only letters, digits, `.`, `_` and `-` (a GitLab owner may be a
/// `group/subgroup` path, whose parts are each checked), so a value can sit in a git config key,
/// a shell `case` and a URL with no quoting question. A repo that fails is refused here, not
/// quietly left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    host: String,
    owner: String,
    name: String,
    kind: ForgeKind,
}

impl Repo {
    pub fn new(host: &str, owner: &str, name: &str, kind: ForgeKind) -> Result<Self, BadRepo> {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let part = |text: &str| {
            !text.is_empty()
                && text != "."
                && text != ".."
                && text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        };
        let owner_ok = match kind {
            ForgeKind::GitHub => part(owner),
            ForgeKind::GitLab => owner.split('/').all(part),
        };
        if !part(&host) || !owner_ok || !part(name) {
            return Err(BadRepo(format!(
                "{host}/{owner}/{name} is not a repository purlis can name for a chat's forge \
                 route: a part is empty or has a character other than a letter, a digit, '.', \
                 '_' or '-'"
            )));
        }
        Ok(Self {
            host,
            owner: owner.to_owned(),
            name: name.to_owned(),
            kind,
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn kind(&self) -> ForgeKind {
        self.kind
    }

    fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// **What the chat is routed to**, from slice A's endpoint and the chat's start.
pub struct Route {
    /// The chat's own egress port (`http://127.0.0.1:<port>`, [`crate::sandbox::egress::url_of`]).
    pub egress_url: String,
    /// The chat's own unix socket, for `gh`.
    pub gh_socket: PathBuf,
    /// The chat's CA certificate (public), which the proxy's leaf certificates are signed under.
    pub ca_cert: PathBuf,
    /// The chat's sentinel.
    pub sentinel: SecretString,
    /// The app's own binary, which git starts as the chat's credential helper.
    pub helper_exe: PathBuf,
    /// The chat's recorded repos.
    pub repos: Vec<Repo>,
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Route")
            .field("egress_url", &self.egress_url)
            .field("gh_socket", &self.gh_socket)
            .field("ca_cert", &self.ca_cert)
            .field("repos", &self.repos)
            .finish_non_exhaustive()
    }
}

/// The wiring of one chat, composed and written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wiring {
    /// The variables to set, over whatever the chat's line has.
    pub env: Vec<(String, String)>,
    /// `GIT_CONFIG_KEY_n`/`VALUE_n` pairs, in the order they must be appended after every pair
    /// already in the chat's environment: the helper reset first.
    pub git_pairs: Vec<(String, String)>,
    /// The folder holding the `gh` wrapper, to go first on the chat's `PATH`.
    pub bin: PathBuf,
}

/// The folder in the chat's scratch the wiring is written in.
const FOLDER: &str = "forge-wire";

/// The names taken out of a chat's environment: git reads each over a per-host `sslCAInfo`, so a
/// chat that had one would be trusting its bundle for the forge hosts instead of the chat's CA.
const OVERRIDDEN: [&str; 2] = ["GIT_SSL_CAINFO", "GIT_SSL_CAPATH"];

/// Ambient forge tokens taken out of a chat's environment (ADR, D3h): a chat carries the sentinel
/// and nothing else. `GH_TOKEN`, `GH_ENTERPRISE_TOKEN` and `GITLAB_TOKEN` are then set to it.
/// Any variable starting with [`FORGE_TOKEN_PREFIX`] goes too.
const AMBIENT_FORGE_TOKENS: [&str; 7] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITLAB_TOKEN",
    "GLAB_TOKEN",
    "CI_JOB_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
];

/// The prefix of purlis's own forge-token variables, none of which a chat may keep.
const FORGE_TOKEN_PREFIX: &str = "PURLIS_FORGE_TOKEN";

/// **Composes and writes** the wiring for `route`, in `scratch` (a folder no other chat shares).
pub fn compose(route: &Route, scratch: &Path) -> io::Result<Wiring> {
    if !route.egress_url.starts_with("http://127.0.0.1:") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a chat's forge route is its own egress port on the loopback interface",
        ));
    }
    let sentinel = route.sentinel.expose_secret();
    if sentinel.is_empty() || sentinel.contains(|c: char| c.is_control() || c.is_whitespace()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the sentinel is empty, or has a space or a control character",
        ));
    }
    let socket = route
        .gh_socket
        .to_str()
        .filter(|s| !s.contains(['\n', '\r']));
    let (Some(socket), Some(ca)) = (
        socket,
        route.ca_cert.to_str().filter(|s| !s.contains(['\n', '\r'])),
    ) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a path of the forge route is not text",
        ));
    };
    let helper = quoted(&route.helper_exe).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the credential helper's path cannot be quoted for the shell git runs it through",
        )
    })?;

    let folder = scratch.join(FOLDER);
    let gh = folder.join("gh");
    let bin = folder.join("bin");
    for dir in [&folder, &gh, &bin] {
        std::fs::create_dir_all(dir)?;
        set_mode(dir, 0o700)?;
    }
    // Holds the one setting and no login: `hosts.yml` is never written here.
    let config = gh.join("config.yml");
    std::fs::write(
        &config,
        format!(
            "# purlis: this chat's gh settings. No login is kept here: the chat's gh reaches\n\
             # purlis's forge proxy over this socket and carries a sentinel, never a token.\n\
             http_unix_socket: '{}'\n",
            socket.replace('\'', "''")
        ),
    )?;
    set_mode(&config, 0o600)?;
    let wrapper = bin.join("gh");
    std::fs::write(&wrapper, gh_wrapper(&route.repos))?;
    set_mode(&wrapper, 0o700)?;

    let mut env = vec![
        ("GH_HOST".to_owned(), GH_HOST.to_owned()),
        ("GH_CONFIG_DIR".to_owned(), shown(&gh)),
        ("GH_TOKEN".to_owned(), sentinel.to_owned()),
        ("GH_ENTERPRISE_TOKEN".to_owned(), sentinel.to_owned()),
        ("GITLAB_TOKEN".to_owned(), sentinel.to_owned()),
        ("GLAB_CA_CERT".to_owned(), ca.to_owned()),
        ("HTTPS_PROXY".to_owned(), route.egress_url.clone()),
        ("https_proxy".to_owned(), route.egress_url.clone()),
        (FETCH_WITH_CLI_ENV.to_owned(), "true".to_owned()),
        (SENTINEL_ENV.to_owned(), sentinel.to_owned()),
    ];
    env.sort();

    // The reset first, so `osxkeychain` and every helper of the person's are dropped before
    // purlis's one is named, and so cannot be asked to store the sentinel.
    let mut git_pairs = vec![("credential.helper".to_owned(), String::new())];
    let mut hosts: Vec<&str> = route.repos.iter().map(|repo| repo.host()).collect();
    hosts.sort_unstable();
    hosts.dedup();
    let named: String = hosts.iter().map(|host| format!(" --host {host}")).collect();
    for host in &hosts {
        git_pairs.push((
            format!("credential.https://{host}.helper"),
            format!("!{helper} {SENTINEL_HELPER_ARG}{named}"),
        ));
    }
    for host in &hosts {
        git_pairs.push((format!("http.https://{host}/.sslCAInfo"), ca.to_owned()));
    }
    Ok(Wiring {
        env,
        git_pairs,
        bin,
    })
}

/// **`env`, a chat's environment, with `wiring` applied**: its variables set over the chat's own
/// (one of a name replaces it), the variables that would send trust elsewhere taken out
/// ([`OVERRIDDEN`]), the ambient forge tokens scrubbed ([`AMBIENT_FORGE_TOKENS`]), and the git pairs appended after every pair already there. `GIT_CONFIG_COUNT`
/// is raised to include them.
///
/// **The forge lock's marker is taken out** ([`crate::forgelock::MARKER`]): where it stays,
/// the composition adds its own empty helper after every pair, which would drop purlis's helper
/// named here. The wiring's own reset comes first, and the lock's global file still empties every
/// helper of the person's.
pub fn apply(mut env: Vec<(String, String)>, wiring: &Wiring) -> Vec<(String, String)> {
    env.retain(|(name, _)| {
        name != crate::forgelock::MARKER
            && !OVERRIDDEN.contains(&name.as_str())
            && !AMBIENT_FORGE_TOKENS.contains(&name.as_str())
            && !name.starts_with(FORGE_TOKEN_PREFIX)
            && !wiring.env.iter().any(|(set, _)| set == name)
    });
    env.extend(wiring.env.iter().cloned());
    append_git_pairs(env, &wiring.git_pairs)
}

/// `env` with `pairs` as the next `GIT_CONFIG_KEY_<n>`/`GIT_CONFIG_VALUE_<n>`, and the count
/// raised. The last of a name is the one read, as for [`crate::forgelock::arm`].
fn append_git_pairs(
    mut env: Vec<(String, String)>,
    pairs: &[(String, String)],
) -> Vec<(String, String)> {
    const COUNT: &str = "GIT_CONFIG_COUNT";
    let n: usize = env
        .iter()
        .rev()
        .find(|(name, _)| name == COUNT)
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0);
    env.retain(|(name, _)| name != COUNT);
    for (at, (key, value)) in pairs.iter().enumerate() {
        let (k, v) = (
            format!("GIT_CONFIG_KEY_{}", n + at),
            format!("GIT_CONFIG_VALUE_{}", n + at),
        );
        env.retain(|(name, _)| *name != k && *name != v);
        env.push((k, key.clone()));
        env.push((v, value.clone()));
    }
    env.push((COUNT.to_owned(), (n + pairs.len()).to_string()));
    env
}

/// **The folder to put first on a chat's `PATH`**, joined to the one already named (persona tools,
/// [`crate::sandbox::tools::FIRST_ON_PATH`]) so both are first, `bin` ahead of it.
pub fn first_on_path(already: Option<&str>, bin: &Path) -> String {
    match already.filter(|dir| !dir.is_empty()) {
        Some(already) => format!("{}:{already}", bin.display()),
        None => bin.display().to_string(),
    }
}

/// **The hosts a chat's egress proxy denies as a direct tunnel** (D3g): [`KNOWN_FORGE_HOSTS`] and
/// the host of every recorded repo, but never a [`DOWNLOAD_ONLY`] host. The only route to them
/// is the forge route, which terminates the tunnel itself.
pub fn denied_hosts(repos: &[Repo]) -> Vec<String> {
    let mut hosts: Vec<String> = KNOWN_FORGE_HOSTS
        .iter()
        .map(|host| (*host).to_owned())
        .chain(repos.iter().map(|repo| repo.host().to_owned()))
        .filter(|host| !DOWNLOAD_ONLY.contains(&host.as_str()))
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts
}

/// The subcommands of `gh` that take `-R`/`--repo`, by the words that name them: the whole of the
/// wrapper's mechanism. A subcommand missing here fails in `gh` with its remote-matching
/// sentence, so a test holds the table to the `gh` on the machine where there is one.
pub const TAKES_REPO: &[(&str, &[&str])] = &[
    (
        "pr",
        &[
            "checkout",
            "checks",
            "close",
            "comment",
            "create",
            "diff",
            "edit",
            "list",
            "lock",
            "merge",
            "ready",
            "reopen",
            "revert",
            "review",
            "status",
            "unlock",
            "update-branch",
            "view",
        ],
    ),
    (
        "issue",
        &[
            "close", "comment", "create", "delete", "develop", "edit", "list", "lock", "pin",
            "reopen", "status", "transfer", "unlock", "unpin", "view",
        ],
    ),
    (
        "run",
        &[
            "cancel", "delete", "download", "list", "rerun", "view", "watch",
        ],
    ),
    ("workflow", &["disable", "enable", "list", "run", "view"]),
    (
        "release",
        &[
            "create",
            "delete",
            "delete-asset",
            "download",
            "edit",
            "list",
            "upload",
            "view",
        ],
    ),
    ("label", &["clone", "create", "delete", "edit", "list"]),
    ("cache", &["delete", "list"]),
    ("ruleset", &["check", "list", "view"]),
    ("secret", &["delete", "list", "set"]),
    ("variable", &["delete", "get", "list", "set"]),
];

/// The commands of `gh` that take `-R` with no subcommand.
pub const TAKES_REPO_ALONE: &[&str] = &["browse"];

/// The name the wrapper is written under.
pub const GH_WRAPPER: &str = "gh";

/// **The `gh` wrapper**: a POSIX shell script that finds the recorded repo of the working
/// directory from its remotes, and where the chat gave no `-R` of its own and the subcommand
/// takes one, passes `-R github.localhost/<owner>/<repo>`. It then runs the next `gh` on `PATH`
/// that is not itself. **It never reads `gh`'s output.** A directory with no recorded GitHub
/// remote gets `gh` unchanged.
pub fn gh_wrapper(repos: &[Repo]) -> String {
    let mut script = String::from(
        "#!/bin/sh\n\
         # purlis: this chat's gh. With GH_HOST=github.localhost, gh matches no remote of\n\
         # github.com, so the repo of the working directory is named with -R. Generated; the\n\
         # chat's own -R wins. It never reads gh's output.\n\
         self=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\n\
         real=\n\
         IFS=:\n\
         for dir in $PATH; do\n\
         \t[ -n \"$dir\" ] && [ \"$dir\" != \"$self\" ] || continue\n\
         \tif [ -x \"$dir/gh\" ] && [ ! -d \"$dir/gh\" ]; then real=\"$dir/gh\"; break; fi\n\
         done\n\
         unset IFS\n\
         slug=\n\
         if [ -z \"$real\" ]; then echo 'purlis: no gh is installed on this chat'\"'\"'s PATH' >&2; exit 127; fi\n\
         # How many words name the subcommand: 2 where gh takes -R, 1 for a command alone, else 0.\n\
         words=0\n\
         case \"$1 $2\" in\n",
    );
    for (group, subs) in TAKES_REPO {
        let cases: Vec<String> = subs
            .iter()
            .map(|sub| format!("\"{group} {sub}\""))
            .collect();
        script.push_str(&format!("\t{}) words=2;;\n", cases.join("|")));
    }
    script.push_str("esac\n");
    if !TAKES_REPO_ALONE.is_empty() {
        script.push_str("case \"$1\" in\n");
        for alone in TAKES_REPO_ALONE {
            script.push_str(&format!("\t{alone}) words=1;;\n"));
        }
        script.push_str("esac\n");
    }
    script.push_str(
        "[ \"$words\" = 0 ] && exec \"$real\" \"$@\"\n\
         # The chat's own -R, --repo or -R<repo> stands.\n\
         for arg in \"$@\"; do\n\
         \tcase \"$arg\" in\n\
         \t\t--) break;;\n\
         \t\t-R|-R*|--repo|--repo=*) exec \"$real\" \"$@\";;\n\
         \tesac\n\
         done\n\
         # The recorded repo of this directory: the first remote whose address names one.\n\
         for remote in $(git remote 2>/dev/null); do\n\
         \turl=$(git config --get \"remote.$remote.url\" 2>/dev/null) || continue\n\
         \trest=${url#*://}\n\
         \trest=${rest#*@}\n\
         \thost=${rest%%[:/]*}\n\
         \tpath=${rest#*[:/]}\n\
         \tpath=${path%/}\n\
         \tpath=${path%.git}\n\
         \tcase \"$host $path\" in\n",
    );
    for repo in repos.iter().filter(|r| r.kind() == ForgeKind::GitHub) {
        script.push_str(&format!(
            "\t\t\"{} {}\") slug={}/{}; break;;\n",
            repo.host,
            repo.slug(),
            GH_HOST,
            repo.slug()
        ));
    }
    script.push_str(
        "\tesac\n\
         done\n\
         [ -n \"$slug\" ] || exec \"$real\" \"$@\"\n\
         if [ \"$words\" = 2 ]; then group=$1; sub=$2; shift 2; exec \"$real\" \"$group\" \"$sub\" -R \"$slug\" \"$@\"; fi\n\
         group=$1; shift; exec \"$real\" \"$group\" -R \"$slug\" \"$@\"\n",
    );
    script
}

/// `path` single-quoted for the shell, or none when it holds a quote or is not text.
fn quoted(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    (!text.contains(['\'', '\n', '\r', '\0'])).then(|| format!("'{text}'"))
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
