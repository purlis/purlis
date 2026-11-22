// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's `[policy]` in the window** (rulings Q308, Q308a, Q308b; ADR 0125): what waits
//! for the person's press, what the file says that purlis could not read, and the press itself.
//!
//! A tightening of `[policy]` applies at once; **a loosening holds only once the person presses
//! Accept** here ([`accept_project_policy`], the window's alone), which the app seals on this
//! machine ([`purlis_core::enforcement::press`]). A chat can never accept it.

use purlis_core::enforcement;

use crate::planes::{PlaneId, Planes};

/// One family the project now runs looser than you accepted, or one dispatch deny it lifted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyWaiting {
    /// The family's key, as `[policy]` spells it.
    pub family: String,
    /// What you accepted: `off`, `log` or `enforce`; the sandbox's level `off`, `floor` or `full`.
    pub accepted: String,
    /// What the project's file says now.
    pub committed: String,
}

/// One rule the press changes, by its `@id`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PressRule {
    /// Its `@id`.
    pub id: String,
    /// `loosens`, `tightens` or `changes`.
    pub kind: String,
    /// What happened to it: `added`, `removed`, `edited`, `now permits`, `now forbids`, `now
    /// asks` or `no longer asks`.
    pub what: String,
}

/// One mode the press changes: a family's, or a rule's (`rules.<@id>`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PressMode {
    /// The family's key, or `rules.<@id>`.
    pub key: String,
    /// The mode in force before: `off`, `log` or `enforce`.
    pub before: String,
    /// The mode after.
    pub after: String,
    /// `loosens` or `tightens`.
    pub kind: String,
}

/// One `[policy.limits]` number the press changes, said as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PressLimit {
    /// The key, `<family>.<kebab>`.
    pub key: String,
    /// The number before, or `no cap`.
    pub before: String,
    /// The number after, or `no cap`.
    pub after: String,
    /// `loosens` or `tightens`.
    pub kind: String,
}

/// One tightening of your own file that the press lets go.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PressLocal {
    /// What it sets: `template`, a family's key, or `rules.<@id>`.
    pub key: String,
    /// What your sealed tightening holds.
    pub was: String,
    /// What runs once you accept.
    pub now: String,
}

/// The commit a press accepts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PressCommit {
    /// Its full sha.
    pub sha: String,
    /// Its subject line.
    pub subject: String,
    /// The author the commit names itself, as written. Purlis cannot check who wrote it.
    pub author: String,
}

/// **The loosening press** (M75-15): what Accept would accept, against the policy in force.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyPress {
    /// Why it is raised: `loosening`, `past-the-cap`, `local-removal` or `lost-store`.
    pub reason: String,
    /// Each rule that differs.
    pub rules: Vec<PressRule>,
    /// Each mode that differs.
    pub modes: Vec<PressMode>,
    /// Each limit that differs.
    pub limits: Vec<PressLimit>,
    /// Each tightening of your own file the press lets go: listed in every press.
    pub local: Vec<PressLocal>,
    /// Each other part that differs and no typed row covers, said by name: they wait like any
    /// change.
    pub other: Vec<String>,
    /// The commit, none for the removal of a tightening from your own file.
    pub commit: Option<PressCommit>,
    /// After a lost store, what was lost, in one sentence.
    pub lost: Option<String>,
}

fn number_said(number: Option<i64>) -> String {
    number.map_or_else(|| "no cap".to_owned(), |n| n.to_string())
}

fn press_of(root: &std::path::Path) -> Option<(PolicyPress, String)> {
    use purlis_core::policy::press::{self, Reason};
    let held = press::of(root)?;
    let shown = PolicyPress {
        reason: match held.reason {
            Reason::Loosening => "loosening",
            Reason::PastTheCap => "past-the-cap",
            Reason::LocalRemoval => "local-removal",
            Reason::LostStore => "lost-store",
        }
        .to_owned(),
        rules: held
            .diff
            .rules
            .iter()
            .map(|one| PressRule {
                id: one.id.clone(),
                kind: one.kind.word().to_owned(),
                what: one.what.to_owned(),
            })
            .collect(),
        modes: held
            .diff
            .modes
            .iter()
            .map(|one| PressMode {
                key: one.key.clone(),
                before: one.before.to_owned(),
                after: one.after.to_owned(),
                kind: one.kind.word().to_owned(),
            })
            .collect(),
        limits: held
            .diff
            .limits
            .iter()
            .map(|one| PressLimit {
                key: one.key.clone(),
                before: number_said(one.before),
                after: number_said(one.after),
                kind: one.kind.word().to_owned(),
            })
            .collect(),
        local: held
            .diff
            .local
            .iter()
            .map(|one| PressLocal {
                key: one.key.clone(),
                was: one.was.clone(),
                now: one.now.clone(),
            })
            .collect(),
        other: held.diff.other.iter().map(|one| one.part.clone()).collect(),
        commit: held.commit.map(|one| PressCommit {
            sha: one.sha,
            subject: one.subject,
            author: one.author,
        }),
        lost: held.lost.map(|lost| lost.said()),
    };
    Some((shown, held.digest))
}

/// What the window says about the project's `[policy]` on this machine.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyState {
    /// Each family that waits for your press to loosen.
    pub waiting: Vec<PolicyWaiting>,
    /// Each value of `[policy]` purlis could not read, in one sentence, enforced meanwhile.
    pub refused: Vec<String>,
    /// The families that mean nothing without a sandbox (`network`, `ports`, `processes`),
    /// where the project's level is no sandbox: said beside the press that would make it so.
    pub not_in_force: Vec<String>,
    /// Whether this machine keeps an acceptance whose seal does not hold, which counts as none.
    pub unsealed: bool,
    /// The digest of the committed text this was read from: what Accept names, so a press on a
    /// file that changed since it was shown accepts nothing.
    pub digest: String,
    /// The template the project's committed file names (`open`, `watch`, `hold`, `team` or
    /// `strict`), or `watch` where it names none (Q308a: the window says it, Q308 instruction 45).
    pub template: String,
    /// Whether the file named no template, so `watch`, the shipped default, holds.
    pub template_default: bool,
    /// Each family the project sets apart from its template, in the docs' order.
    pub overrides: Vec<PolicyOverride>,
    /// Each family an administrator's lock holds enforced on this machine, whatever the file
    /// says (Q308a: Settings shows it as locked).
    pub locked: Vec<String>,
    /// The loosening press (M75-15): the diff against the policy in force, the commit and its
    /// author, raised while a loosening, a commit past the accepted floor's cap, the removal of a
    /// tightening from your own file or a lost store waits for Accept.
    pub press: Option<PolicyPress>,
}

/// One family the project's `[policy]` sets apart from its template.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyOverride {
    /// The family's key, as `[policy]` spells it.
    pub family: String,
    /// What the file says: `off`, `log` or `enforce`.
    pub said: String,
    /// How your chats here run it now: stricter than `said` where a lock holds it, where the
    /// family is not switchable yet, or where a loosening waits for your press.
    pub runs: String,
}

/// The project's `[policy]` as the window says it: one [`enforcement::standing`], so the template,
/// the families set apart, what waits and the digest Accept names are all of one read of the
/// file. Only reads: the core alone resolves the policy, and nothing here keeps anything.
fn state_of(root: &std::path::Path) -> PolicyState {
    let standing = enforcement::standing(root);
    let overrides = enforcement::Family::ALL
        .into_iter()
        .filter_map(|family| {
            let said = standing.said.overrides.get(&family)?;
            Some(PolicyOverride {
                family: family.key().to_owned(),
                said: said.word().to_owned(),
                runs: standing.runs.mode(family).word().to_owned(),
            })
        })
        .collect();
    let (press, press_digest) = press_of(root).unzip();
    PolicyState {
        press,
        waiting: standing
            .waiting
            .into_iter()
            .map(|(family, accepted, committed)| PolicyWaiting {
                family: family.key().to_owned(),
                accepted: accepted.word().to_owned(),
                committed: committed.word().to_owned(),
            })
            // The sandbox waits by its level (the follow-up's item 1): `off`, `floor` or `full`.
            .chain(standing.level.map(|(accepted, committed)| PolicyWaiting {
                family: enforcement::Family::Sandbox.key().to_owned(),
                accepted: accepted.word().to_owned(),
                committed: committed.word().to_owned(),
            }))
            // A deny you accepted that the project's file took away waits for the same press
            // (the review of #7113): `dispatch.denies steward -> devops from denied to lifted`.
            .chain(standing.lifted.into_iter().map(|deny| PolicyWaiting {
                family: format!("{} {deny}", purlis_core::dispatchgrant::DENY_RULE),
                accepted: "denied".to_owned(),
                committed: "lifted".to_owned(),
            }))
            .collect(),
        refused: standing.refused,
        not_in_force: standing
            .not_in_force
            .into_iter()
            .map(|family| family.key().to_owned())
            .collect(),
        unsealed: standing.unsealed,
        // What Accept names is both reads: the old press's digest, and the new press's.
        digest: format!("{}+{}", standing.digest, press_digest.unwrap_or_default()),
        template: standing.said.template.word().to_owned(),
        template_default: !standing.said.template_said,
        overrides,
        locked: standing
            .locked
            .into_iter()
            .map(|family| family.key().to_owned())
            .collect(),
    }
}

/// One line of the project's enforcement log: a refusal a family set to `log` let through.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyLogLine {
    /// When, UTC, RFC 3339.
    pub at: String,
    /// The id of the rule that would have refused.
    pub rule: String,
    /// The family's key.
    pub family: String,
    /// The number of the chat it was in, while that chat is open here; `null` for one that has
    /// closed, or that purlis could not name: the window says it is unknown.
    pub session: Option<u32>,
    /// The host it was about, where the rule is about one.
    pub host: Option<String>,
}

/// The most lines the window is handed, newest first: the count says them all.
const MOST_LINES: usize = 500;

/// What the project's enforcement log holds, newest first, at most [`MOST_LINES`], with how
/// many lines it holds in all.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyLog {
    /// How many refusals were let through and logged, every chat's.
    pub count: u32,
    /// The newest of them.
    pub lines: Vec<PolicyLogLine>,
    /// How many lines the log holds that are not as purlis writes them, left out of the count
    /// and the lines: what a process other than the app wrote there.
    pub dropped: u32,
}

/// Every chat's enforcement log at `root`, newest first, and how many lines were dropped.
fn logged_at(root: &std::path::Path) -> (Vec<enforcement::Logged>, usize) {
    let Ok(files) = std::fs::read_dir(enforcement::log_dir(root)) else {
        return (Vec::new(), 0);
    };
    let mut lines = Vec::new();
    let mut dropped = 0;
    for chat in files.filter_map(Result::ok).filter_map(|file| {
        let name = file.file_name().into_string().ok()?;
        name.strip_suffix(".jsonl").map(str::to_owned)
    }) {
        let Some(path) = enforcement::log_file(root, &chat) else {
            continue;
        };
        let text = std::fs::read_to_string(path).unwrap_or_default();
        // A chat's log is appended to, so its own lines are newest last: reversed, they keep
        // their order through the sort below wherever two share a second.
        for line in text.lines().rev() {
            match serde_json::from_str::<enforcement::Logged>(line) {
                Ok(logged) if as_purlis_writes(&logged, &chat) => lines.push(logged),
                _ => dropped += 1,
            }
        }
    }
    // RFC 3339 at one precision, UTC (checked above): the text sorts as the time does. Stable,
    // so lines of one second keep the order above.
    lines.sort_by(|one, next| next.at.cmp(&one.at));
    (lines, dropped)
}

/// **Whether `line`, read from chat `chat`'s log, is one the app could have written**: the same
/// checks as a line a chat's process hands over (`handoff.rs`'s `policy_line`), so a process
/// that writes the file itself puts nothing on the page but a known rule, a family, a
/// `host:port` and a time.
fn as_purlis_writes(line: &enforcement::Logged, chat: &str) -> bool {
    let Some(family) = enforcement::Family::of_key(&line.family) else {
        return false;
    };
    let short = |text: &str| text.chars().count() <= enforcement::MOST_CHARS;
    line.chat == chat
        && short(&line.rule)
        && enforcement::known_rule(family, &line.rule)
        && line
            .host
            .as_deref()
            .is_none_or(|host| short(host) && enforcement::is_host_and_port(host))
        && jiff::civil::DateTime::strptime("%Y-%m-%dT%H:%M:%SZ", &line.at).is_ok()
}

/// The log as the window is handed it: the count of every line, the newest [`MOST_LINES`], each
/// with its chat's number where `session_of` names one open here.
fn log_of(
    (logged, dropped): (Vec<enforcement::Logged>, usize),
    session_of: impl Fn(&str) -> Option<u32>,
) -> PolicyLog {
    let count = u32::try_from(logged.len()).unwrap_or(u32::MAX);
    let lines = logged
        .into_iter()
        .take(MOST_LINES)
        .map(|line| PolicyLogLine {
            session: session_of(&line.chat),
            at: line.at,
            rule: line.rule,
            family: line.family,
            host: line.host,
        })
        .collect();
    PolicyLog {
        count,
        lines,
        dropped: u32::try_from(dropped).unwrap_or(u32::MAX),
    }
}

/// **What the project's `[policy]` let through and logged** (Q308a: the "would have blocked"
/// count, which opens the log): every chat's lines, newest first. A read, off the window's
/// thread.
#[tauri::command]
#[specta::specta]
pub async fn policy_log(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PolicyLog, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let logged = crate::off_the_window(READING_POLICY, move || Ok(logged_at(&root))).await?;
    Ok(log_of(logged, |chat| held.chats().session_of_id(chat)))
}

/// What the commands here name their work, off the window's thread.
const READING_POLICY: &str = "reading the project's policy";

/// **The project's `[policy]` as it stands for you**: what waits for your press, and what the
/// file says that purlis could not read. Off the window's thread: it reads the project's files,
/// the administrator's policy and this machine's acceptance.
#[tauri::command]
#[specta::specta]
pub async fn project_policy_state(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PolicyState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(READING_POLICY, move || Ok(state_of(&root))).await
}

/// The person's press, made here: the window's command layer is its one maker.
#[allow(
    clippy::disallowed_methods,
    reason = "the window's command layer is the one place that makes the person's press"
)]
fn the_persons_press() -> purlis_core::secrets::answers::Press {
    purlis_core::secrets::answers::Press::of_the_persons_press()
}

/// **Your press: accept the project's `[policy]` as its committed file said it when it was
/// shown** (`shown`, the state's digest), looser where it is looser, at once. Sealed on this
/// machine by the app; refused where it cannot be sealed, or where the file changed since it was
/// shown. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn accept_project_policy(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: String,
) -> Result<PolicyState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(READING_POLICY, move || {
        let kept = |why: std::io::Error| {
            format!("purlis could not keep your acceptance on this machine: {why}")
        };
        // The digest is both reads (`state_of`): the settings' and the press's.
        let (settings, shown_press) = shown.split_once('+').unwrap_or((&shown, ""));
        // Both are checked before either is kept, so a press on a project that changed since it
        // was shown accepts neither.
        if enforcement::digest_of(&root) != settings {
            return Err(kept(std::io::Error::other(
                "the project's policy changed since it was shown, so nothing was accepted; look \
                 at it again",
            )));
        }
        let press_kept = !shown_press.is_empty();
        if press_kept {
            purlis_core::policy::press::accept(&root, &the_persons_press(), shown_press)
                .map_err(kept)?;
        }
        // The second half can fail after the first was kept: said as it is, never as nothing kept.
        enforcement::press(&root, settings).map_err(|why| {
            if press_kept {
                format!(
                    "purlis kept your acceptance of the project's rules and limits, but could not \
                     keep the rest of it on this machine: {why}. Accept again to finish."
                )
            } else {
                kept(why)
            }
        })?;
        Ok(state_of(&root))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plane whose committed settings file says `text`.
    fn plane(text: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a folder");
        std::fs::write(dir.path().join("charter.toml"), text).expect("the settings file");
        dir
    }

    #[test]
    fn the_window_is_told_the_template_and_each_override_with_how_it_runs() {
        let dir =
            plane("[policy]\ntemplate = \"team\"\nguards.release = \"log\"\nskills = \"off\"\n");
        let state = state_of(dir.path());
        assert_eq!(state.template, "team");
        assert!(!state.template_default);
        assert_eq!(
            state.overrides,
            [
                PolicyOverride {
                    family: "guards.release".into(),
                    said: "log".into(),
                    runs: "log".into(),
                },
                // Not switchable yet: the file says off, and it runs enforced.
                PolicyOverride {
                    family: "skills".into(),
                    said: "off".into(),
                    runs: "enforce".into(),
                },
            ]
        );
    }

    #[test]
    fn a_project_that_names_no_template_is_said_to_run_the_default_watch() {
        let dir = plane("[plane]\nmode = \"commit\"\n");
        let state = state_of(dir.path());
        assert_eq!(state.template, "watch");
        assert!(state.template_default);
        assert!(state.overrides.is_empty());
    }

    const ONE: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";
    const TWO: &str = "01JABCDEFGHJKMNPQRSTVWXYZ1";

    /// A log line as the app writes it, of `family`'s `rule`, in chat `chat`.
    fn line(at: &str, family: &str, rule: &str, chat: &str, host: Option<&str>) -> String {
        let host = host.map_or(String::new(), |host| format!(",\"host\":\"{host}\""));
        format!(
            "{{\"at\":\"{at}\",\"rule\":\"{rule}\",\"family\":\"{family}\",\"chat\":\"{chat}\"{host}}}\n"
        )
    }

    /// A plane whose enforcement log holds `logs`, each a chat's id and its file's text.
    fn logged(logs: &[(&str, String)]) -> tempfile::TempDir {
        let dir = plane("");
        let folder = enforcement::log_dir(dir.path());
        std::fs::create_dir_all(&folder).expect("the log folder");
        for (chat, text) in logs {
            std::fs::write(folder.join(format!("{chat}.jsonl")), text).expect("a chat's log");
        }
        dir
    }

    fn times(lines: &[enforcement::Logged]) -> Vec<(&str, Option<&str>)> {
        lines
            .iter()
            .map(|line| (line.at.as_str(), line.host.as_deref()))
            .collect()
    }

    #[test]
    fn the_log_is_every_chats_lines_newest_first() {
        let merge = enforcement::RULE_MERGE_HOLD;
        let dir = logged(&[
            (
                ONE,
                line("2026-11-17T09:00:00Z", "guards.merge", merge, ONE, None)
                    + &line("2026-11-17T11:00:00Z", "guards.merge", merge, ONE, None),
            ),
            (
                TWO,
                line("2026-11-17T10:00:00Z", "guards.merge", merge, TWO, None),
            ),
        ]);
        std::fs::write(
            enforcement::log_dir(dir.path()).join("notes.txt"),
            "not a log",
        )
        .expect("another file");
        let (lines, dropped) = logged_at(dir.path());
        assert_eq!(
            times(&lines),
            [
                ("2026-11-17T11:00:00Z", None),
                ("2026-11-17T10:00:00Z", None),
                ("2026-11-17T09:00:00Z", None),
            ]
        );
        assert_eq!(dropped, 0);
    }

    #[test]
    fn of_one_second_the_line_written_last_is_first() {
        let ask = enforcement::RULE_HOST_ASK;
        let at = "2026-11-17T09:00:00Z";
        let dir = logged(&[(
            ONE,
            line(at, "asks", ask, ONE, Some("a.example:443"))
                + &line(at, "asks", ask, ONE, Some("b.example:443"))
                + &line(at, "asks", ask, ONE, Some("c.example:443")),
        )]);
        let (lines, _) = logged_at(dir.path());
        assert_eq!(
            times(&lines),
            [
                (at, Some("c.example:443")),
                (at, Some("b.example:443")),
                (at, Some("a.example:443")),
            ]
        );
    }

    #[test]
    fn a_line_the_app_would_not_write_is_left_out_and_counted() {
        // A chat's process cannot write the log, but a line written there by any other means
        // puts nothing on the page that a handed-over line could not (the review of #7093).
        let at = "2026-11-17T09:00:00Z";
        let ask = enforcement::RULE_HOST_ASK;
        let long = "a".repeat(enforcement::MOST_CHARS + 1);
        let bad = [
            "not a line\n".to_owned(),
            line(at, "guards.git", "rm -rf /home/someone", ONE, None),
            line(at, "guards.save", enforcement::RULE_MERGE_HOLD, ONE, None),
            line(at, "nothing", ask, ONE, None),
            line(at, "asks", ask, ONE, Some("https://x/a?b")),
            line(at, "asks", ask, ONE, Some("example.com")),
            line(at, "asks", ask, ONE, Some(&format!("{long}:443"))),
            line(at, "asks", ask, TWO, None),
            line("yesterday, about noon", "asks", ask, ONE, None),
            line("2026-11-17T09:00:00+04:00", "asks", ask, ONE, None),
        ];
        let kept = line(at, "asks", ask, ONE, Some("example.com:443"));
        let dir = logged(&[(ONE, bad.concat() + &kept)]);
        let (lines, dropped) = logged_at(dir.path());
        assert_eq!(times(&lines), [(at, Some("example.com:443"))]);
        assert_eq!(dropped, bad.len());
    }

    #[test]
    fn the_count_says_every_line_and_the_window_is_handed_the_newest_500() {
        let lines: Vec<enforcement::Logged> = (0..=MOST_LINES)
            .map(|n| enforcement::Logged {
                at: format!("2026-11-17T09:{:02}:{:02}Z", n / 60, n % 60),
                rule: enforcement::RULE_HOST_ASK.to_owned(),
                family: "asks".to_owned(),
                chat: ONE.to_owned(),
                host: None,
            })
            .rev()
            .collect();
        let log = log_of((lines, 3), |_| None);
        assert_eq!(log.count, 501);
        assert_eq!(log.lines.len(), 500);
        assert_eq!(log.lines[0].at, "2026-11-17T09:08:20Z");
        assert_eq!(log.dropped, 3);
    }

    #[test]
    fn a_line_names_its_chat_by_number_only_while_it_is_open() {
        let of = |chat: &str| enforcement::Logged {
            at: "2026-11-17T09:00:00Z".to_owned(),
            rule: enforcement::RULE_HOST_ASK.to_owned(),
            family: "asks".to_owned(),
            chat: chat.to_owned(),
            host: None,
        };
        let log = log_of((vec![of(ONE), of(TWO)], 0), |chat| {
            (chat == ONE).then_some(12)
        });
        let sessions: Vec<Option<u32>> = log.lines.iter().map(|line| line.session).collect();
        assert_eq!(sessions, [Some(12), None]);
    }

    #[test]
    fn a_project_with_no_log_has_logged_nothing() {
        let dir = plane("");
        assert_eq!(logged_at(dir.path()), (Vec::new(), 0));
    }
}
