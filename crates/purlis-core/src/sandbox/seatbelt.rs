// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Charter's own compiled sandbox on macOS: the Seatbelt profile a whole harness runs inside
//! (ADR 0067 §2), and the one a profile's program answers `--version` inside (D-88k).
//!
//! One profile for every harness charter wraps (opencode and Codex): what a chat may write, the
//! denial classes after it, and what each harness keeps for itself ([`Own`]). Seatbelt takes
//! the last rule that matches, so every denial comes after every grant.
//!
//! **One sandbox per process.** Seatbelt refuses a second profile inside one charter applied
//! (measured on macOS 26.2: `sandbox_apply: Operation not permitted` under any profile that
//! denies anything), so a harness whose own sandbox is applied through Seatbelt cannot keep it
//! on inside this one.

use std::path::{Path, PathBuf};

use super::{Access, Denial, PLANTED, Reach, real};

/// What the system starts a program with, and the refusal of a start longer than that: the
/// terminal's, which refuses a chat's start the same way (#1425, #1995).
pub use purlis_terminal::exec::{Exec, TOO_LONG, exec_size, fits_exec};

/// Why a profile could not be written: a path holding a control character, which a rule could
/// not state exactly.
pub const CONTROL: &str = "purlis cannot write a sandbox profile for a path holding a control \
                           character";

/// Why a profile could not be handed over: longer than [`MOST_PROFILE`].
pub const TOO_LARGE: &str = "purlis cannot hand the sandbox a profile this long: the project's \
                             configs name more paths than one chat's sandbox can hold";

/// What a person can do about a refusal `why` of [`profile`] or [`fits_exec`], where there is
/// something. `opt_out` is whether they may start the chat without the sandbox: where an
/// administrator's policy forbids it, the way out does not name it.
pub(crate) fn way_out(why: &str, opt_out: bool) -> Option<&'static str> {
    if why == TOO_LONG {
        // The sandbox is not what is too long, so leaving it off is no way on.
        return Some(purlis_terminal::exec::TOO_LONG_WAY_OUT);
    }
    if why == super::wrapper::NO_PURLIS {
        return Some("Restart purlis, then start the chat again.");
    }
    (why == TOO_LARGE).then_some(if opt_out {
        "Have the configs name fewer scripts, or start this chat without the sandbox from the \
         new-chat picker."
    } else {
        "Have the configs name fewer scripts."
    })
}

/// The refusal of a chat whose profile could not be written for `why`: after `lead`, what
/// happened, then the way out where there is one (#1418). `opt_out` is whether the person may
/// start the chat without the sandbox ([`super::At::no_opt_out`] says where policy forbids
/// it): where they may not, the refusal names no opt-out (#1423), and [`super::Applied::line`]
/// ends it with the policy and who set it.
pub fn not_started(lead: &str, why: &str, opt_out: bool) -> String {
    match way_out(why, opt_out) {
        Some(then) => format!("{lead} {why}, so nothing was started. {then}"),
        None => format!("{lead} {why}, so nothing was started."),
    }
}

/// The longest profile a wrap hands `sandbox-exec` (#1418). It goes in argv (`-p`), which
/// holds 1 MiB on macOS with the environment in it: one config at [`super::planted::MOST_NAMED`]
/// writes about 300 KB, so two fit, and a quarter is left for the environment and the
/// harness's own words. A longer one is refused by [`TOO_LARGE`], not left to fail the start
/// with `E2BIG`. The profile with the rest of the start, the first message and the whole
/// environment among it, is measured by [`fits_exec`] (#1425).
pub const MOST_PROFILE: usize = 768 * 1024;

/// What a harness charter wraps keeps for itself, beside the chat's own directory and temp
/// directory: each grant is only what a turn of that harness writes (rulings V73 and V73a).
#[derive(Debug, Clone, Default)]
pub struct Own {
    /// The harness's own directories. A chat whose directory holds one, or is inside one, is
    /// not wrapped: its directory's grant would be a grant on what a later harness loads.
    pub dirs: Vec<PathBuf>,
    /// Why such a chat is not wrapped, as one clause.
    pub overlap: &'static str,
    /// Filters, in the `file-write*` allow, for what a turn writes.
    pub allow: Vec<String>,
    /// Directories it writes into, which get the later-code names denied at any depth too.
    pub roots: Vec<PathBuf>,
    /// Whole rules, after every denial class.
    pub deny: Vec<String>,
    /// Whole rules last of all: a read given back inside a folder denied above.
    pub after: Vec<String>,
}

/// `text` as an SBPL string, or [`CONTROL`].
pub fn string(text: &str) -> Result<String, &'static str> {
    if text.chars().any(char::is_control) {
        return Err(CONTROL);
    }
    Ok(format!(
        "\"{}\"",
        text.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// `path` as the kernel names it, as an SBPL string.
pub fn quote(path: &Path) -> Result<String, &'static str> {
    string(&real(path).display().to_string())
}

/// `path` under every name the kernel may give it, each as an SBPL string: as it resolves, and
/// across a macOS firmlink (`/Users` and `/System/Volumes/Data/Users`), which resolving leaves
/// as spelled (#1356).
pub fn kernel_strings(path: &Path) -> Result<Vec<String>, &'static str> {
    super::kernel_names(path)
        .iter()
        .map(|name| string(&name.display().to_string()))
        .collect()
}

/// `path` itself under every name the kernel may give it ([`super::kernel_names_itself`]), each
/// as an SBPL string: a link by its own name, never by what it points at.
pub fn kernel_strings_itself(path: &Path) -> Result<Vec<String>, &'static str> {
    super::kernel_names_itself(path)
        .iter()
        .map(|name| string(&name.display().to_string()))
        .collect()
}

/// The names a rule on `path` is written under: as the kernel names it, and, where its last part
/// is a link, that link itself in the folder as the kernel names it. A rule on the target alone
/// would let the link be removed and a file made in its place (#1336).
pub fn held_names(path: &Path) -> Vec<PathBuf> {
    let resolved = real(path);
    let mut out = vec![resolved.clone()];
    if let (Some(folder), Some(name)) = (path.parent(), path.file_name()) {
        let itself = real(folder).join(name);
        if itself != resolved {
            out.push(itself);
        }
    }
    out
}

/// The rules that deny `denial`: under each name it is held by, as itself and as its target
/// (#1336), each also across a firmlink (#1356). Everything below the name, or the entry alone
/// for one held as itself, which is then never written as an entry: never created, renamed,
/// moved or removed (#2752). One held under every spelling is a regular expression with each
/// letter in either case, so a folder not there yet and made under another spelling of its name
/// is held too (#2752).
pub fn denial_rules(denial: &Denial) -> Result<Vec<String>, &'static str> {
    let what = match (denial.reach, denial.access) {
        (Reach::Itself, _) | (Reach::AndBelow, Access::Write) => "file-write*",
        (Reach::AndBelow, Access::ReadWrite) => "file-read* file-write*",
    };
    if let Some(rules) = pattern_rules(denial, what)? {
        return Ok(rules);
    }
    let mut names: Vec<PathBuf> = Vec::new();
    for held in held_names(&denial.path) {
        for name in super::both_firmlink_names(held) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    let mut rules = Vec::new();
    for name in names {
        let spelled = name.display().to_string();
        let filter = match (denial.any_case, denial.reach) {
            (false, Reach::AndBelow) => format!("(subpath {})", string(&spelled)?),
            (false, Reach::Itself) => format!("(literal {})", string(&spelled)?),
            (true, reach) => {
                let tail = match reach {
                    Reach::AndBelow => "(/.*)?$",
                    Reach::Itself => "$",
                };
                let regex = format!("^{}{tail}", either_case(&escaped(&spelled)));
                format!("(regex {})", string(&regex)?)
            }
        };
        rules.push(format!("(deny {what} {filter})"));
    }
    Ok(rules)
}

/// The rules that deny `denial` where its path holds a [`super::ANY_NAME`] part (SK-9): what is
/// above the first such part as the kernel names it and across a firmlink, each such part one
/// name, and the rest as written. `None` for a path with no such part.
fn pattern_rules(denial: &Denial, what: &str) -> Result<Option<Vec<String>>, &'static str> {
    let parts: Vec<String> = denial
        .path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    let Some(first) = parts.iter().position(|part| part == super::ANY_NAME) else {
        return Ok(None);
    };
    let above: PathBuf = parts[..first].iter().collect();
    let below = parts[first..]
        .iter()
        .map(|part| {
            if part == super::ANY_NAME {
                "[^/]+".to_owned()
            } else {
                escaped(part)
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    let tail = match denial.reach {
        Reach::AndBelow => "(/.*)?$",
        Reach::Itself => "$",
    };
    let mut rules: Vec<String> = Vec::new();
    for name in super::both_firmlink_names(real(&above)) {
        let prefix = escaped(&name.display().to_string());
        let rule = format!(
            "(deny {what} (regex {}))",
            string(&format!("^{prefix}/{below}{tail}"))?
        );
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }
    Ok(Some(rules))
}

/// `text` with every regular-expression character escaped.
pub fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.^$|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A regular expression for `below` and its contents under `dir`, both as the kernel names them.
pub fn under(dir: &Path, below: &str) -> String {
    format!("^{}/{below}", escaped(&real(dir).display().to_string()))
}

/// The rules that keep every [`PLANTED`] name, at any depth below `root`, from being written.
pub fn planted_rules(root: &Path) -> Result<Vec<String>, &'static str> {
    let root = escaped(&real(root).display().to_string());
    PLANTED
        .iter()
        .map(|planted| {
            let tail = match planted.reach {
                Reach::Itself => "$",
                Reach::AndBelow => "(/.*)?$",
            };
            let mut body = String::new();
            let parts: Vec<&str> = planted.path.split('/').collect();
            for (at, part) in parts.iter().enumerate() {
                if *part == "**" {
                    body.push_str("(.*/)?");
                    continue;
                }
                body.push_str(&escaped(part));
                if at + 1 < parts.len() {
                    body.push('/');
                }
            }
            let regex = format!("^{root}/(.*/)?{body}{tail}");
            Ok(format!("(deny file-write* (regex {}))", string(&regex)?))
        })
        .collect()
}

/// The one lookup the certificate check needs (#1337): Go programs on macOS, `gh` among them,
/// verify a host's certificate by asking the `trustd` agent, and fail every TLS connection
/// without it. The same rule Claude Code's `enableWeakerNetworkIsolation` adds to its profile.
/// The agent fetches the addresses a certificate names, past the egress proxy: a residual the
/// operator accepted (R1, 2026-10-13). On unless the project's `certificate-checks` is `false`
/// or policy locks it ([`super::CertificateChecks`]).
pub const TRUST: &str = "(allow mach-lookup (global-name \"com.apple.trustd.agent\"))";

/// `own`, widened by what the project's sandbox widens ([`super::Widened`]): its package caches
/// (D-1337-6), each tool's folder written whole, inside each of cargo's bare repositories and
/// never an entry itself, and cargo's few files; each folder purlis made pinned as an entry, so
/// it is never removed, renamed or swapped for a link; the later-code names denied at any
/// depth of each, as in every folder a chat writes, and a bare repository's `config` and
/// `hooks` never written. And the certificate check, unless the project or policy turned it off
/// (D-1337-7, R1).
pub fn widen(own: &mut Own, widened: &super::Widened) -> Result<(), &'static str> {
    if let Some(caches) = &widened.caches {
        for tree in &caches.trees {
            own.allow.push(format!("(subpath {})", quote(tree)?));
            own.roots.push(tree.clone());
        }
        for bare in &caches.bare {
            own.allow
                .push(format!("(regex {})", string(&under(bare, "[^/]+/.+$"))?));
            own.roots.push(bare.clone());
            own.deny.push(format!(
                "(deny file-write* (regex {}))",
                string(&under(
                    bare,
                    &format!("[^/]+/({})(/.*)?$", super::caches::BARE_RUN.join("|"))
                ))?
            ));
        }
        for file in &caches.files {
            own.allow.push(format!("(literal {})", quote(file)?));
        }
        for folder in caches.folders() {
            own.deny
                .push(format!("(deny file-write* (literal {}))", quote(&folder)?));
        }
    }
    if widened.trust {
        own.after.push(TRUST.to_owned());
    }
    // What a person allowed this chat (D-sock-1): each socket by the path the kernel names it
    // by, connect only, and each service by its whole name, never a prefix.
    for socket in &widened.sockets {
        own.after.push(format!(
            "(allow network-outbound (remote unix-socket (path-literal {})))",
            string(&socket.display().to_string())?
        ));
    }
    for service in &widened.services {
        own.after.push(format!(
            "(allow mach-lookup (global-name {}))",
            string(service)?
        ));
    }
    // Each service on this Mac a person allowed this chat (qw384): connect to that one loopback
    // port, which a Go client reaches directly, never through the proxy.
    for port in &widened.ports {
        own.after.push(super::localport::seatbelt_rule(*port));
    }
    Ok(())
}

/// Lets a wrapped chat write each of `folders`, a person's grants (#1342): each in the
/// `file-write*` allow, and each a root the later-code names are denied at any depth in. Every
/// denial class follows them in the profile, so a class still wins inside one.
pub fn granted(own: &mut Own, folders: &[PathBuf]) -> Result<(), &'static str> {
    for folder in folders {
        // As judged, already the kernel's spelling (D-1342-12): never resolved again, so a
        // folder swapped for a link since is not followed to where it now points.
        own.allow.push(format!(
            "(subpath {})",
            string(&folder.display().to_string())?
        ));
        own.roots.push(folder.clone());
        // And every project manifest at any depth of it (D-T55-4): #1336 holds them by path
        // only in the chat's own folders, which a granted folder is not.
        let names = super::MANIFESTS
            .iter()
            .map(|name| escaped(name))
            .collect::<Vec<_>>()
            .join("|");
        let regex = format!(
            "^{}/(.*/)?({names})$",
            escaped(&folder.display().to_string())
        );
        own.deny
            .push(format!("(deny file-write* (regex {}))", string(&regex)?));
    }
    Ok(())
}

/// The keychain files of the home `home`, denied with the vaults class: no lookup of the
/// security service is allowed either, so a wrap holds a keyring vault ([`BASE`]).
pub fn keychains(home: Option<&Path>) -> Option<Denial> {
    home.map(|home| Denial {
        class: super::Class::Vaults,
        path: home.join("Library/Keychains"),
        access: Access::ReadWrite,
        named: None,
        reach: Reach::AndBelow,
        any_case: false,
    })
}

/// The Seatbelt profile for a chat in `cwd`, with its own temp directory `tmp`, denied
/// `denied`, keeping `own` for its harness, reaching the network through its own proxy's
/// `proxy_ports` alone (#1664: HTTP and SOCKS5, so never another chat's) and reporting on
/// `hook_socket`.
pub fn profile(
    denied: &[Denial],
    own: &Own,
    cwd: &Path,
    tmp: &Path,
    proxy_ports: &[u16],
    hook_socket: Option<&Path>,
) -> Result<String, &'static str> {
    let mut out = String::from(BASE);
    let mut line = |text: String| {
        out.push_str(&text);
        out.push('\n');
    };
    for port in proxy_ports {
        line(format!(
            "(allow network-outbound (remote ip \"localhost:{port}\"))"
        ));
    }
    if let Some(socket) = hook_socket {
        line(format!(
            "(allow network-outbound (remote unix-socket (path-literal {})))",
            quote(socket)?
        ));
    }

    // The socket's folder is held unwritten, as Claude Code's settings hold it (#1408), and as
    // every denial is: by both names, across a firmlink, its folders above pinned.
    let mut held: Vec<Denial> = denied.to_vec();
    if let Some(folder) = hook_socket.and_then(Path::parent) {
        held.push(Denial {
            class: super::Class::Integrity,
            path: folder.to_path_buf(),
            access: Access::Write,
            named: None,
            reach: Reach::AndBelow,
            any_case: false,
        });
    }
    let denied = held.as_slice();

    let cwd_real = real(cwd);
    for dir in &own.dirs {
        let dir = real(dir);
        if dir.starts_with(&cwd_real) || cwd_real.starts_with(&dir) {
            return Err(own.overlap);
        }
    }

    // What the chat may write.
    let mut allow = vec![
        format!("(subpath {})", quote(cwd)?),
        format!("(subpath {})", quote(tmp)?),
    ];
    allow.extend(own.allow.iter().cloned());
    line("(allow file-write*".to_owned());
    for rule in allow {
        line(format!("  {rule}"));
    }
    out.push_str(DEVICES);
    out.push_str(")\n");

    // What it may never write, after what it may: Seatbelt takes the last rule that matches.
    let mut line = |text: String| {
        out.push_str(&text);
        out.push('\n');
    };
    // Each under every name the kernel may give it: as it resolves, and across a macOS
    // firmlink (`/Users` and `/System/Volumes/Data/Users`), which resolving leaves as spelled
    // (#1356).
    for denial in denied {
        for rule in denial_rules(denial)? {
            line(rule);
        }
    }
    // Each directory between the chat's own and a denied path, as an entry only, so it is never
    // moved away with the denied path in it, nor replaced (measured: a plane-root chat could
    // otherwise move `.charter` aside and write `.charter/app` under its new name).
    let mut pinned = std::collections::BTreeSet::new();
    let mut pin = |folder: &Path, line: &mut dyn FnMut(String)| -> Result<(), &'static str> {
        let names = kernel_strings(folder)?;
        if pinned.insert(names.first().cloned()) {
            for name in names {
                line(format!("(deny file-write* (literal {name}))"));
            }
        }
        Ok(())
    };
    // The chat's own directory and its temp directory too, as entries: moved whole into the
    // other, every path rule under the one moved would no longer match (measured).
    for root in [&cwd_real, &real(tmp)] {
        pin(root, &mut line)?;
    }
    // Between a name of the chat's directory and the same side's name of the denied path.
    let cwd_names = super::kernel_names(&cwd_real);
    for denial in denied {
        for path in super::kernel_names(&denial.path) {
            for cwd_name in &cwd_names {
                for ancestor in super::ancestors_within(&path, cwd_name) {
                    pin(&ancestor, &mut line)?;
                }
            }
        }
    }
    // On every directory the chat may write, so a protected name made in one cannot be moved
    // into another with its parent.
    let mut roots = vec![cwd.to_path_buf(), tmp.to_path_buf()];
    roots.extend(own.roots.iter().cloned());
    for root in &roots {
        for rule in planted_rules(root)? {
            line(rule);
        }
        // No directory made under `.git/modules`, so a submodule's git directory, config and
        // hooks inside, is never moved in whole.
        line(format!(
            "(deny file-write-create (require-all (vnode-type DIRECTORY) (regex {})))",
            string(&format!(
                "^{}/(.*/)?\\.git/modules/.+",
                escaped(&real(root).display().to_string())
            ))?
        ));
    }
    for rule in own.deny.iter().chain(&own.after) {
        line(rule.clone());
    }
    // Last, so nothing given back reaches them: no project's vaults are read (#2071).
    for rule in vaults_everywhere_rules()? {
        line(rule);
    }
    if out.len() > MOST_PROFILE {
        return Err(TOO_LARGE);
    }
    Ok(out)
}

/// **The floor's profile** (#2071): files and the network as with no sandbox, but the denial
/// classes and the floor's `held` folders, and what the harness keeps for itself (`own.deny`,
/// `own.after`). Seatbelt takes the last rule that matches, so each denial follows the
/// `(allow default)` it narrows. What else stays held ([`FLOOR_HELD`]):
///
/// - nothing is started outside the profile: no Apple Event is sent, nothing is opened through
///   Launch Services, and no program that hands a job to launchd, cron or at is run;
/// - no unix socket but the chat's hook socket and the ones a person allowed it (the operator's
///   ruling Q1), whose folder is never written, and no port of this machine but the chat's own
///   proxy's `proxy_ports`, through which every host is reached;
/// - every denied path held as the full profile holds it: under each name the kernel may give it
///   (#1336, #1356), and every folder above it pinned as an entry up to `/`, so it is never moved
///   aside with what it holds; every name a later program runs ([`PLANTED`]) never written at any
///   depth anywhere, and no directory made under a `.git/modules`.
pub fn floor_profile(
    denied: &[Denial],
    held: &[PathBuf],
    own: &Own,
    proxy_ports: &[u16],
    hook_socket: Option<&Path>,
) -> Result<String, &'static str> {
    let mut out = String::from(FLOOR_BASE);
    out.push_str(FLOOR_HELD);
    let mut line = |text: String| {
        out.push_str(&text);
        out.push('\n');
    };
    for port in proxy_ports {
        line(format!(
            "(allow network-outbound (remote ip \"localhost:{port}\"))"
        ));
    }
    let mut all: Vec<Denial> = denied.to_vec();
    if let Some(socket) = hook_socket {
        line(format!(
            "(allow network-outbound (remote unix-socket (path-literal {})))",
            quote(socket)?
        ));
        // Its folder is never written, so the socket is never replaced (#1408).
        if let Some(folder) = socket.parent() {
            all.push(Denial {
                class: super::Class::Integrity,
                path: folder.to_path_buf(),
                access: Access::Write,
                named: None,
                reach: Reach::AndBelow,
                any_case: false,
            });
        }
    }
    all.extend(held.iter().map(|path| Denial {
        class: super::Class::LaterCode,
        path: path.clone(),
        access: Access::Write,
        named: None,
        reach: Reach::AndBelow,
        any_case: false,
    }));
    for denial in &all {
        for rule in denial_rules(denial)? {
            line(rule);
        }
    }
    // Every folder above a denied path, as an entry: never moved away with it, nor replaced.
    let mut pinned = std::collections::BTreeSet::new();
    for denial in &all {
        for path in super::kernel_names(&denial.path) {
            for ancestor in path.ancestors().skip(1) {
                if ancestor.parent().is_none() {
                    continue;
                }
                for name in kernel_strings(ancestor)? {
                    if pinned.insert(name.clone()) {
                        line(format!("(deny file-write* (literal {name}))"));
                    }
                }
            }
        }
    }
    for rule in floor_planted_rules()? {
        line(rule);
    }
    line(
        "(deny file-write-create (require-all (vnode-type DIRECTORY) (regex #\"^/(.*/)?\\.git/modules/.+\")))"
            .to_owned(),
    );
    for rule in every_project_rules()? {
        line(rule);
    }
    for rule in own.deny.iter().chain(&own.after) {
        line(rule.clone());
    }
    // Last, so nothing given back reaches them: no project's vaults are read (#2071).
    for rule in vaults_everywhere_rules()? {
        line(rule);
    }
    if out.len() > MOST_PROFILE {
        return Err(TOO_LARGE);
    }
    Ok(out)
}

/// **What the floor keeps from being written in every project on this machine** (#2071): its
/// state folder, under every spelling, and its manifests, under every name, wherever they are
/// and however a name is spelled. The full sandbox writes no project but the chat's own, and
/// none of these in it; at the floor every folder is written, so each is held by name.
pub fn every_project_rules() -> Result<Vec<String>, &'static str> {
    let mut rules = Vec::new();
    for state in crate::names::STATE_DIR.spellings() {
        let regex = format!("^/(.*/)?{}(/.*)?$", either_case(&escaped(state)));
        rules.push(format!("(deny file-write* (regex {}))", string(&regex)?));
    }
    for manifest in super::MANIFESTS {
        let regex = format!("^/(.*/)?{}$", either_case(&escaped(manifest)));
        rules.push(format!("(deny file-write* (regex {}))", string(&regex)?));
    }
    // And every workspace's manifest (#2046), in any case.
    rules.push(format!(
        "(deny file-write* (regex {}))",
        string(&workspace_manifests_regex())?
    ));
    Ok(rules)
}

/// The pattern every project's workspace manifests match, `…/workspaces/<workspace>/workspace.json`,
/// in any case (#2046).
pub fn workspace_manifests_regex() -> String {
    format!(
        "^/(.*/)?{}/[^/]+/{}$",
        either_case(&escaped(super::WORKSPACES)),
        either_case(&escaped(super::WORKSPACE_MANIFEST))
    )
}

/// The pattern plane `root`'s `workspaces` folder and each workspace's folder in it match, the
/// entries alone, in any case (#2046): `<root>/workspaces` and `<root>/workspaces/<workspace>`,
/// under each name the kernel gives the root.
pub fn workspace_folders_regexes(root: &Path) -> Vec<String> {
    super::kernel_names(root)
        .iter()
        .map(|name| {
            format!(
                "^{}/{}(/[^/]+)?$",
                either_case(&escaped(&name.display().to_string())),
                either_case(&escaped(super::WORKSPACES))
            )
        })
        .collect()
}

/// **No workspace's folder of `planes` is made, moved or removed by a chat** (#2046): a
/// workspace's manifest is held by its path, and a folder moved aside would take it out of that
/// path to be written and back. Directories alone, and their entries alone: what is in a
/// workspace is written as ever, and a folder called `workspaces` anywhere else is any folder.
pub fn workspace_folder_rules(planes: &[PathBuf]) -> Result<Vec<String>, &'static str> {
    let mut rules = Vec::new();
    for plane in planes {
        for regex in workspace_folders_regexes(plane) {
            let rule = format!(
                "(deny file-write* (require-all (vnode-type DIRECTORY) (regex {})))",
                string(&regex)?
            );
            if !rules.contains(&rule) {
                rules.push(rule);
            }
        }
    }
    Ok(rules)
}

/// **No project's vaults are read, at either level** (#2071): the vaults and the fingerprint key
/// under every spelling of a state folder, wherever it is and however it is spelled. The
/// chat's own project's are denied by path besides ([`super::Denied::of`]).
pub fn vaults_everywhere_rules() -> Result<Vec<String>, &'static str> {
    let mut rules = Vec::new();
    for state in crate::names::STATE_DIR.spellings() {
        for (kept, tail) in [
            ("vaults", "(/.*)?$"),
            (crate::secrets::fingerprint::KEY_FILE, "$"),
        ] {
            let regex = format!(
                "^/(.*/)?{}{tail}",
                either_case(&format!("{}/{}", escaped(state), escaped(kept)))
            );
            rules.push(format!(
                "(deny file-read* file-write* (regex {}))",
                string(&regex)?
            ));
        }
    }
    Ok(rules)
}

/// What the floor holds besides files ([`floor_profile`]): nothing started outside the profile,
/// no unix socket and no port of this machine. The hook socket, the proxy's ports and what a
/// person allowed are given back after it.
pub const FLOOR_HELD: &str = "(deny appleevent-send)
(deny lsopen)
(deny job-creation)
(deny mach-lookup
  (global-name \"com.apple.coreservices.launchservicesd\")
  (global-name \"com.apple.coreservices.appleevents\"))
(deny process-exec
  (literal \"/bin/launchctl\")
  (literal \"/usr/bin/osascript\")
  (literal \"/usr/bin/open\")
  (literal \"/usr/bin/crontab\")
  (literal \"/usr/bin/at\")
  (literal \"/usr/bin/batch\"))
(deny network-outbound (remote unix-socket))
(deny network-outbound (remote ip \"localhost:*\"))
";

/// The rules that keep every [`PLANTED`] name from being written, at any depth anywhere: the
/// floor lets a chat write every folder, so each name is held wherever it is.
pub fn floor_planted_rules() -> Result<Vec<String>, &'static str> {
    PLANTED
        .iter()
        .map(|planted| {
            let tail = match planted.reach {
                Reach::Itself => "$",
                Reach::AndBelow => "(/.*)?$",
            };
            let mut body = String::new();
            let parts: Vec<&str> = planted.path.split('/').collect();
            for (at, part) in parts.iter().enumerate() {
                if *part == "**" {
                    body.push_str("(.*/)?");
                    continue;
                }
                body.push_str(&escaped(part));
                if at + 1 < parts.len() {
                    body.push('/');
                }
            }
            // Each letter either case: the disk compares names case-folded, so a name spelled
            // another way is the same file.
            let regex = format!("^/(.*/)?{}{tail}", either_case(&body));
            Ok(format!("(deny file-write* (regex {}))", string(&regex)?))
        })
        .collect()
}

/// `pattern` with each ASCII letter as a class of both its cases (`a` as `[aA]`), so a name
/// matches however it is spelled on a disk that compares names case-folded.
pub fn either_case(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() * 4);
    let mut escaped = false;
    for c in pattern.chars() {
        if escaped {
            out.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => {
                out.push(c);
                escaped = true;
            }
            c if c.is_ascii_alphabetic() => {
                out.push('[');
                out.push(c.to_ascii_lowercase());
                out.push(c.to_ascii_uppercase());
                out.push(']');
            }
            c => out.push(c),
        }
    }
    out
}

/// What the floor's profile starts from (#2071): everything a process may do, as with no
/// sandbox, narrowed after it by [`floor_profile`].
const FLOOR_BASE: &str = "(version 1)
(allow default)
";

/// The profile a program answers `--version` inside, before a sandboxed chat starts on it
/// (D-88k): no network at all, and nothing written but `tmp`, the probe's own temp directory,
/// and `/dev/null`. Stricter than any chat's, so a program that loads a file a chat wrote
/// gets nothing out of the probe.
pub fn probe_profile(tmp: &Path) -> Result<String, &'static str> {
    Ok(format!(
        "{BASE}(allow file-write* (subpath {}) (literal \"/dev/null\"))\n",
        quote(tmp)?
    ))
}

/// What every wrapped chat is allowed, whatever its plane: each line was needed by a harness
/// or by a command it ran (measured), and nothing here reaches the network or writes a file.
const BASE: &str = "(version 1)
(deny default)
(allow process-exec)
(allow process-fork)
(allow signal (target same-sandbox))
(allow process-info* (target same-sandbox))
(allow sysctl-read)
(allow file-read*)
(allow file-ioctl (literal \"/dev/ptmx\") (literal \"/dev/tty\") (regex #\"^/dev/ttys[0-9]+$\"))
(allow pseudo-tty)
(allow ipc-posix-shm-read* (ipc-posix-name-prefix \"apple.cfprefs.\"))
(allow ipc-posix-sem)
(allow user-preference-read)
(allow mach-lookup
  (global-name \"com.apple.system.opendirectoryd.libinfo\")
  (global-name \"com.apple.cfprefsd.daemon\")
  (global-name \"com.apple.cfprefsd.agent\")
  (global-name \"com.apple.logd\")
  (global-name \"com.apple.FSEvents\"))
(allow system-socket)
";

/// The devices a terminal program writes, inside the `file-write*` allow.
const DEVICES: &str = "  (literal \"/dev/null\")
  (literal \"/dev/tty\")
  (literal \"/dev/ptmx\")
  (regex #\"^/dev/ttys[0-9]+$\")";

/// The variables that point a wrapped chat's traffic at charter's egress proxy. Both cases,
/// because programs disagree on which they read.
pub const PROXY_ENV: [&str; 6] = [
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "ALL_PROXY",
    "https_proxy",
    "http_proxy",
    "all_proxy",
];

/// The variables that would send a host past the proxy, emptied. The profile refuses such a
/// connection anyway, and this way it is refused by the proxy, which says why.
pub const NO_PROXY_ENV: [&str; 2] = ["NO_PROXY", "no_proxy"];

/// What tools read in place of `TMPDIR`, each pointed into a wrapped chat's own temp directory
/// (#1120, measured on macOS 26.2): `TMP` and `TEMP`, and `xcrun`'s cache and clang's module
/// cache by the name below it, which otherwise go to the system's per-user temp and cache
/// folders (`/var/folders/…/T` and `…/C`). No chat may write those, so clang and Swift fail
/// on the module cache without this. Fresh each start: no chat loads a module another built.
pub const TEMP_ENV: [(&str, &str); 4] = [
    ("TMP", ""),
    ("TEMP", ""),
    ("xcrun_db", "xcrun_db"),
    ("CLANG_MODULE_CACHE_PATH", "clang-module-cache"),
];

/// What a wrapped chat's environment gains: its traffic pointed at the proxy at `proxy`, the
/// way past it emptied, and its own temp directory `tmp`, under every name in [`TEMP_ENV`] too.
/// `mktemp` and Foundation's `NSTemporaryDirectory` read no variable at all, and are refused:
/// a block sorted as `sandboxblock::Kind::SystemTemp`, whose notice gives `$TMPDIR` as the way round.
pub fn env(proxy: &str, tmp: &Path) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = PROXY_ENV
        .iter()
        .map(|key| ((*key).to_owned(), proxy.to_owned()))
        .collect();
    env.extend(
        NO_PROXY_ENV
            .iter()
            .map(|key| ((*key).to_owned(), String::new())),
    );
    env.push(("TMPDIR".to_owned(), tmp.display().to_string()));
    env.extend(TEMP_ENV.iter().map(|(key, below)| {
        let at = if below.is_empty() {
            tmp.to_path_buf()
        } else {
            tmp.join(below)
        };
        ((*key).to_owned(), at.display().to_string())
    }));
    env
}

#[cfg(test)]
#[path = "seatbelt_tests.rs"]
mod tests;
