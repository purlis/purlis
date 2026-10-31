// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The rules a wrapped chat is judged by: the profile purlis compiles, read.**
//!
//! purlis compiles one policy into one profile for each wrapped chat (ADR 0067 §2), in the
//! language of macOS's Seatbelt. On Linux the supervisor judges each connect and each file
//! operation it performs by the same profile, read here, so the two systems hold the same
//! classes from the same compile and cannot drift apart.
//!
//! What is read is what purlis writes, and nothing else: `allow` and `deny` rules on files and
//! on outbound connections, with `subpath`, `literal`, `regex`, `vnode-type`, `require-all`,
//! `require-any` and `require-not` filters, `remote ip` and `remote unix-socket`, and the one
//! socket a chat may serve on, `network-bind` with `local unix-socket` (#2019; a listen's
//! `network-inbound` has no counterpart here, where a socket is held at its bind). The last
//! rule that matches an operation decides it, as in Seatbelt; none matching refuses it. A rule
//! on something Linux has no counterpart of (a Mach service, an Apple event) is set aside. Any
//! other construct refuses the whole profile, so a rule purlis starts writing is never ignored
//! here unseen.
//!
//! `process-exec` is read for the kernel's own rule ([`Profile::starts_from`]), which names what
//! is beneath a path: an `allow` of it says folders or files by `subpath` or `literal`, or
//! everything, and any other filter refuses the profile. The kernel's rule takes nothing back, so
//! a `deny` of it with a filter has no effect here (the floor's, of programs that start something
//! outside the sandbox on macOS).

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use regex::Regex;

/// A profile, read.
#[derive(Debug, Clone)]
pub struct Profile {
    rules: Vec<Rule>,
}

/// Why a profile could not be read: the construct and where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread(pub String);

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the sandbox profile could not be read: {}", self.0)
    }
}

impl std::error::Error for Unread {}

/// An operation a supervisor asks about.
#[derive(Debug, Clone, Copy)]
pub enum Op<'a> {
    /// On the file at `path`, as the kernel names it, of kind `kind`.
    File {
        what: FileOp,
        path: &'a Path,
        kind: Kind,
    },
    /// An outbound connection to `remote`.
    Connect(Remote<'a>),
    /// Binding a unix socket at `path`, to serve there (#2019).
    BindUnix(&'a Path),
}

/// What is done to a file, by the name Seatbelt gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOp {
    /// `file-read-data`: its contents, or a folder's entries.
    ReadData,
    /// `file-read-metadata`: what it is, never what is in it.
    ReadMetadata,
    /// `file-write-data`: its contents changed, or truncated.
    WriteData,
    /// `file-write-create`: made, or moved or linked into place.
    Create,
    /// `file-write-unlink`: removed, or moved away.
    Unlink,
    /// `file-write-mode`: its permissions changed.
    Mode,
    /// `file-write-times`: its access or modification time changed.
    Times,
}

impl FileOp {
    /// Seatbelt's word for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::ReadData => "file-read-data",
            Self::ReadMetadata => "file-read-metadata",
            Self::WriteData => "file-write-data",
            Self::Create => "file-write-create",
            Self::Unlink => "file-write-unlink",
            Self::Mode => "file-write-mode",
            Self::Times => "file-write-times",
        }
    }
}

/// What a file is, as `vnode-type` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Regular,
    Directory,
    Symlink,
    Fifo,
    Socket,
    CharacterDevice,
    BlockDevice,
}

impl Kind {
    fn named(name: &str) -> Option<Kind> {
        Some(match name {
            "REGULAR-FILE" => Self::Regular,
            "DIRECTORY" => Self::Directory,
            "SYMLINK" => Self::Symlink,
            "FIFO" => Self::Fifo,
            "SOCKET" => Self::Socket,
            "TTY" | "CHARACTER-DEVICE" => Self::CharacterDevice,
            "BLOCK-DEVICE" => Self::BlockDevice,
            _ => return None,
        })
    }
}

/// Where a connection goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remote<'a> {
    /// An internet address and port.
    Ip { ip: IpAddr, port: u16 },
    /// A unix socket by its path, or `None` for one with no path (an abstract or unnamed one).
    Unix(Option<&'a Path>),
}

/// What a profile writes and reaches, for the kernel's own rules beneath the supervisor's: the
/// supervisor itself is held to them (Landlock), so a mistake in judging cannot reach past them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reach {
    /// Every folder or file a rule may let a chat write, each the deepest fixed path the rule
    /// begins with: `/` where the profile allows everything.
    pub written: Vec<PathBuf>,
    /// Every port of this machine a chat may connect to.
    pub ports: Vec<u16>,
    /// Every unix socket a chat may serve on (#2019), as `local unix-socket` names it.
    pub serves: Vec<PathBuf>,
    /// Whether a chat may reach other machines directly (the floor).
    pub any_host: bool,
}

#[derive(Debug, Clone)]
struct Rule {
    allow: bool,
    /// Each operation it names, `default` and wildcards such as `file-write*` included.
    ops: Vec<String>,
    /// Any of these matches; none at all matches everything.
    filters: Vec<Filter>,
}

#[derive(Debug, Clone)]
enum Filter {
    Subpath(PathBuf),
    Literal(PathBuf),
    Regex(Regex),
    Vnode(Kind),
    All(Vec<Filter>),
    Any(Vec<Filter>),
    Not(Box<Filter>),
    /// `remote ip "localhost:<port>"`, `"localhost:*"` or `"*:*"`; or, held to one loopback
    /// address, `"127.0.0.1:<port>"` or `"[::1]:<port>"` (a confined extension's proxy).
    Ip {
        local_only: bool,
        /// The one loopback address it names, where it names one rather than `localhost`.
        exact: Option<IpAddr>,
        port: Option<u16>,
    },
    /// `remote unix-socket`, with its `path-literal` where it names one.
    Unix(Option<PathBuf>),
    /// `local unix-socket (path-literal …)` (#2019): the one socket a chat may bind.
    LocalUnix(PathBuf),
}

/// Operations Linux has a counterpart of, and which this module judges. `file-link` is read
/// and names no operation asked here: the supervisor judges a link as a write of the file it
/// names and a create of the new name, so the write rules beside it already decide it.
const JUDGED: [&str; 14] = [
    "default",
    "process-exec",
    "file*",
    "file-link",
    "file-read*",
    "file-read-data",
    "file-read-metadata",
    "file-write*",
    "file-write-create",
    "file-write-data",
    "file-write-unlink",
    "file-write-mode",
    "network-outbound",
    "network-bind",
];

/// Operations purlis writes rules for that this module does not judge.
const SET_ASIDE: [&str; 17] = [
    "process-fork",
    "process-info*",
    "signal",
    "sysctl-read",
    "file-ioctl",
    "pseudo-tty",
    "ipc-posix-shm-read*",
    "ipc-posix-sem",
    "user-preference-read",
    "mach-lookup",
    "system-socket",
    "appleevent-send",
    "lsopen",
    "job-creation",
    "iokit-open",
    "file-write-setugid",
    "network-inbound",
];

impl Profile {
    /// `text`, read, or why it could not be.
    pub fn parse(text: &str) -> Result<Profile, Unread> {
        let forms = Parser::new(text).forms()?;
        let mut rules = Vec::new();
        for form in forms {
            let Expr::List(items) = form else {
                return Err(Unread(format!("a word outside a rule: {form:?}")));
            };
            let Some(Expr::Symbol(head)) = items.first() else {
                return Err(Unread("an empty or unnamed form".to_owned()));
            };
            match head.as_str() {
                "version" => continue,
                "allow" | "deny" => {}
                other => return Err(Unread(format!("the form `{other}`"))),
            }
            let allow = head == "allow";
            let mut ops = Vec::new();
            let mut rest = &items[1..];
            while let Some(Expr::Symbol(op)) = rest.first() {
                ops.push(op.clone());
                rest = &rest[1..];
            }
            if ops.is_empty() {
                return Err(Unread(format!("a rule naming no operation: {items:?}")));
            }
            let judged = ops
                .iter()
                .filter(|op| JUDGED.contains(&op.as_str()))
                .count();
            let aside = ops
                .iter()
                .filter(|op| SET_ASIDE.contains(&op.as_str()))
                .count();
            if judged + aside != ops.len() {
                return Err(Unread(format!("an operation in {ops:?}")));
            }
            if judged == 0 {
                continue;
            }
            if aside != 0 {
                return Err(Unread(format!("a rule mixing operations: {ops:?}")));
            }
            let filters = rest.iter().map(filter).collect::<Result<Vec<_>, _>>()?;
            if allow
                && ops.iter().any(|op| names(op, "process-exec"))
                && !filters
                    .iter()
                    .all(|it| matches!(it, Filter::Subpath(_) | Filter::Literal(_)))
            {
                return Err(Unread(format!(
                    "a start of a program allowed by what the kernel cannot name: {filters:?}"
                )));
            }
            rules.push(Rule {
                allow,
                ops,
                filters,
            });
        }
        Ok(Profile { rules })
    }

    /// Whether `op` is allowed: the last rule that matches it decides, and none refuses it.
    pub fn allows(&self, op: &Op<'_>) -> bool {
        let name = match op {
            Op::File { what, .. } => what.name(),
            Op::Connect(_) => "network-outbound",
            Op::BindUnix(_) => "network-bind",
        };
        let mut allowed = false;
        for rule in &self.rules {
            if !rule.ops.iter().any(|it| names(it, name)) {
                continue;
            }
            if rule.filters.is_empty() || rule.filters.iter().any(|it| it.matches(op)) {
                allowed = rule.allow;
            }
        }
        allowed
    }

    /// **Whether a chat's server may be bound at `path`** (#2019): an absolute path the profile
    /// names to serve on (`local unix-socket (path-literal …)`, never `default` alone), where it
    /// may make and take away a socket. The one unix bind the supervisor makes for a chat.
    pub fn serves_at(&self, path: &Path) -> bool {
        let file = |what| Op::File {
            what,
            path,
            kind: Kind::Socket,
        };
        path.is_absolute()
            && self.reach().serves.iter().any(|served| served == path)
            && self.allows(&Op::BindUnix(path))
            && self.allows(&file(FileOp::Create))
            && self.allows(&file(FileOp::Unlink))
    }

    /// **Where a program may be started from**, for the kernel's own rule: each folder or file
    /// the profile's rules allow it beneath, `/` where they allow it everywhere, and nothing
    /// where none does. A `deny` with no filter takes back what came before it; one with a filter
    /// is the kernel's to leave (see the module's text).
    pub fn starts_from(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        for rule in &self.rules {
            if !rule.ops.iter().any(|it| names(it, "process-exec")) {
                continue;
            }
            if !rule.allow {
                if rule.filters.is_empty() {
                    out.clear();
                }
                continue;
            }
            let roots = if rule.filters.is_empty() {
                vec![PathBuf::from("/")]
            } else {
                rule.filters.iter().map(Filter::root).collect()
            };
            for root in roots {
                if !out.contains(&root) {
                    out.push(root);
                }
            }
        }
        out
    }

    /// What the profile writes and reaches ([`Reach`]).
    pub fn reach(&self) -> Reach {
        let mut reach = Reach::default();
        let add = |path: PathBuf, reach: &mut Reach| {
            if !reach.written.contains(&path) {
                reach.written.push(path);
            }
        };
        for rule in self.rules.iter().filter(|rule| rule.allow) {
            let writes = rule
                .ops
                .iter()
                .any(|op| op == "default" || op == "file*" || op.starts_with("file-write"));
            let connects = rule
                .ops
                .iter()
                .any(|op| op == "default" || op == "network-outbound");
            if writes {
                if rule.filters.is_empty() {
                    add(PathBuf::from("/"), &mut reach);
                }
                for filter in &rule.filters {
                    add(filter.root(), &mut reach);
                }
            }
            if rule.ops.iter().any(|op| op == "network-bind") {
                for filter in &rule.filters {
                    if let Filter::LocalUnix(path) = filter
                        && !reach.serves.contains(path)
                    {
                        reach.serves.push(path.clone());
                        // The supervisor makes the socket there, and takes away one a server
                        // left: the folder's, whether or not the socket is there yet.
                        if let Some(folder) = path.parent() {
                            add(folder.to_path_buf(), &mut reach);
                        }
                    }
                }
            }
            if connects {
                if rule.filters.is_empty() {
                    reach.any_host = true;
                }
                for filter in &rule.filters {
                    match filter {
                        Filter::Ip {
                            local_only: true,
                            port: Some(port),
                            ..
                        } => {
                            if !reach.ports.contains(port) {
                                reach.ports.push(*port);
                            }
                        }
                        Filter::Ip { .. } => reach.any_host = true,
                        _ => {}
                    }
                }
            }
        }
        reach
    }
}

/// Whether a rule's operation `rule` names the operation `op`.
/// `default` never names a bind: a chat binds only a socket a rule names (#2019).
fn names(rule: &str, op: &str) -> bool {
    (rule == "default" && op != "network-bind")
        || rule == op
        || rule
            .strip_suffix('*')
            .is_some_and(|prefix| op.starts_with(prefix))
}

impl Filter {
    fn matches(&self, op: &Op<'_>) -> bool {
        match (self, op) {
            (Self::All(all), _) => all.iter().all(|it| it.matches(op)),
            (Self::Any(any), _) => any.iter().any(|it| it.matches(op)),
            (Self::Not(it), _) => !it.matches(op),
            (Self::Subpath(root), Op::File { path, .. }) => path.starts_with(root),
            (Self::Literal(it), Op::File { path, .. }) => path == it,
            (Self::Regex(regex), Op::File { path, .. }) => regex.is_match(&path.to_string_lossy()),
            (Self::Vnode(kind), Op::File { kind: it, .. }) => kind == it,
            (
                Self::Ip {
                    local_only,
                    exact,
                    port,
                },
                Op::Connect(Remote::Ip { ip, port: to }),
            ) => {
                let at = match exact {
                    Some(exact) => same_address(*exact, *ip),
                    None => !local_only || loopback(*ip),
                };
                at && port.is_none_or(|port| port == *to)
            }
            (Self::LocalUnix(path), Op::BindUnix(to)) => path.as_path() == *to,
            (Self::Unix(path), Op::Connect(Remote::Unix(to))) => match path {
                None => true,
                Some(path) => *to == Some(path.as_path()),
            },
            _ => false,
        }
    }

    /// The deepest fixed path this filter begins with, for [`Reach::written`].
    fn root(&self) -> PathBuf {
        match self {
            Self::Subpath(path) | Self::Literal(path) => path.clone(),
            Self::Regex(regex) => {
                let fixed = fixed_prefix(regex.as_str());
                match fixed.rfind('/') {
                    Some(0) | None => PathBuf::from("/"),
                    Some(at) => PathBuf::from(&fixed[..at]),
                }
            }
            Self::All(all) => all
                .iter()
                .map(Filter::root)
                .max_by_key(|path| path.as_os_str().len())
                .unwrap_or_else(|| PathBuf::from("/")),
            _ => PathBuf::from("/"),
        }
    }
}

/// Whether `ip` is `exact`, an IPv4 address also by its IPv4-mapped IPv6 form.
fn same_address(exact: IpAddr, ip: IpAddr) -> bool {
    let plain = |ip: IpAddr| match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    };
    plain(exact) == plain(ip)
}

/// Whether `ip` is this machine's own loopback, as Seatbelt's `localhost` means it.
fn loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

/// The text a regular expression anchored with `^` matches before its first special character.
fn fixed_prefix(regex: &str) -> String {
    let body = regex.trim_start_matches("(?s)").trim_start_matches('^');
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(escaped) if !escaped.is_ascii_alphanumeric() => out.push(escaped),
                _ => break,
            },
            '.' | '^' | '$' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}' => break,
            c => out.push(c),
        }
    }
    out
}

fn filter(expr: &Expr) -> Result<Filter, Unread> {
    let Expr::List(items) = expr else {
        return Err(Unread(format!("a filter that is not a list: {expr:?}")));
    };
    let (Some(Expr::Symbol(head)), rest) = (items.first(), &items[1.min(items.len())..]) else {
        return Err(Unread("an unnamed filter".to_owned()));
    };
    let one_string = || -> Result<&str, Unread> {
        match rest {
            [Expr::Str(text)] => Ok(text.as_str()),
            _ => Err(Unread(format!("`{head}` without one string"))),
        }
    };
    Ok(match head.as_str() {
        "subpath" => Filter::Subpath(PathBuf::from(one_string()?)),
        "literal" => Filter::Literal(PathBuf::from(one_string()?)),
        "regex" => {
            let pattern = one_string()?;
            // Seatbelt's `.` matches every character, a line break among them.
            Filter::Regex(
                Regex::new(&format!("(?s){pattern}"))
                    .map_err(|why| Unread(format!("the pattern {pattern:?}: {why}")))?,
            )
        }
        "vnode-type" => match rest {
            [Expr::Symbol(kind)] => Filter::Vnode(
                Kind::named(kind).ok_or_else(|| Unread(format!("the vnode type {kind}")))?,
            ),
            _ => return Err(Unread("`vnode-type` without one kind".to_owned())),
        },
        "require-all" => Filter::All(rest.iter().map(filter).collect::<Result<_, _>>()?),
        "require-any" => Filter::Any(rest.iter().map(filter).collect::<Result<_, _>>()?),
        "require-not" => match rest {
            [one] => Filter::Not(Box::new(filter(one)?)),
            _ => return Err(Unread("`require-not` without one filter".to_owned())),
        },
        "remote" => remote(rest)?,
        "local" => local(rest)?,
        other => return Err(Unread(format!("the filter `{other}`"))),
    })
}

/// `local unix-socket (path-literal …)`, the one local filter purlis writes (#2019).
fn local(rest: &[Expr]) -> Result<Filter, Unread> {
    match rest {
        [Expr::Symbol(unix), Expr::List(path)] if unix == "unix-socket" => match &path[..] {
            [Expr::Symbol(head), Expr::Str(path)]
                if head == "path-literal" && path.starts_with('/') =>
            {
                Ok(Filter::LocalUnix(PathBuf::from(path)))
            }
            _ => Err(Unread(format!("the local socket filter {path:?}"))),
        },
        _ => Err(Unread(format!("the local filter {rest:?}"))),
    }
}

fn remote(rest: &[Expr]) -> Result<Filter, Unread> {
    match rest {
        [Expr::Symbol(ip), Expr::Str(at)] if ip == "ip" => {
            let (host, port) = at
                .rsplit_once(':')
                .ok_or_else(|| Unread(format!("the address {at:?}")))?;
            let (local_only, exact) = match host {
                "localhost" => (true, None),
                "*" => (false, None),
                "127.0.0.1" => (true, Some(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))),
                "[::1]" => (true, Some(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST))),
                _ => return Err(Unread(format!("the host {host:?}"))),
            };
            let port = match port {
                "*" => None,
                port => Some(
                    port.parse()
                        .map_err(|_| Unread(format!("the port {port:?}")))?,
                ),
            };
            Ok(Filter::Ip {
                local_only,
                exact,
                port,
            })
        }
        [Expr::Symbol(unix)] if unix == "unix-socket" => Ok(Filter::Unix(None)),
        [Expr::Symbol(unix), Expr::List(path)] if unix == "unix-socket" => match &path[..] {
            [Expr::Symbol(head), Expr::Str(path)] if head == "path-literal" => {
                Ok(Filter::Unix(Some(PathBuf::from(path))))
            }
            _ => Err(Unread(format!("the socket filter {path:?}"))),
        },
        _ => Err(Unread(format!("the remote filter {rest:?}"))),
    }
}

#[derive(Debug, Clone)]
enum Expr {
    Symbol(String),
    Str(String),
    List(Vec<Expr>),
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Parser {
            chars: text.chars().peekable(),
        }
    }

    fn forms(mut self) -> Result<Vec<Expr>, Unread> {
        let mut out = Vec::new();
        while self.skip_blank() {
            out.push(self.expr()?);
        }
        Ok(out)
    }

    /// Past white space and comments; whether anything is left.
    fn skip_blank(&mut self) -> bool {
        loop {
            match self.chars.peek() {
                Some(c) if c.is_whitespace() => {
                    self.chars.next();
                }
                Some(';') => {
                    for c in self.chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some(_) => return true,
                None => return false,
            }
        }
    }

    fn expr(&mut self) -> Result<Expr, Unread> {
        match self.chars.next() {
            Some('(') => {
                let mut items = Vec::new();
                loop {
                    if !self.skip_blank() {
                        return Err(Unread("a rule left open".to_owned()));
                    }
                    if self.chars.peek() == Some(&')') {
                        self.chars.next();
                        return Ok(Expr::List(items));
                    }
                    items.push(self.expr()?);
                }
            }
            Some('"') => self.string(),
            Some('#') if self.chars.peek() == Some(&'"') => {
                self.chars.next();
                let mut out = String::new();
                for c in self.chars.by_ref() {
                    if c == '"' {
                        return Ok(Expr::Str(out));
                    }
                    out.push(c);
                }
                Err(Unread("a string left open".to_owned()))
            }
            Some(')') => Err(Unread("a `)` with no rule".to_owned())),
            Some(c) => {
                let mut out = String::from(c);
                while let Some(&c) = self.chars.peek() {
                    if c.is_whitespace() || c == '(' || c == ')' || c == '"' {
                        break;
                    }
                    out.push(c);
                    self.chars.next();
                }
                Ok(Expr::Symbol(out))
            }
            None => Err(Unread("the end, inside a rule".to_owned())),
        }
    }

    fn string(&mut self) -> Result<Expr, Unread> {
        let mut out = String::new();
        while let Some(c) = self.chars.next() {
            match c {
                '"' => return Ok(Expr::Str(out)),
                '\\' => match self.chars.next() {
                    Some(escaped @ ('\\' | '"')) => out.push(escaped),
                    other => return Err(Unread(format!("the escape \\{other:?}"))),
                },
                c => out.push(c),
            }
        }
        Err(Unread("a string left open".to_owned()))
    }
}

#[cfg(test)]
mod tests;
