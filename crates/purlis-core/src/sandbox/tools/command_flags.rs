// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Flags that name a command** (#5799, ADR 0112): the forms of a program kept offered a Run
//! grant that would have it run a command, or read a file that can name one, which the person
//! reading the exact command would not see run.
//!
//! `ssh`, `scp`, `sftp`, `rsync`, `kubectl`, `psql`, `zip` and `wget` are the usual reason a chat
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
];

/// `-o Key=Value` or `-o "Key Value"`, where `Key` is one of [`SSH_RUNS`], whatever its case.
fn ssh_option(flag: &str, value: &str) -> Option<String> {
    if flag != "o" {
        return None;
    }
    let key = value
        .trim_start()
        .split(|c: char| c == '=' || c.is_whitespace())
        .next()
        .unwrap_or_default();
    SSH_RUNS
        .iter()
        .find(|one| one.eq_ignore_ascii_case(key))
        .map(|one| format!("-o {one}"))
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

fn none(_flag: &str, _value: &str) -> Option<String> {
    None
}

const PROGRAMS: &[Flags] = &[
    Flags {
        names: &["ssh", "slogin"],
        valued: "BbcDEeFIiJLlmOoPpQRSWw",
        // A config file can name a command; a PKCS#11 library is code ssh loads.
        refused: "FI",
        in_a_cluster: &[],
        long: &[],
        value: ssh_option,
        long_valued: &[],
    },
    Flags {
        names: &["scp"],
        valued: "cDFiJlOoPSX",
        // `-S` is the program it runs for ssh, `-D` the sftp server it runs, `-F` a config file.
        refused: "DFS",
        in_a_cluster: &[],
        long: &[],
        value: ssh_option,
        long_valued: &[],
    },
    Flags {
        names: &["sftp"],
        valued: "BbcDFiJlOoPRSsX",
        // `-b` a batch file, whose `!` runs a shell, and as `scp`.
        refused: "bDFS",
        in_a_cluster: &[],
        long: &[],
        value: ssh_option,
        long_valued: &[],
    },
    Flags {
        names: &["rsync"],
        valued: "BfT@",
        // `-e` the remote shell it runs, `-M` options for the other end's rsync.
        refused: "eM",
        in_a_cluster: &[],
        long: &["rsh", "rsync-path", "remote-option", "daemon", "config"],
        value: none,
        long_valued: &[],
    },
    Flags {
        names: &["zip"],
        valued: "",
        refused: "",
        in_a_cluster: &["TT"],
        long: &["unzip-command"],
        value: none,
        long_valued: &[],
    },
    Flags {
        names: &["wget"],
        valued: "aABDeiIlnOoPQRtTUwX",
        // `-e` runs a line of wgetrc, `use_askpass` among them.
        refused: "e",
        in_a_cluster: &[],
        long: &["use-askpass", "execute", "config"],
        value: none,
        long_valued: &[],
    },
    Flags {
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
    },
    Flags {
        names: &["psql"],
        valued: "cdfFhLopPRTUv",
        // `-f` a file of commands, `-o` an output that may be `|command`.
        refused: "fo",
        in_a_cluster: &[],
        long: &["file", "output"],
        value: psql_command,
        long_valued: &["command"],
    },
];

/// **Why `name`'s run with `args` would run a command one of its flags names** (#5799), if it
/// would: the form, and that the person may leave it out or declare the program as a tool.
/// `name` is the program's file name, whatever its case.
pub fn names_a_command(name: &str, args: &[String]) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let program = PROGRAMS
        .iter()
        .find(|one| one.names.contains(&lower.as_str()))?;
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
            return None;
        }
        if let Some(long) = word.strip_prefix("--") {
            let (named, inline) = match long.split_once('=') {
                Some((named, value)) => (named, Some(value)),
                None => (long, None),
            };
            if named.is_empty() {
                continue;
            }
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
