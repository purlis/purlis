// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's `[policy]` in the window** (rulings Q308, Q308a, Q308b; ADR 0125): what waits
//! for the person's press, what the file says that purlis could not read, and the press itself.
//!
//! A tightening of `[policy]` applies at once; **a loosening holds only once the person presses
//! Accept** here ([`accept_project_policy`], the window's alone), which the app seals on this
//! machine ([`purlis_core::enforcement::press`]). A chat can never accept it.

use purlis_core::enforcement;
use tauri::Manager;

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

/// One pair said never to: the asking persona, and the persona it never dispatches to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct NeverPair {
    /// The persona that asks.
    pub asking: String,
    /// The persona it never dispatches to.
    pub target: String,
}

/// **The never-dispatch list's pairs the window offers to keep after a new seal key** (#7714,
/// H1 of #7734's round 3): what Keep these nevers ([`keep_project_nevers`]) seals, exactly.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct NeversToKeep {
    /// The pairs the list's file holds now, in its order: none where it holds none, or is gone.
    pub pairs: Vec<NeverPair>,
    /// Their digest: what the press names, so a list that changed since it was shown keeps
    /// nothing.
    pub digest: String,
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
    /// Whether the seal's key on this machine is another program's, most likely an earlier
    /// build's before an update (#7714): nothing holds, and the window offers Make a new key and
    /// accept ([`accept_project_policy_with_new_key`]) in place of Accept.
    pub key_foreign: bool,
    /// Whether the acceptance does not hold because purlis made a new seal key on this machine
    /// since it was sealed (#7714), never because something edited it: the window says so.
    pub new_key: bool,
    /// Whether purlis vouches that this project's never-dispatch list was anchored by a seal key
    /// it has replaced since (#7714): it reads as unread until the person keeps the pairs shown
    /// ([`keep_project_nevers`]). Accept never seals them.
    pub nevers_new_key: bool,
    /// The pairs Keep these nevers keeps, where `nevers_new_key` and the list's file reads.
    pub nevers_to_keep: Option<NeversToKeep>,
    /// Whether that list holds a name purlis cannot show faithfully (#7734's round 4): no pair
    /// is offered to keep, and the window says so.
    pub nevers_unshowable: bool,
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

/// What the waiting row for the merge opt-in is named: the route changes, not the hook's mode.
const MERGE_ROUTE_ROW: &str = "merges through the forge route";

/// What the waiting row for lineage is named: a parent taking up its ended child's branch through
/// the forge route (`[policy] lineage`).
const LINEAGE_ROW: &str = "lineage through the forge route";

/// What a waiting row for a protected branch the project dropped is named, before the branch.
const PROTECTED_ROW: &str = "protected branch";

/// The project's `[policy]` as the window says it: one [`enforcement::standing`], so the template,
/// the families set apart, what waits and the digest Accept names are all of one read of the
/// file. Only reads: the core alone resolves the policy, and nothing here keeps anything.
fn state_of(root: &std::path::Path) -> PolicyState {
    state_of_standing(enforcement::standing(root))
}

/// [`state_of`], of a read already made: what the prerequisite list hands on of the same read
/// it found the prerequisite in (`crate::prerequisites`).
pub(crate) fn state_of_standing(standing: enforcement::Standing) -> PolicyState {
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
    PolicyState {
        waiting: standing
            .waiting
            .into_iter()
            .map(|(family, accepted, committed)| {
                // The merge opt-in's row has equal modes, because the hook already runs the
                // mode (`watch` is `log`): what changes is the forge route, held until now.
                if family == enforcement::Family::GuardsMerge && accepted == committed {
                    return PolicyWaiting {
                        family: MERGE_ROUTE_ROW.to_owned(),
                        accepted: "held".to_owned(),
                        committed: committed.word().to_owned(),
                    };
                }
                PolicyWaiting {
                    family: family.key().to_owned(),
                    accepted: accepted.word().to_owned(),
                    committed: committed.word().to_owned(),
                }
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
            // Lineage the person accepted a denial of, that the project's file now allows, and a
            // protected branch it no longer names, wait for the same press.
            .chain(standing.lineage_waiting.then(|| PolicyWaiting {
                family: LINEAGE_ROW.to_owned(),
                accepted: "denied".to_owned(),
                committed: "allowed".to_owned(),
            }))
            .chain(
                standing
                    .protection_lifted
                    .into_iter()
                    .map(|name| PolicyWaiting {
                        family: format!("{PROTECTED_ROW} {name}"),
                        accepted: "protected".to_owned(),
                        committed: "lifted".to_owned(),
                    }),
            )
            .collect(),
        refused: standing.refused,
        not_in_force: standing
            .not_in_force
            .into_iter()
            .map(|family| family.key().to_owned())
            .collect(),
        unsealed: standing.unsealed,
        key_foreign: standing.key_foreign,
        new_key: standing.new_key,
        nevers_new_key: standing.nevers_new_key,
        nevers_to_keep: standing.nevers_to_keep.map(|kept| NeversToKeep {
            pairs: kept
                .pairs
                .into_iter()
                .map(|(asking, target)| NeverPair { asking, target })
                .collect(),
            digest: kept.digest,
        }),
        nevers_unshowable: standing.nevers_unshowable,
        digest: standing.digest,
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

/// **Your press: accept the project's `[policy]` as its committed file said it when it was
/// shown** (`shown`, the state's digest), looser where it is looser, at once. Sealed on this
/// machine by the app; refused where it cannot be sealed, or where the file changed since it was
/// shown. The window's alone. Made through the project's prerequisite list, which is evaluated
/// again after it and tells every window (ADR 0130 §2).
#[tauri::command]
#[specta::specta]
pub async fn accept_project_policy(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: String,
) -> Result<PolicyState, String> {
    let held = planes.held(&plane)?;
    let (root, kept) = (held.root().to_path_buf(), held.prerequisites());
    crate::off_the_window(READING_POLICY, move || {
        pressed(&root, &kept, &shown, || {
            every_other_list(&app.state::<Planes>(), &plane)
        })
    })
    .await
}

/// [`accept_project_policy`] on the project at `root`, whose list `kept` holds, off the window's
/// thread; then the list of each project `others` lists is evaluated again ([`evaluate_each`]).
pub(crate) fn pressed(
    root: &std::path::Path,
    kept: &purlis_core::prerequisites::Kept,
    shown: &str,
    others: impl FnOnce() -> Others,
) -> Result<PolicyState, String> {
    let pressed = kept.accept(root, shown);
    evaluate_each(others());
    pressed
        .map_err(|why| format!("purlis could not keep your acceptance on this machine: {why}"))?;
    Ok(state_of(root))
}

/// The list of each project of `others` evaluated again: after any Accept, which may make the
/// seal's key, read it again where it was another program's (#7073), or replace it (#7714),
/// whatever the press came to. `others` is listed once the press, and any Keychain prompt it
/// waited on, is done, so a project opened meanwhile is among them.
fn evaluate_each(others: Others) {
    for (other, list) in others {
        list.evaluate(&other);
    }
}

/// **Your press where the seal's key is another program's: make a new key and accept the
/// project's `[policy]` as it was shown** (#7714), on the project `plane` names, never the one in
/// front when it lands. The old `purlis-seal` items are replaced, which macOS may ask you to
/// allow; the digest is checked again once its prompt returns; then a new key seals this
/// project's acceptance. Every other project accepted on this machine then needs accepting
/// again, and each one's list is evaluated again. The window's alone. The window draws it in the
/// prerequisite modal (#7719).
#[tauri::command]
#[specta::specta]
pub async fn accept_project_policy_with_new_key(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: String,
) -> Result<PolicyState, String> {
    let held = planes.held(&plane)?;
    let (root, kept) = (held.root().to_path_buf(), held.prerequisites());
    crate::off_the_window(READING_POLICY, move || {
        new_key_pressed(&root, &kept, &shown, || {
            every_other_list(&app.state::<Planes>(), &plane)
        })
    })
    .await
}

/// Each held project but one, by its root, with the list the app holds for it.
pub(crate) type Others = Vec<(
    std::path::PathBuf,
    std::sync::Arc<purlis_core::prerequisites::Kept>,
)>;

/// Each held project but `plane`, by its root, with the list the app holds for it.
pub(crate) fn every_other_list(planes: &Planes, plane: &PlaneId) -> Others {
    planes
        .open_now()
        .into_iter()
        .filter(|other| other != plane)
        .filter_map(|other| planes.held(&other).ok())
        .map(|held| (held.root().to_path_buf(), held.prerequisites()))
        .collect()
}

/// [`accept_project_policy_with_new_key`] on the project at `root`, whose list `kept` holds, off
/// the window's thread; then the list of each project `others` lists is evaluated again, since
/// a new seal key breaks every acceptance the old one sealed ([`evaluate_each`]).
fn new_key_pressed(
    root: &std::path::Path,
    kept: &purlis_core::prerequisites::Kept,
    shown: &str,
    others: impl FnOnce() -> Others,
) -> Result<PolicyState, String> {
    let pressed = kept.accept_with_new_key(root, shown);
    evaluate_each(others());
    pressed
        .map_err(|why| format!("purlis could not keep your acceptance on this machine: {why}"))?;
    Ok(state_of(root))
}

/// **Your press: keep the never-dispatch pairs the window showed, sealed with the new seal key**
/// (#7714, H1 of #7734's round 3), on the project `plane` names. Its own press, never a part of
/// Accept: `shown` is the digest of the pairs shown ([`NeversToKeep`]), so a list that changed
/// since keeps nothing. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn keep_project_nevers(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: String,
) -> Result<PolicyState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(READING_POLICY, move || nevers_kept(&root, &shown)).await
}

/// [`keep_project_nevers`] on the project at `root`, off the window's thread.
fn nevers_kept(root: &std::path::Path, shown: &str) -> Result<PolicyState, String> {
    purlis_core::dispatchnever::keep_shown_after_a_new_key(root, shown)
        .map_err(|why| format!("purlis could not keep your never-dispatch list: {why}"))?;
    Ok(state_of(root))
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
    fn the_new_key_press_is_the_windows_alone_as_accept_is() {
        // #7714: making a new seal key replaces what every plane's seal rests on, so no chat
        // reaches it through a link, as no chat reaches Accept.
        for press in [
            "accept_project_policy",
            "accept_project_policy_with_new_key",
            "keep_project_nevers",
        ] {
            assert!(purlis_session_protocol::ui::window_only(press), "{press}");
            assert!(
                !purlis_session_protocol::ui::served_on_a_link(press),
                "{press}"
            );
        }
    }

    #[test]
    fn the_new_key_press_reads_the_project_it_names_and_no_other() {
        // #7714, ruling 6: the press is bound to its own project. What another project shows is
        // never accepted on this one.
        let (one, other) = (
            plane("[policy]\ntemplate = \"open\"\n"),
            plane("[policy]\ntemplate = \"team\"\n"),
        );
        let planes = Planes::telling(std::sync::Arc::new(|_| {}), crate::Shipped::default(), None);
        let (first, second) = (planes.open(one.path()), planes.open(other.path()));
        let held = planes.held(&first).expect("held");
        let (root, kept) = (held.root().to_path_buf(), held.prerequisites());
        let others = || every_other_list(&planes, &first);
        let shown_there = state_of(planes.held(&second).expect("held").root()).digest;
        let why =
            new_key_pressed(&root, &kept, &shown_there, others).expect_err("nothing accepted");
        assert!(why.contains("changed since it was shown"), "{why}");
        // What this project shows passes, and is then refused only because a test is not the
        // app, which alone seals.
        let why = new_key_pressed(&root, &kept, &state_of(&root).digest, others)
            .expect_err("not the app");
        assert!(!why.contains("changed since it was shown"), "{why}");
    }

    /// Each list told of `planes`' projects since the last take.
    type Heard = std::sync::Arc<std::sync::Mutex<Vec<crate::prerequisites::Prerequisites>>>;

    /// **`press` on the first of three held projects evaluates the other two again** (#7885's
    /// review, F3 and F4): their folders are not watched, so a change to them is heard by
    /// nothing but the press.
    fn every_other_project_is_evaluated_again_after(
        press: impl FnOnce(
            &std::path::Path,
            &purlis_core::prerequisites::Kept,
            &str,
            &dyn Fn() -> Others,
        ),
    ) {
        const WATCH: &str = "[policy]\ntemplate = \"watch\"\n";
        let dirs = [
            plane("[policy]\ntemplate = \"open\"\n"),
            plane(WATCH),
            plane(WATCH),
        ];
        let refused: Vec<_> = dirs
            .iter()
            .map(|dir| std::fs::canonicalize(dir.path()).expect("real"))
            .collect();
        for dir in &refused[1..] {
            crate::watchset::refuse::refuse(dir);
        }
        let heard = Heard::default();
        let planes = Planes::telling(std::sync::Arc::new(|_| {}), crate::Shipped::default(), None)
            .telling_prerequisites({
                let heard = std::sync::Arc::clone(&heard);
                std::sync::Arc::new(move |told| heard.lock().expect("heard").push(told))
            });
        let ids: Vec<PlaneId> = dirs.iter().map(|dir| planes.open(dir.path())).collect();
        let by = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while heard.lock().expect("heard").len() < ids.len() {
            assert!(std::time::Instant::now() < by, "never: each open's list");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        heard.lock().expect("heard").clear();
        for dir in &dirs[1..] {
            std::fs::write(
                dir.path().join("charter.toml"),
                "[policy]\ntemplate = \"open\"\n",
            )
            .expect("loosened");
        }
        let held = planes.held(&ids[0]).expect("held");
        let (root, kept) = (held.root().to_path_buf(), held.prerequisites());
        press(&root, &kept, &state_of(&root).digest, &|| {
            every_other_list(&planes, &ids[0])
        });
        let told: Vec<PlaneId> = std::mem::take(&mut *heard.lock().expect("heard"))
            .into_iter()
            .filter(|told| !told.list.is_empty())
            .map(|told| told.plane)
            .collect();
        for dir in &refused[1..] {
            crate::watchset::refuse::allow(dir);
        }
        for other in &ids[1..] {
            assert!(
                told.contains(other),
                "{other:?} not evaluated again: {told:?}"
            );
        }
    }

    #[test]
    fn the_new_key_press_evaluates_every_other_held_project_again() {
        // #7714: a new key breaks every acceptance the old one sealed, whatever the press came
        // to (a test is not the app, so here it comes to a refusal).
        every_other_project_is_evaluated_again_after(|root, kept, shown, others| {
            let _ = new_key_pressed(root, kept, shown, others);
        });
    }

    #[test]
    fn an_accept_evaluates_every_other_held_project_again() {
        // An Accept may make the seal's key, or read it again where it was another program's
        // (#7073): every other project's list may move with it.
        every_other_project_is_evaluated_again_after(|root, kept, shown, others| {
            let _ = pressed(root, kept, shown, others);
        });
    }

    #[test]
    fn keeping_the_nevers_where_no_new_key_listed_them_keeps_nothing() {
        // H2 of #7734's round 3: the press seals only a list purlis vouches a new key left
        // unread, so a project no new key listed shows nothing to keep and keeps nothing.
        let dir = plane("[policy]\ntemplate = \"open\"\n");
        assert_eq!(state_of(dir.path()).nevers_to_keep, None);
        let none = purlis_core::dispatchnever::digest_of_pairs(&[]);
        let why = nevers_kept(dir.path(), &none).expect_err("nothing kept");
        assert!(why.contains("kept nothing"), "{why}");
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
