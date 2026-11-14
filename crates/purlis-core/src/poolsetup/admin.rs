// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Send to your admin, and waiting** (#4496, spec #4469 stories 44 to 46): where doctor shows
//! the person can't install, the words to send their cluster's admin, and the step waiting for
//! them, checked again by itself.
//!
//! - **What is sent is made only of what is public**: doctor's own report (what it asked and
//!   found, in its own words), this build's version, the guide's address, and the commands the
//!   admin runs with their own sign-in, pointed at no context of the person's. No sign-in, token,
//!   key or credential is ever read to make it, and any line that still looks like one is left
//!   out ([`held_free_of_secrets`]), so a cluster's error words can't carry one along.
//! - **Waiting is a choice the person made** ([`Asked`], kept in `pool-setup.json`): the context
//!   they asked about and when. Whether the admin has done it is read fresh each time, by doctor.
//!   A background recheck ([`recheck`]) marks it ready once, which is what raises its one
//!   needs-you entry; answering that entry, or choosing again, forgets it.

use super::{Choices, Facts, Kind, read_choices, write_choices};
use crate::runners::Refused;

/// The guide a team's admin installs purlis on a cluster by.
pub const GUIDE: &str = "https://github.com/purlis/purlis/blob/main/docs/cluster-pool-setup.md";

/// What a line that looks like a secret reads as, in what the person shares.
pub const LEFT_OUT: &str = "[a line left out: it looked like a secret]";

/// **That the person asked their admin** (#4496): about the cluster behind context `context`,
/// at `since`, from the project `project` where the tab was in one, and, once the background
/// recheck found the cluster ready, when.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Asked {
    pub context: String,
    pub since: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<std::path::PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready: Option<u64>,
}

/// **`text`, with every line that looks like a secret left out** (#4496, #4497): what a person
/// shares with someone else, or a chat is started with, goes through this last.
///
/// Read with the scanner a save's guard reads with ([`crate::secretshape::leak_spans_whole_and_by_line`]),
/// whole and a line at a time, so a secret that runs over several lines leaves out every line it
/// touches; and each line also through every escaped and encoded reading of it
/// ([`crate::secretshape::token_kind_as_read`]), so a token spelled in JSON's escapes or a URL's
/// percent-encoding is left out as the token it is.
#[must_use]
pub fn held_free_of_secrets(text: &str) -> String {
    let spans = crate::secretshape::leak_spans_whole_and_by_line(text);
    let mut at = 0;
    let mut kept = Vec::new();
    for line in text.split_inclusive('\n') {
        let here = at..at + line.len();
        at += line.len();
        let bare = line.strip_suffix('\n').unwrap_or(line);
        let bare = bare.strip_suffix('\r').unwrap_or(bare);
        let touched = spans
            .iter()
            .any(|(span, _)| span.start < here.end && here.start < span.end);
        kept.push(
            if touched
                || crate::secretshape::secret_kind(bare).is_some()
                || crate::secretshape::token_kind_as_read(bare).is_some()
                || a_cluster_credential(bare)
            {
                LEFT_OUT
            } else {
                bare
            },
        );
    }
    kept.join("\n")
}

/// **Whether `line` carries a credential a cluster's sign-in is made of**, in a shape the shared
/// scanner doesn't read: a bearer header (a GKE `ya29.` token among them), a kubeconfig's
/// key or token data, a URL with a password in it, a PEM spelled in base64, or a token behind
/// JSON's `\u` escapes. kubectl and an API server echo these, and what Pool setup shares is made
/// of what they said.
fn a_cluster_credential(line: &str) -> bool {
    static SHAPES: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let shapes = SHAPES.get_or_init(|| {
        regex::Regex::new(concat!(
            r"(?i)\bbearer\s+[^\s]{8,}",
            r"|(?i)\b(?:client-key-data|client-certificate-data|token-data|id-token|refresh-token|access-token|id_token|refresh_token|access_token)\b",
            r"|\bya29\.[A-Za-z0-9_-]{8,}",
            r"|\bk8s-aws-v1\.[A-Za-z0-9_-]{8,}",
            r"|[A-Za-z][A-Za-z0-9+.-]*://[^/\s:@]+:[^/\s@]+@",
            r"|LS0tLS1CRUdJTi",
        ))
        .expect("a pattern this module wrote")
    });
    if shapes.is_match(line) {
        return true;
    }
    // JSON's `\uXXXX` escapes, read as the characters they spell, then asked again.
    let unescaped = json_unescaped(line);
    unescaped != line
        && (crate::secretshape::token_kind_as_read(&unescaped).is_some()
            || !crate::secretshape::leak_spans(&unescaped).is_empty()
            || shapes.is_match(&unescaped))
}

/// `line` with each JSON `\uXXXX` escape read as its character; an escape that spells none is kept.
fn json_unescaped(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find("\\u") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        match after
            .get(..4)
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .and_then(char::from_u32)
        {
            Some(c) => {
                out.push(c);
                rest = &after[4..];
            }
            None => {
                out.push_str("\\u");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// **Whether doctor shows the person can't install** (story 44): it ran, and the cluster lacks
/// something the install's own parts don't add, such as a right only an admin holds.
#[must_use]
pub fn cannot_install(facts: &Facts) -> bool {
    !facts.cluster.operator_runs()
        && !facts.cluster.joining
        && facts
            .cluster
            .doctor
            .as_ref()
            .is_some_and(purlis_cluster_doctor::Report::lacks_more_than_the_install)
}

/// **The words for the admin** (#4496), where doctor shows the person can't install: what doctor
/// found, what purlis needs, the guide, and the commands, a check and the install, as the admin
/// runs them on their own context. Held free of secrets.
#[must_use]
pub fn text(facts: &Facts, choices: &Choices) -> Option<String> {
    if !cannot_install(facts) {
        return None;
    }
    let report = facts.cluster.doctor.as_ref()?;
    let version = env!("CARGO_PKG_VERSION");
    let words = format!(
        "Hello. I'd like our Kubernetes cluster to run my purlis chats, and purlis's check \
         found it lacks what I can't add with my own sign-in. Could you have a look?\n\
         \n\
         What the check found, in its own words:\n\
         {report}\n\
         \n\
         What purlis needs: its cluster operator, installed as this guide installs it, and the \
         rights its \"Let a teammate use the pool\" section grants, for me:\n\
         {GUIDE}\n\
         \n\
         The check only reads. Run it with your own sign-in:\n\
         {check}\n\
         \n\
         The install, a part at a time, from a checkout of purlis {version}:\n\
         {install}\n\
         \n\
         Once it's there, send me the pool's name and its key (the guide's step 6 prints it). \
         My Pool setup notices by itself.\n\
         \n\
         Sent from purlis {version}. It holds no password, token, key or credential.",
        check = super::cluster::doctor_by_hand(""),
        install = super::install::all_by_hand(choices.operator_image.as_deref(), ""),
    );
    Some(held_free_of_secrets(&words))
}

/// **Whether the setup on context `context` waits for the admin**: the person asked about it.
#[must_use]
pub fn waiting_on(choices: &Choices, context: Option<&str>) -> bool {
    choices
        .admin
        .as_ref()
        .is_some_and(|asked| Some(asked.context.as_str()) == context)
}

/// **Keeps that the person sent the words to their admin** (#4496), about the context chosen on
/// Where, at `now`, from `project`. Refused where no cluster was chosen.
pub fn asked(
    config: &std::path::Path,
    project: Option<&std::path::Path>,
    now: u64,
) -> Result<(), Refused> {
    let _locked = crate::runners::lock(config);
    let mut choices = read_choices(config)?;
    let (Some(Kind::Cluster), Some(context)) = (choices.kind, choices.context.clone()) else {
        return Err(Refused::Other(
            "choose your team's cluster on Where first".to_owned(),
        ));
    };
    choices.admin = Some(Asked {
        context,
        since: now,
        project: project.map(std::path::Path::to_path_buf),
        ready: None,
    });
    write_choices(config, choices)
}

/// **Forgets that the person waits for their admin** (#4496): they answered its entry, stopped
/// waiting, or the setup went on.
pub fn forget(config: &std::path::Path) -> Result<(), Refused> {
    let _locked = crate::runners::lock(config);
    let mut choices = read_choices(config)?;
    if choices.admin.is_none() {
        return Ok(());
    }
    choices.admin = None;
    write_choices(config, choices)
}

/// **The one background recheck** (#4496, story 45): where the person waits for their admin and
/// it isn't ready yet, asks the cluster as Check the cluster does, and once doctor no longer
/// shows they can't install (or the operator runs, or they join their team's install), marks it
/// ready at `now`. Whether it became ready on this call, which is what raises its entry, once.
pub fn recheck(
    config: &std::path::Path,
    kubectl: Result<&crate::clusterpool::Kubectl, String>,
    now: u64,
) -> Result<bool, Refused> {
    let choices = read_choices(config)?;
    let Some(asked) = choices.admin.as_ref().filter(|asked| asked.ready.is_none()) else {
        return Ok(false);
    };
    let cluster = super::cluster::read(kubectl, Some(&asked.context), true);
    // Ready is only what was read: the operator runs, the person joins their team's install, or
    // doctor finds nothing the install can't add. A context gone or a cluster that didn't answer
    // says nothing either way.
    let goes_on = cluster.operator_runs()
        || cluster.joining
        || cluster
            .doctor
            .as_ref()
            .is_some_and(|report| !report.lacks_more_than_the_install());
    if !goes_on {
        return Ok(false);
    }
    let _locked = crate::runners::lock(config);
    let mut choices = read_choices(config)?;
    match choices.admin.as_mut() {
        Some(kept) if kept.context == asked.context && kept.ready.is_none() => {
            kept.ready = Some(now);
        }
        _ => return Ok(false),
    }
    write_choices(config, choices)?;
    Ok(true)
}

/// **The entry the person is told by** (#4496): the context that is ready, and the project it was
/// asked from, once the recheck marked it ready.
#[must_use]
pub fn ready(choices: &Choices) -> Option<&Asked> {
    choices.admin.as_ref().filter(|asked| asked.ready.is_some())
}

#[cfg(test)]
mod tests;
