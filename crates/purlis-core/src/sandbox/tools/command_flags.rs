// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Flags that name a command** (#5799, ADR 0112): the forms of a program kept offered a Run
//! grant that would have it run a command, or read a file that can name one, which the person
//! reading the exact command would not see run.
//!
//! `ssh`, `scp`, `sftp`, `rsync`, `kubectl`, `psql`, `mysql`, `zip` and `wget` are the usual reason a chat
//! is handed a key, so each stays offered Allow once on the exact command (D-5619-7). Only the
//! forms below are refused: `ssh -o ProxyCommand`, `-F` and `-I`, `scp -S`, `sftp -b`,
//! `rsync -e`, `zip -TT`, `wget --use-askpass` and `-e`, `kubectl --kubeconfig`, `psql -f`, `-o`
//! and a backslash command, and the like. Read as their own parsers read them, and more widely
//! where a spelling is in doubt: short flags in a cluster (`-avze`), a value in the same word or
//! the next, a long option by any of its abbreviations (`--rs` for `--rsh`), and every word up to
//! a `--`, wherever the program would stop reading flags. A form caught by chance loses an
//! Allow and nothing more, as a refused name does (D-5424-1).
//!
//! A granted run starts with no stdin and a home of its own ([`super::home_of_its_own`]), so no
//! settings file and no prompt of the chat's is read: what a run reads is in its command.

/// One program's flags as its parser reads them.
struct Flags {
    /// The names it goes by.
    names: &'static [&'static str],
    /// Short flags that take a value: the rest of their word, or the next word.
    valued: &'static str,
    /// Short flags whose value is only the rest of their word, never the next (mysql's `-p`).
    attached: &'static str,
    /// Short flags refused whatever follows them.
    refused: &'static str,
    /// Text refused anywhere in a cluster of short flags: `zip`'s `-TT` is one flag of two letters.
    in_a_cluster: &'static [&'static str],
    /// Long options refused, by any abbreviation of theirs.
    long: &'static [&'static str],
    /// Flags refused for their value, by the flag as it is spelled without its dashes (a short
    /// flag's letter, or a long option's whole name); the form it is said as, where refused.
    value: fn(flag: &str, value: &str) -> Option<String>,
    /// Long options whose value [`Self::value`] reads, by their whole name.
    long_valued: &'static [&'static str],
    /// Long options spelled as mysql's: `_` for `-`, and a `loose-`, `enable-` or `maximum-`
    /// before the name.
    mysql_spelling: bool,
    /// Whether an operand's host is handed to ssh as a word of its own (rsync's `host:path`), so
    /// one starting with `-` would be an option of ssh's, even after a `--`.
    operands_reach_ssh: bool,
}

/// The options of ssh's own configuration that run a command or load code (`ssh_config(5)`).
const SSH_RUNS: &[&str] = &[
    "ProxyCommand",
    "LocalCommand",
    "PermitLocalCommand",
    "KnownHostsCommand",
    "PKCS11Provider",
    "SecurityKeyProvider",
    "Include",
    "Match",
    // run as X11 forwarding starts
    "XAuthLocation",
];

/// `-o Key=Value` or `-o "Key Value"`, where `Key` is one of [`SSH_RUNS`], whatever its case.
fn ssh_option(flag: &str, value: &str) -> Option<String> {
    // A jump host is pasted into the command ssh has a shell run (#5876's third review).
    if flag == "J" {
        return shell_would_read(value).then(|| "-J".to_owned());
    }
    if flag != "o" {
        return None;
    }
    // ssh splits the line with quotes and backslashes as a shell would, so a quoted name is
    // the same name (#5876's second review): judged with them taken out, and by its start, so
    // no spelling of what follows the name matters.
    let bare: String = value
        .chars()
        .filter(|c| !matches!(c, '"' | '\'' | '\\'))
        .collect::<String>()
        .trim_start()
        .to_ascii_lowercase();
    if let Some(one) = SSH_RUNS
        .iter()
        .find(|one| bare.starts_with(&one.to_ascii_lowercase()))
    {
        return Some(format!("-o {one}"));
    }
    // ProxyJump, and the host and user `%h` and `%r` paste into the command it becomes: named
    // exactly (`UserKnownHostsFile` is another option), its value with its quotes kept.
    let raw = value.trim_start();
    let (key, rest) = raw
        .split_once(|c: char| c == '=' || c.is_whitespace())
        .unwrap_or((raw, ""));
    let key: String = key
        .chars()
        .filter(|c| !matches!(c, '"' | '\'' | '\\'))
        .collect();
    PASTED
        .iter()
        .find(|one| one.eq_ignore_ascii_case(&key))
        .filter(|_| {
            shell_would_read(rest.trim_start_matches(|c: char| c == '=' || c.is_whitespace()))
        })
        .map(|one| format!("-o {one}"))
}

/// ssh's options whose value it pastes into a command its shell runs (`ProxyJump` becomes a
/// `ProxyCommand`, whose `%h` and `%r` are the host and user).
const PASTED: &[&str] = &["ProxyJump", "Hostname", "User"];

/// Whether `value` holds anything but what a host, a user, a port and a list of jumps are
/// spelled with, which a shell might read.
fn shell_would_read(value: &str) -> bool {
    !value.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '.' | '-' | '_' | '@' | ':' | ',' | '/' | '[' | ']')
    })
}

/// psql's `-c` or `--command` holding a backslash command: `\!` runs a shell, and `\copy …
/// program`, `\o |…`, `\g |…` and `\w |…` run a command.
fn psql_command(flag: &str, value: &str) -> Option<String> {
    let said = match flag {
        "c" => "-c",
        "command" => "--command",
        _ => return None,
    };
    value
        .contains('\\')
        .then(|| format!("{said} with a backslash command"))
}

/// mysql's `-e` or `--execute` holding a shell command: `\!` or `system`, `source` (a file of
/// commands, which may hold one), and any other backslash command (`\P` sets a pager it runs).
fn mysql_execute(flag: &str, value: &str) -> Option<String> {
    let said = match flag {
        "e" => "-e",
        "execute" => "--execute",
        _ => return None,
    };
    let lower = value.to_ascii_lowercase();
    let system = lower
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|word| matches!(word, "system" | "source" | "edit" | "pager"));
    (system || value.contains('\\')).then(|| format!("{said} with a shell command"))
}

fn none(_flag: &str, _value: &str) -> Option<String> {
    None
}

const PROGRAMS: &[Flags] = &[
    // ssh's own tools, before ssh's row, which their names' first part would reach.
    Flags {
        attached: "",
        names: &["ssh-keygen"],
        // ssh-keygen's getopt: "ABHKLQUXceghiklopquvyb:C:D:E:F:G:I:J:M:N:O:P:R:V:Y:Z:a:f:g:j:m:n:r:s:t:w:z:"
        valued: "bCDEFGIJMNOPRVYZafgjmnrstwz",
        // a PKCS#11 library, or a security key provider: code it loads
        refused: "Dw",
        in_a_cluster: &[],
        long: &[],
        value: none,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "",
        names: &["ssh-add"],
        // ssh-add's getopt: "vkKlLcdDTxXE:e:h:H:M:m:qs:S:t:"
        valued: "EehHMmsSt",
        // a PKCS#11 library, or a security key provider
        refused: "sS",
        in_a_cluster: &[],
        long: &[],
        value: none,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "",
        names: &["ssh", "slogin"],
        // OpenSSH's getopt string: "1246ab:c:e:fgi:kl:m:no:p:qstvxAB:CD:E:F:GI:J:KL:MNO:P:Q:R:S:TVw:W:XYy"
        valued: "BbcDEeFIiJLlmOoPpQRSWw",
        // A config file can name a command; a PKCS#11 library is code ssh loads.
        refused: "FI",
        in_a_cluster: &[],
        long: &[],
        value: ssh_option,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "",
        names: &["scp"],
        // scp's: "12346ABCTdfOpqRrstvD:F:J:P:S:c:i:l:o:X:" (`-O` is the legacy protocol, no value)
        valued: "cDFiJloPSX",
        // `-S` is the program it runs for ssh, `-D` the sftp server it runs, `-F` a config file.
        refused: "DFS",
        in_a_cluster: &[],
        long: &[],
        value: ssh_option,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "",
        names: &["sftp"],
        // sftp's: "1246AafhNpqrvCc:D:i:l:o:s:S:b:B:F:J:P:R:X:"
        valued: "BbcDFiJloPRSsX",
        // `-b` a batch file, whose `!` runs a shell, and as `scp`.
        refused: "bDFS",
        in_a_cluster: &[],
        long: &[],
        value: ssh_option,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "",
        names: &["rsync"],
        valued: "BfT@",
        // `-e` the remote shell it runs, `-M` options for the other end's rsync.
        refused: "eM",
        in_a_cluster: &[],
        long: &["rsh", "rsync-path", "remote-option", "daemon", "config"],
        value: none,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: true,
    },
    Flags {
        attached: "",
        names: &["zip"],
        valued: "",
        refused: "",
        in_a_cluster: &["TT"],
        long: &["unzip-command"],
        value: none,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "",
        names: &["wget", "wget2"],
        valued: "aABDeiIlnOoPQRtTUwX",
        // `-e` runs a line of wgetrc, `use_askpass` among them.
        refused: "e",
        in_a_cluster: &[],
        // wget2 (Fedora's `wget`) also loads plugins: a library by path, or from folders.
        long: &[
            "use-askpass",
            "execute",
            "config",
            "plugin",
            "local-plugin",
            "plugin-dirs",
        ],
        value: none,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "",
        names: &["kubectl"],
        valued: "",
        refused: "",
        in_a_cluster: &[],
        // A kubeconfig's `exec:` runs a credential plugin; kustomize's plugins and helm run.
        long: &[
            "kubeconfig",
            "enable-alpha-plugins",
            "enable-exec",
            "enable-helm",
            "helm-command",
        ],
        value: none,
        long_valued: &[],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "",
        names: &["psql"],
        valued: "cdfFhLopPRTUv",
        // `-f` a file of commands, `-o` an output that may be `|command`.
        refused: "fo",
        in_a_cluster: &[],
        long: &["file", "output"],
        value: psql_command,
        long_valued: &["command"],
        mysql_spelling: false,
        operands_reach_ssh: false,
    },
    Flags {
        attached: "p",
        names: &["mysql", "mariadb"],
        valued: "DehPSu",
        refused: "",
        in_a_cluster: &[],
        // A pager it runs, a file of settings that can name one, and plugins it loads.
        long: &[
            "pager",
            "defaults-file",
            "defaults-extra-file",
            "plugin-dir",
        ],
        value: mysql_execute,
        long_valued: &["execute"],
        mysql_spelling: true,
        operands_reach_ssh: false,
    },
];

/// **Why `name`'s run with `args` would run a command one of its flags names** (#5799), if it
/// would: the form, and that the person may leave it out or declare the program as a tool.
/// `name` is the program's file name, whatever its case.
pub fn names_a_command(name: &str, args: &[String]) -> Option<String> {
    // By every name its builds go by: `kubectl.docker`, `psql-16`, `wget2` (#5876's review).
    let forms = super::name_forms(name);
    let program = PROGRAMS
        .iter()
        .find(|one| forms.iter().any(|form| one.names.contains(&form.as_str())))?;
    let form = refused_form(program, args)?;
    Some(format!(
        "{name} {form} can run a command it names, so purlis does not offer to allow {name} with \
         it: leave {form} out, or declare what it's for as a tool, with the arguments it takes"
    ))
}

/// The first form of `args` that `program` refuses.
fn refused_form(program: &Flags, args: &[String]) -> Option<String> {
    let mut words = args.iter();
    while let Some(word) = words.next() {
        if word == "--" {
            // rsync hands an operand's host to ssh as a word of its own.
            let reaches =
                program.operands_reach_ssh && words.any(|operand| operand.starts_with('-'));
            return reaches.then(|| "an operand starting with -".to_owned());
        }
        if let Some(long) = word.strip_prefix("--") {
            let (named, inline) = match long.split_once('=') {
                Some((named, value)) => (named, Some(value)),
                None => (long, None),
            };
            if named.is_empty() {
                continue;
            }
            let spelled;
            let named = if program.mysql_spelling {
                let lower = named.to_ascii_lowercase().replace('_', "-");
                spelled = ["loose-", "enable-", "maximum-"]
                    .iter()
                    .find_map(|prefix| lower.strip_prefix(prefix).map(str::to_owned))
                    .unwrap_or(lower);
                spelled.as_str()
            } else {
                named
            };
            if let Some(one) = program.long.iter().find(|one| one.starts_with(named)) {
                return Some(format!("--{one}"));
            }
            if let Some(one) = program
                .long_valued
                .iter()
                .find(|one| one.starts_with(named))
            {
                let value = inline.map_or_else(|| words.next().cloned(), |v| Some(v.to_owned()));
                if let Some(said) = (program.value)(one, value.as_deref().unwrap_or_default()) {
                    return Some(said);
                }
            }
            continue;
        }
        let Some(cluster) = word.strip_prefix('-').filter(|rest| !rest.is_empty()) else {
            continue;
        };
        if let Some(one) = program
            .in_a_cluster
            .iter()
            .find(|one| cluster.contains(**one))
        {
            return Some(format!("-{one}"));
        }
        for (at, flag) in cluster.char_indices() {
            if program.refused.contains(flag) {
                return Some(format!("-{flag}"));
            }
            if program.attached.contains(flag) {
                break;
            }
            if program.valued.contains(flag) {
                let rest = &cluster[at + flag.len_utf8()..];
                let value = if rest.is_empty() {
                    words.next().cloned().unwrap_or_default()
                } else {
                    rest.to_owned()
                };
                if let Some(said) = (program.value)(&flag.to_string(), &value) {
                    return Some(said);
                }
                break;
            }
        }
    }
    None
}
