// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's skills, as purlis-core asks [`purlis_skills`] for them (ADR 0063, SK-1).
//!
//! How a chat's skill set is worked out is `purlis-skills`' alone: which skills a chat gets, by
//! which [`Route`] and from which [`Origin`], and why each left-out one is left out. This module
//! hands that crate what purlis-core knows about a chat, where the chat is armed
//! ([`for_chat`]) and where a briefed chat's `SessionStart` lists its skills ([`briefed`]), and
//! it holds the `skill-studio` flag ([`studio_on`]). A harness adapter asks [`for_chat`] and
//! turns the answer into its harness's argv, env and config; the route each harness takes is its
//! declaration's fact, [`crate::harness::Harness::skills`].

use std::path::{Path, PathBuf};

pub use purlis_skills::change::{
    BuiltInTemplates, Offer, Problem, SkillTemplate, Template, TemplateId, check,
};
/// The line format's one parser, and its `str.strip` and `str.splitlines` (#3482).
pub use purlis_skills::fence;
pub use purlis_skills::{
    Argument, At, BUILT_IN_OWN_PREFIX, BUILT_IN_PREFIXES, Budget, Chat, Clash, Compat,
    DIR_IN_BUNDLE, Declared, Effective, Field, Finding, Fit, FromHarness, Given, Grant,
    HarnessSkills, Honours, LOCAL_DIR, LeftOut, Lint, Machine, ManualOnly, Missing, Narrowing,
    OnMachine, Origin, Outcome, OutsideSkill, Over, Owner, POINTER, PROJECT_DIR, Reason, Reference,
    Refusal, Reload, Route, Severity, Shared, Skill, SkillSet, SkillsChanged, Spent, Stamp, Switch,
    Whose, beside, compat, declared, grant_file, grant_of, handed_text, handed_text_as, in_bundle,
    leaf_name, level_dir, level_shown, listing, named, named_at, persona_dir, read, scan,
    set_grant, share,
};
pub use purlis_skills::{Hidden, HiddenWhy, INDEX, Index, Router, Routing, StillOver};
pub use purlis_skills::{SkillFiles, SkillLint};

/// A skill's grading (SK-16): its fixtures, the four messages and the built-in runner.
pub use purlis_skills::grade;
/// Skill runs: the span from the moment a skill starts to the end of the turn (SK-3).
pub use purlis_skills::runs;
/// The install review's summary and a source's tree (SK-10).
pub use purlis_skills::{review, tree};
pub mod source;
pub use purlis_skills::{About, Deprecated, Lifecycle, about_of};
pub mod first_open;
pub mod grades;
pub mod graph;
pub mod history;
pub mod lifecycle;
pub mod studio;
pub mod verbs;

use crate::harness::{Harness, Kit};

pub mod levels;
mod on_machine;
pub mod persona;
pub mod router;
mod routing;

pub use routing::{
    PinnedBy, PinsHeld, PinsRefused, PinsTogether, pinners_of, pins_file, pins_refused,
    pins_written,
};
pub(crate) use routing::{pin, pinners_of_a_chat, pins_or_none, pins_together};

pub use on_machine::on_machine;
pub(crate) use on_machine::{folders, skill_dirs};

/// **The Skill studio's records in this machine's state folder**, each written by the app or the
/// session host alone and read back as its own, so a chat's sandbox denies writing each, with
/// what is below it (`sandbox::kept_by_purlis`): the records of which levels a running chat could
/// have made and the folders not accepted yet (SK-9), the drafts and the tries and grades' arms
/// (SK-9, SK-16), the grades of local copies, the grades under way and their lock, and the chats
/// grades kept (SK-16, SK-59), and the opt-in to grade after each edit and its lock (SK-58).
pub(crate) const KEPT_IN_STATE: [&str; 9] = [
    levels::DIR,
    verbs::drafts::DRAFTS_DIR,
    verbs::drafts::TRIES_DIR,
    grades::LOCAL_DIR,
    grades::kept::DIR_NAME,
    grades::underway::DIR,
    grades::underway::CLAIM,
    grades::after_edit::FILE,
    grades::after_edit::LOCK,
];

/// The variable a chat whose harness cannot load its skills is started with: the directory of
/// the built-in skills its set holds ([`SkillSet::built_in_dir`]), whose skills its `SessionStart` briefing then
/// lists ([`briefed`]). Set by the app for such a chat alone, so a harness that loads the skills
/// itself is never briefed on them twice.
pub const LISTED_ENV: &str = "PURLIS_SKILLS_DIR";

/// The variable a briefed chat is started with when its set holds the plane's skills (ADR 0099):
/// the set's folder purlis laid out for it ([`lay_out`]), and its `SessionStart` briefing then
/// lists them with the built-in ones, each by the `SKILL.md` purlis wrote for it there (SK-8), so
/// a briefed chat reads what every other route hands. Set only where the `skill-studio` flag let
/// the plane's skills into the chat's set and their folder was laid out, so a briefing never reads
/// the plane where arming did not.
pub const PLANE_LISTED_ENV: &str = "PURLIS_SKILLS_PLANE";

/// The plane a chat's skills come from beside the built-in ones (ADR 0099), as a chat is armed.
#[derive(Debug, Clone, Copy)]
pub struct PlaneSkills<'a> {
    /// The plane's root.
    pub plane: &'a Path,
    /// The workspace the chat works in, or `None` for a chat at the plane root.
    pub workspace: Option<&'a str>,
    /// The repo skills of the clone the chat works in, for its harness, which the host read
    /// ([`repo_skills_of_chat`]): where one has a plane skill's name and purlis builds what the
    /// harness is handed, the narrower repo copy wins and the plane's is left out (SK-6, decision
    /// 51). Empty for a chat in no clone.
    pub repo: &'a [OutsideSkill],
    /// What the chat's router extension answered at its start (SK-15b), which every routing of
    /// its set applies again: `None` for a chat routed by the built-in router.
    pub routed: Option<&'a router::Routed>,
}

/// The skill set of a chat on `harness` armed with `kit`, asked of [`purlis_skills::skill_set`].
///
/// The built-in skills come from the bundle in `kit`, and the plane's from `kit.skills`, where
/// the `skill-studio` flag let them in.
pub fn for_chat(kit: Kit<'_>, harness: Harness) -> SkillSet {
    let built_in = kit.plugin.and_then(in_bundle);
    routed(
        built_in.as_deref(),
        kit.skills,
        kit.persona.map(|persona| persona.persona),
        harness,
    )
}

/// **The harness's own skills a chat on `harness` armed with `kit` is handed as turned off**
/// (SK-6, #2769): each of `chosen`'s, except a built-in skill's name, which is never turned off
/// (a harness that switches by the name alone would reach purlis's own), and one the harness
/// cannot leave out on its own ([`crate::harness_plugin::Adapter::skill_named_alone`]). What
/// every adapter turns into its own switch.
///
/// And the index skill's name (SK-15), where the chat's set holds the plane's skills, a skill of
/// the harness's own or of the chat's clone has it, and the harness can leave that copy out by
/// name and keep purlis's ([`Route::off_by_name_spares_handed`]): so exactly one copy of the name
/// reaches the chat, purlis's.
pub fn skills_off_for(
    kit: Kit<'_>,
    harness: Harness,
    chosen: &crate::harness_plugin::Chosen,
) -> Vec<String> {
    // By the name the harness knows each by (`purlis-<name>` on opencode's route): a harness
    // skill of a built-in skill's bare name is then another skill, and turned off as any is.
    // And the index skill's own name there, `purlis-skill-index` (SK-15).
    let built_in = harness.skills().built_in_names(
        built_in_names(kit.plugin.and_then(in_bundle).as_deref())
            .iter()
            .map(String::as_str),
    );
    let adapter = harness.adapter().plugins();
    let mut off: Vec<String> = chosen
        .skills_off
        .iter()
        .filter(|name| !built_in.contains(name.as_str()))
        .filter(|name| adapter.skill_named_alone(name).is_none())
        .cloned()
        .collect();
    let index_elsewhere = chosen.own_index
        || kit
            .skills
            .is_some_and(|at| at.repo.iter().any(|one| one.id == INDEX && !one.off));
    if kit.skills.is_some()
        && harness.skills().off_by_name_spares_handed()
        && !off.iter().any(|name| name == INDEX)
        && index_elsewhere
    {
        off.push(INDEX.to_owned());
    }
    off
}

/// **Whether `harness` has a skill of its own named [`INDEX`]**, outside any plugin, where `env`
/// finds its skills (SK-15; #3320): what [`skills_off_for`] turns off so the chat gets purlis's
/// index alone. Read only on a route where turning that name off spares purlis's copy
/// ([`Route::off_by_name_spares_handed`]); elsewhere it is never turned off.
///
/// A plugin's skill of that name is another skill, never turned off: Claude Code knows it as
/// `<plugin>:skill-index`, and so does Codex. Measured on codex-cli 0.163.0 in a throwaway
/// `CODEX_HOME` holding its own `skills/skill-index` and a plugin `zp` from a local marketplace
/// with a `skill-index` of its own: `codex debug prompt-input` listed `skill-index` and
/// `zp:skill-index`, and `-c 'skills.config=[{name="skill-index",enabled=false}]'` took out the
/// first alone. purlis's index reaches a Codex chat in its briefing as `skill-index`, so the
/// plugin's never shares its name.
pub fn has_own_index(harness: Harness, env: &crate::harness_plugin::Env<'_>) -> bool {
    harness.skills().off_by_name_spares_handed()
        && on_machine(harness, env)
            .iter()
            .any(|one| matches!(one, OnMachine::Skill { plugin: None, name } if name == INDEX))
}

/// The names of the built-in skills of the bundle installed beside this binary
/// ([`built_in_here`]): what a save of `[harness_skills]` refuses to turn off.
pub fn built_in_names_here() -> Vec<String> {
    built_in_names(built_in_here().as_deref())
}

/// The names of the built-in skills in `dir`, the bundle's `skills/` ([`in_bundle`]): none where
/// there is no bundle. The one list a switch, a save and an adapter refuse to turn off.
pub fn built_in_names(dir: Option<&Path>) -> Vec<String> {
    dir.map(|dir| read(dir).into_iter().map(|skill| skill.name).collect())
        .unwrap_or_default()
}

/// **The skills purlis ships**, by name: the bundle's `plugin/skills/<name>/SKILL.md`, each
/// folder named as its skill (held to that folder by a test). Compiled in, so a name stays
/// reserved ([`name_reserved`]) where this binary finds no bundle beside it.
pub const SHIPPED: [&str; 11] = [
    "add-curation-action",
    "browser",
    "compact",
    "handoff",
    "persona",
    "safe-remove",
    "secrets",
    "smart-close",
    "theme",
    "update",
    "working-in-a-clone",
];

/// **What a skill that would take a built-in skill's name is refused with** (spec #3679 S12),
/// by the window and `purlis skill` alike.
pub const NAME_RESERVED: &str =
    "That name belongs to a built-in skill; pick another, or make a copy.";

/// **The names that belong to a built-in skill**, so no skill of the plane may take one (spec
/// #3679 S12, ADR 0099 §3): each skill purlis ships ([`SHIPPED`]) or the bundle's `skills/`
/// holds, and every name a chat knows purlis's own skills by on any route
/// ([`Route::built_in_names`]: `purlis-<name>`, and the index skill's). Read once, then asked of
/// as many names as need it. A skill of such a name never reaches a chat beside the built-in one
/// anyway; this keeps one from being made at all.
pub struct Reserved(std::collections::HashSet<String>);

impl Reserved {
    /// The names reserved with the built-in skills in `built_in`, the bundle's `skills/`, read
    /// once: [`SHIPPED`]'s, and whatever it holds.
    pub fn of(built_in: Option<&Path>) -> Reserved {
        let bundled = built_in_names(built_in);
        let bare: Vec<&str> = SHIPPED
            .iter()
            .copied()
            .chain(bundled.iter().map(String::as_str))
            .chain([INDEX])
            .collect();
        let mut names = Route::Config.built_in_names(bare.iter().copied());
        names.extend(bare.iter().map(|name| (*name).to_owned()));
        Reserved(names)
    }

    /// Whether `name` belongs to a built-in skill.
    pub fn holds(&self, name: &str) -> bool {
        self.0.contains(name)
    }
}

/// **Whether `name` belongs to a built-in skill** ([`Reserved`]) with the built-in skills in
/// `built_in`: the one check every change that lands a skill under a name makes
/// ([`verbs::Ctx::not_built_in`]), Make my own copy's too (#3686).
pub fn name_reserved(built_in: Option<&Path>, name: &str) -> bool {
    Reserved::of(built_in).holds(name)
}

/// **A skill of the plane named as a built-in skill** (spec #3679 S12): one made before its name
/// was reserved, by a purlis older than the built-in, or written into the plane by hand. No chat
/// gets it, since the built-in skill has its name; Needs you lists it as a clash to rename
/// (#3689).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedAsABuiltIn {
    pub name: String,
    pub level: verbs::Level,
    /// Its folder, relative to the plane, `/` between the parts.
    pub folder: String,
}

/// **Every skill of the plane at `plane` named as a built-in skill** ([`name_reserved`], with the
/// built-in skills in `built_in`), at every level ([`verbs::Level::every`]), in that order.
pub fn named_as_a_built_in(plane: &Path, built_in: Option<&Path>) -> Vec<NamedAsABuiltIn> {
    let reserved = Reserved::of(built_in);
    let mut out = Vec::new();
    for level in verbs::Level::every(plane) {
        let Some(dir) = level.dir(plane) else {
            continue;
        };
        for skill in read(&dir) {
            if reserved.holds(&skill.name) {
                out.push(NamedAsABuiltIn {
                    folder: verbs::rel(plane, skill.path.parent().unwrap_or(&skill.path)),
                    name: skill.name,
                    level: level.clone(),
                });
            }
        }
    }
    out
}

/// The folder of links a harness on [`Route::Plugin`] or [`Route::Config`] is handed the skills
/// of `set` its folder holds by ([`SkillSet::in_the_folder`]: the plane's, and on the config
/// route the built-in ones under their own names): the one the host laid out
/// ([`Kit::laid_out`]), where the set holds such a skill. Nothing is read or written here: an
/// adapter only turns the set into argv, env and config (ADR 0063 as amended 2026-10-20).
pub fn linked<'a>(set: &SkillSet, kit: Kit<'a>) -> Option<&'a Path> {
    if set.in_the_folder().next().is_none() && set.index().is_none() {
        return None;
    }
    kit.laid_out
}

/// **The host's part of arming a chat with the skills its set's folder holds**: the folder of
/// links for the set of a chat whose plane skills come from `at` (its plane, its workspace or
/// none, the repo skills of its clone and its router's answer), running as `persona`, with the
/// built-in skills in `built_in`, laid out under `sets`. `at` is `None` where the plane's skills
/// do not reach chats: the set is then the built-in skills alone, laid out only on a route that
/// names them their own ([`Route::names_built_ins_its_own`]). `None` where the set holds nothing
/// the folder holds ([`SkillSet::in_the_folder`]), or the folder could not be laid out (said in
/// the log). The set is the one [`for_chat`] works out for that chat on `harness`: a plane skill
/// a narrower repo copy hides is not laid out (SK-6), nor one its router hid (SK-15b). Each skill
/// is handed as `harness` reads it ([`crate::harness::Harness::skill_frontmatter`], SK-8).
pub fn lay_out_for_chat(
    at: Option<PlaneSkills<'_>>,
    persona: Option<&str>,
    built_in: Option<&Path>,
    sets: &Path,
    harness: Harness,
) -> Option<PathBuf> {
    let set = routed(built_in, at, persona, harness);
    match lay_out(&set, sets, &harness.skill_frontmatter()) {
        Ok(folder) => folder,
        Err(why) => {
            tracing::warn!(
                "purlis: this chat's skills could not be laid out under {} ({why}); {}",
                sets.display(),
                without_the_folder(harness)
            );
            None
        }
    }
}

/// What a chat on `harness` starts with where its set's folder could not be laid out, as the
/// log says it.
pub(crate) fn without_the_folder(harness: Harness) -> &'static str {
    if harness.skills().names_built_ins_its_own() {
        "it starts without purlis's skills"
    } else {
        "it has the built-in skills alone"
    }
}

/// The name of the plugin a set's folder is, in Claude Code: its skills are `skills:<name>`.
pub const LINKED_PLUGIN: &str = "skills";

/// **The folder of the skills of `set` purlis hands in a folder** ([`SkillSet::in_the_folder`]:
/// the plane's, and on a route that names built-in skills their own the built-in ones under
/// that name), under `sets`, laid out if it is not there yet, for a harness that reads
/// `honours`, or `None` for a set with none of them (ADR 0063 as amended 2026-10-20 and
/// 2026-10-22; ADR 0099 as amended 2026-10-26).
///
/// `<sets>/<digest>/` holds `.claude-plugin/plugin.json`, naming the plugin [`LINKED_PLUGIN`],
/// and `skills/<name>`, a folder of each skill's own ([`handout`]): the `SKILL.md` purlis writes
/// for it, never with what would widen the chat, and a link to each plain file of the skill's
/// folder. So Claude Code loads it as a plugin, opencode scans its `skills/`, and a Codex
/// briefing names its files. A skill purlis refuses is left out, and its chat's tab says why
/// ([`not_here`]). `<digest>` is a SHA-256 over every name, folder, text and entry, so the folder
/// of one set is never another's, and it is complete before it takes that name: a running
/// chat's folder never changes. One found complete is used as it is; one that is not is made
/// again.
pub fn lay_out(
    set: &SkillSet,
    sets: &Path,
    honours: &purlis_skills::Honours,
) -> std::io::Result<Option<PathBuf>> {
    let mut handed: Vec<handout::Handed> = set
        .in_the_folder()
        .filter(|effective| plain_name(&effective.skill.name))
        .filter_map(|effective| handout::of(effective, honours).ok())
        .collect();
    // The index skill, in place of what a router hid (SK-15, ADR 0102 §2).
    handed.extend(
        set.index()
            .and_then(|index| handout::index(&index, honours).ok()),
    );
    if handed.is_empty() {
        return Ok(None);
    }
    let digest = {
        use sha2::Digest;
        let mut hash = sha2::Sha256::new();
        for one in &handed {
            handout::digested(one, &mut hash);
        }
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let folder = sets.join(&digest);
    std::fs::create_dir_all(sets)?;
    // The sets' folder, opened as a folder itself once: its lock and the removal of an
    // incomplete folder go through this handle, never back through the path.
    #[cfg(unix)]
    let sets_dir = at::folder(sets).map_err(|why| {
        if at::not_a_folder(&why) {
            std::io::Error::other(format!("{} is a link, not a folder", sets.display()))
        } else {
            why
        }
    })?;
    // Held against [`tidy`] while it is laid out and found: a folder this returns has just been
    // made or touched, so a tidy after it keeps it for [`TIDY_GRACE`].
    #[cfg(unix)]
    let _laying_out = lock_shared_in(&sets_dir)?;
    #[cfg(not(unix))]
    let _laying_out = lock_shared(sets)?;
    if laid_out(&folder, &handed) && touched(&folder) {
        return Ok(Some(folder));
    }
    let building = tempfile::Builder::new()
        .prefix(&format!(".{digest}."))
        .tempdir_in(sets)?;
    build(building.path(), &handed)?;
    if std::fs::symlink_metadata(&folder).is_ok() {
        // Not complete, so not a set's folder any chat was started with: made again.
        #[cfg(unix)]
        at::remove_tree_in(&sets_dir, &digest)?;
        #[cfg(not(unix))]
        std::fs::remove_dir_all(&folder)?;
    }
    let built = building.keep();
    match std::fs::rename(&built, &folder) {
        Ok(()) => Ok(Some(folder)),
        // Another chat's start laid the same set out first.
        Err(_) if laid_out(&folder, &handed) => {
            let _ = std::fs::remove_dir_all(&built);
            Ok(Some(folder))
        }
        Err(why) => {
            let _ = std::fs::remove_dir_all(&built);
            Err(why)
        }
    }
}

/// Builds a set's folder of `handed` in `building`, a folder of purlis's own that nothing else
/// knows of yet: through the handles of the folders made in it, where there are handles.
#[cfg(unix)]
fn build(building: &Path, handed: &[handout::Handed]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mode = rustix::fs::Mode::from_raw_mode(0o755);
    let root = at::folder(building)?;
    rustix::fs::mkdirat(&root, ".claude-plugin", mode)?;
    let manifest = at::folder_in(&root, ".claude-plugin")?;
    at::new_file_in(&manifest, "plugin.json")?.write_all(manifest_text().as_bytes())?;
    rustix::fs::mkdirat(&root, DIR_IN_BUNDLE, mode)?;
    let skills = at::folder_in(&root, DIR_IN_BUNDLE)?;
    for one in handed {
        handout::make_in(&skills, one)?;
    }
    Ok(())
}

/// [`build`] where there are no handles to open a folder by.
#[cfg(not(unix))]
fn build(building: &Path, handed: &[handout::Handed]) -> std::io::Result<()> {
    let manifest = building.join(".claude-plugin");
    std::fs::create_dir(&manifest)?;
    std::fs::write(manifest.join("plugin.json"), manifest_text())?;
    let skills = building.join(DIR_IN_BUNDLE);
    std::fs::create_dir(&skills)?;
    for one in handed {
        handout::make_at(&skills, one)?;
    }
    Ok(())
}

/// Marks `folder` as used now, by its modification time, which [`tidy`] keeps a folder for.
fn touched(folder: &Path) -> bool {
    std::fs::File::open(folder)
        .and_then(|dir| dir.set_modified(std::time::SystemTime::now()))
        .is_ok()
}

/// The variable a chat's start carries the folder of links it is handed by (SK-24): set by the
/// host as it arms the chat ([`crate::hoststart::armed`]), and taken out of the chat's
/// environment again by [`take_hold`] where its terminal is started, so its program never sees
/// it and the terminal notes, while the program runs, that it uses the folder.
pub const SET_ENV: &str = "PURLIS_SKILL_SET";

/// The folder of a set's folder's notes, beside the sets: `<sets>/.held/<digest>/<pid>`, one
/// file for each running program that was handed it ([`crate::session::Spec::holds`]).
pub(crate) const HELD: &str = ".held";

/// The file [`lay_out`] and [`tidy`] take turns on, in the folder of the sets.
const LOCK: &str = ".lock";

/// How long a set's folder is kept after it was last laid out or found, whether or not a
/// running program is noted as using it: the time between a host laying it out and the chat's
/// terminal noting it, with room to spare.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) const TIDY_GRACE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// **Where a program handed the set's folder `linked` is noted as using it**: `<sets>/.held/
/// <digest>`, or `None` for a folder that is not a set's folder of links ([`lay_out`]'s
/// `<sets>/<digest>`, a real folder).
pub(crate) fn hold_of(linked: &Path) -> Option<PathBuf> {
    let digest = linked.file_name()?.to_str()?;
    let sets = linked.parent()?;
    (is_digest(digest)
        && sets.file_name() == Some(std::ffi::OsStr::new(crate::hoststart::SKILL_SETS))
        && crate::leftovers::is_plain_dir(linked))
    .then(|| sets.join(HELD).join(digest))
}

/// **Takes [`SET_ENV`] out of a chat's `env`**, and answers where its program is noted as using
/// the folder it names ([`hold_of`]), where it names a set's folder.
pub fn take_hold(env: &mut Vec<(std::ffi::OsString, std::ffi::OsString)>) -> Option<PathBuf> {
    // Under every name it goes by (V93k), as the chat's environment twins it.
    let names = crate::envvar::spellings(SET_ENV);
    let mut handed = None;
    env.retain(|(key, value)| {
        if !names.iter().any(|name| key == name.as_str()) {
            return true;
        }
        handed.get_or_insert_with(|| PathBuf::from(value));
        false
    });
    handed.as_deref().and_then(hold_of)
}

/// What one [`tidy`] did: the sets' folders it removed, by digest, and each it could not, with
/// why.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tidied {
    pub removed: Vec<String>,
    pub failed: Vec<(String, String)>,
    /// Nothing was looked at: a chat's start was laying its folder out, and a tidy never makes
    /// one wait. The next tidy looks.
    pub busy: bool,
}

/// **Removes every set's folder under `sets` no running program uses** (SK-24): one no live
/// program is noted in ([`HELD`]) and that was not laid out or found within [`TIDY_GRACE`],
/// with its notes, and what a lay-out that never finished left. A tidy never makes a chat's
/// start wait: while one is laying a folder out ([`lay_out`]), it looks at nothing (`busy`).
///
/// **Only `sets`' own entries are removed, by the names purlis gives them**: a folder named by
/// a digest, a lay-out's leftover beside it, and the notes in [`HELD`] named by a process id.
/// **Nothing is read, removed or written through a link**, even one put in place after a check:
/// `sets`, [`HELD`], each folder of notes and each folder removed are opened as folders
/// themselves and worked through their handles (`skills::at`). A link at [`HELD`] or at a
/// folder of notes keeps the folders it may speak for. The links inside a set's folder are
/// removed as links. Off unix nothing is removed: whether a program still runs cannot be
/// asked there ([`crate::process::alive`]).
pub fn tidy(sets: &Path) -> std::io::Result<Tidied> {
    #[cfg(unix)]
    {
        tidy_through_handles(sets)
    }
    #[cfg(not(unix))]
    {
        let _ = sets;
        Ok(Tidied::default())
    }
}

/// What a folder of notes says of its set's folder.
#[cfg(unix)]
#[derive(PartialEq)]
enum Notes {
    /// None of a running program, or none at all.
    None,
    /// One of a running program, or notes that cannot be read for a link in their place.
    Kept,
}

#[cfg(unix)]
fn tidy_through_handles(sets: &Path) -> std::io::Result<Tidied> {
    use rustix::fs::FileType;
    let mut tidied = Tidied::default();
    let sets_dir = match at::folder(sets) {
        Ok(dir) => dir,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound || at::not_a_folder(&why) => {
            return Ok(tidied);
        }
        Err(why) => return Err(why),
    };
    let Some(_tidying) = try_lock_in(&sets_dir)? else {
        tidied.busy = true;
        return Ok(tidied);
    };
    let now = std::time::SystemTime::now();
    let fresh = |entry: &at::Entry| {
        entry.modified.is_some_and(|modified| {
            now.duration_since(modified)
                .is_ok_and(|age| age < TIDY_GRACE)
        })
    };
    // `None` where there are no notes at all; `Err` where something not a folder is in their
    // place, which keeps every set's folder.
    let held_dir = match at::folder_in(&sets_dir, HELD) {
        Ok(dir) => Ok(Some(dir)),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(why),
    };
    let notes_of = |digest: &str| match &held_dir {
        Err(_) => Notes::Kept,
        Ok(None) => Notes::None,
        Ok(Some(held)) => match at::folder_in(held, digest) {
            Ok(notes) => still_held(&notes),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Notes::None,
            Err(_) => Notes::Kept,
        },
    };
    for entry in at::entries(&sets_dir)? {
        let name = entry.name.clone();
        let digest = is_digest(&name);
        if entry.kind != FileType::Directory || !(digest || leftover(&name)) || fresh(&entry) {
            continue;
        }
        if digest && notes_of(&name) == Notes::Kept {
            continue;
        }
        match at::remove_tree_in(&sets_dir, &name) {
            Ok(()) => {
                if digest {
                    if let Ok(Some(held)) = &held_dir {
                        let _ = at::remove_empty_folder_in(held, &name);
                    }
                    tidied.removed.push(name);
                }
            }
            Err(why) => tidied.failed.push((name, why.to_string())),
        }
    }
    // Notes of a folder that has gone, of programs that have gone too.
    if let Ok(Some(held)) = &held_dir {
        for entry in at::entries(held)? {
            let gone = at::entry_in(&sets_dir, &entry.name)?.is_none();
            if entry.kind == FileType::Directory
                && is_digest(&entry.name)
                && gone
                && notes_of(&entry.name) == Notes::None
            {
                let _ = at::remove_empty_folder_in(held, &entry.name);
            }
        }
    }
    Ok(tidied)
}

/// **What the folder of notes `notes` (one set's folder in [`HELD`], opened as a folder itself)
/// says**: [`Notes::Kept`] where a file in it is named by the id of a process that runs and
/// that had started by the time the file was written ([`crate::process::could_be_recorded`]).
/// Every other file named by a process id is removed, through the folder's handle. Anything
/// not named by one is never removed, only said in the log. A process whose start cannot be
/// read is taken as the one noted.
#[cfg(unix)]
fn still_held(notes: &std::os::fd::OwnedFd) -> Notes {
    use rustix::fs::FileType;
    let Ok(entries) = at::entries(notes) else {
        return Notes::Kept;
    };
    let mut held = Notes::None;
    for entry in entries {
        let Ok(pid) = entry.name.parse::<u32>() else {
            tracing::warn!(
                "purlis: {:?} among a skills folder's notes is not one purlis wrote, so it was \
                 left as it is",
                entry.name
            );
            continue;
        };
        if entry.kind != FileType::RegularFile {
            continue;
        }
        let runs = crate::process::alive(pid)
            && match (started_at(pid), entry.modified) {
                (Some(started), Some(written)) => {
                    crate::process::could_be_recorded(started, written)
                }
                _ => true,
            };
        if runs {
            held = Notes::Kept;
        } else {
            let _ = at::remove_file_in(notes, &entry.name);
        }
    }
    held
}

#[cfg(unix)]
pub(crate) mod at;
mod handout;

#[cfg(unix)]
fn started_at(pid: u32) -> Option<std::time::SystemTime> {
    purlis_same_user::Parents::started_at(pid)
}

/// **[`tidy`] of this user's sets' folders, said in the log**, on a thread of its own after
/// `wait`, so nothing waits on it: what the app does as it starts and after a chat closes.
///
/// A fenced test build tidies nothing outside its fence, as it reads no store there
/// ([`studio_on_here`]).
pub fn tidy_here_later(wait: std::time::Duration) {
    if let Some(sets) = sets_here() {
        tidy_later(sets, wait);
    }
}

/// This user's sets' folder ([`crate::hoststart::skill_sets_here`]), or `None` where there is
/// none, or where a fenced test build would reach outside its fence.
fn sets_here() -> Option<PathBuf> {
    crate::hoststart::skill_sets_here()
        .filter(|sets| !crate::fence::FENCED || crate::fence::inside(sets, &crate::fence::fence()))
}

/// **[`tidy`] of `sets`, said in the log**, on a thread of its own after `wait`, so nothing
/// waits on it.
pub fn tidy_later(sets: PathBuf, wait: std::time::Duration) {
    let _ = std::thread::Builder::new()
        .name("purlis-skill-sets-tidy".into())
        .spawn(move || {
            std::thread::sleep(wait);
            said_in_the_log(&sets, tidy(&sets));
        });
}

/// **A host's event log that also tidies the sets' folders after each program ends** (SK-24
/// follow-up, #2879): what `purlis serve` keeps its programs' ends with. A host with no window
/// has no chat close to tidy after, so it tidies [`TIDY_AFTER_A_CLOSE`] after each end its host
/// hears, as the window does after a chat closes, the ends a host that crashed left included.
/// Ends that come together share one tidy: while one waits to run, another end asks for none.
/// Everything else is `record`'s own.
#[cfg(unix)]
pub struct TidiesAfterEachEnd<R> {
    record: R,
    sets: Option<PathBuf>,
    wait: std::time::Duration,
    /// A tidy waits to run.
    waiting: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(unix)]
impl<R> TidiesAfterEachEnd<R> {
    /// `record`, tidying this user's sets' folders, as [`tidy_here_later`] does.
    pub fn here(record: R) -> Self {
        Self::of(record, sets_here(), TIDY_AFTER_A_CLOSE)
    }

    /// `record`, tidying `sets` `wait` after each end; nothing where there are none.
    pub fn of(record: R, sets: Option<PathBuf>, wait: std::time::Duration) -> Self {
        Self {
            record,
            sets,
            wait,
            waiting: std::sync::Arc::default(),
        }
    }

    /// A tidy of the sets `wait` from now, unless one waits already.
    fn tidy_soon(&self) {
        use std::sync::atomic::Ordering;
        let Some(sets) = self.sets.clone() else {
            return;
        };
        if self.waiting.swap(true, Ordering::AcqRel) {
            return;
        }
        let (waiting, wait) = (self.waiting.clone(), self.wait);
        let spawned = std::thread::Builder::new()
            .name("purlis-skill-sets-tidy".into())
            .spawn(move || {
                std::thread::sleep(wait);
                // Cleared before the tidy reads, so an end heard while it runs is tidied after.
                waiting.store(false, Ordering::Release);
                said_in_the_log(&sets, tidy(&sets));
            });
        if spawned.is_err() {
            self.waiting.store(false, Ordering::Release);
        }
    }
}

#[cfg(unix)]
impl<R: purlis_host::Record> purlis_host::Record for TidiesAfterEachEnd<R> {
    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        self.record.record(entry)
    }

    fn ended(&self, ended: &purlis_host::Ended<'_>) {
        self.record.ended(ended);
        self.tidy_soon();
    }

    fn left_by_its_last_host(&self, left: &purlis_host::Started) {
        self.record.left_by_its_last_host(left);
        self.tidy_soon();
    }

    fn log_end(&self) -> Result<(String, u64), String> {
        self.record.log_end()
    }

    fn read_log(&self, after: u64) -> Result<Box<dyn purlis_host::ReadsLog>, String> {
        self.record.read_log(after)
    }
}

/// How long after a chat closes its sets are tidied ([`tidy_here_later`]): time for its program
/// to be ended and its note to go, so the folder it used goes in the same tidy.
pub const TIDY_AFTER_A_CLOSE: std::time::Duration = std::time::Duration::from_secs(5);

/// What a [`tidy`] of `sets` did, in the log.
pub fn said_in_the_log(sets: &Path, tidied: std::io::Result<Tidied>) {
    match tidied {
        Ok(tidied) => {
            if tidied.busy {
                tracing::debug!(
                    "purlis: a chat was starting, so unused skills folders in {} are looked \
                     for after the next chat closes",
                    sets.display()
                );
            }
            for digest in &tidied.removed {
                tracing::info!(
                    "purlis: removed the skills folder {digest} in {}: no running chat uses it",
                    sets.display()
                );
            }
            for (digest, why) in &tidied.failed {
                tracing::warn!(
                    "purlis: the skills folder {digest} in {} could not be removed ({why}); \
                     it is tried again after the next chat closes",
                    sets.display()
                );
            }
        }
        Err(why) => tracing::warn!(
            "purlis: unused skills folders in {} could not be looked for ({why}); they are \
             looked for again after the next chat closes",
            sets.display()
        ),
    }
}

/// Whether `name` is a set's folder's: a SHA-256, in lowercase hex.
fn is_digest(name: &str) -> bool {
    crate::leftovers::is_lowercase_hex(name, 64)
}

/// Whether `name` is what a lay-out that never finished left: `.<digest>.<suffix>`.
#[cfg_attr(not(unix), allow(dead_code))]
fn leftover(name: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|rest| rest.split_once('.'))
        .is_some_and(|(digest, suffix)| {
            is_digest(digest)
                && !suffix.is_empty()
                && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

/// The sets' lock ([`LOCK`]) in the folder `sets`, opened through its handle and never through
/// a link, taken only where nothing holds it: `None` while a lay-out does.
#[cfg(unix)]
fn try_lock_in(sets: &std::os::fd::OwnedFd) -> std::io::Result<Option<std::fs::File>> {
    let file = at::file_in(sets, LOCK)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(why)) => Err(why),
    }
}

/// The sets' lock ([`LOCK`]) in the folder `sets`, opened through its handle and never through
/// a link, taken shared as a lay-out takes it, until the file is dropped: waiting for a tidy to
/// finish, which never takes long.
#[cfg(unix)]
fn lock_shared_in(sets: &std::os::fd::OwnedFd) -> std::io::Result<std::fs::File> {
    let file = at::file_in(sets, LOCK)?;
    file.lock_shared()?;
    Ok(file)
}

/// [`lock_shared_in`] where there are no handles to open a folder by.
#[cfg(not(unix))]
fn lock_shared(sets: &Path) -> std::io::Result<std::fs::File> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(sets.join(LOCK))?;
    file.lock_shared()?;
    Ok(file)
}

/// The plugin manifest a set's folder holds.
fn manifest_text() -> String {
    serde_json::json!({
        "name": LINKED_PLUGIN,
        "description": "This project's skills, as purlis hands them to this chat",
    })
    .to_string()
}

/// Whether `folder` is the complete folder of `handed`: its manifest, and exactly the folder
/// of each skill as handed ([`handout::holds`]).
fn laid_out(folder: &Path, handed: &[handout::Handed]) -> bool {
    let manifest = std::fs::read_to_string(folder.join(".claude-plugin/plugin.json"));
    manifest.ok().as_deref() == Some(manifest_text().as_str())
        && handout::holds(&folder.join(DIR_IN_BUNDLE), handed)
}

/// Whether `name` can name a link in a set's folder: one plain path component.
fn plain_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(std::path::Component::Normal(_)), None)
    ) && !name.contains(['/', '\\'])
}

/// The briefing section that lists the skills of a chat started with [`LISTED_ENV`], or none
/// where it is unset or the set is empty.
///
/// The set is asked of [`purlis_skills::skill_set`] with what the chat was armed with — the
/// directory, the persona the chat runs as in the plane at `root`, and the briefing route,
/// the only one the variable is set for — so it lists what the chat's set holds, never every
/// folder in the directory.
pub fn briefed(
    root: &Path,
    workspace: Option<&str>,
    cwd: Option<&Path>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let dir = env(LISTED_ENV).filter(|dir| !dir.is_empty())?;
    let persona = crate::envvar::lookup(crate::active::PERSONA_ENV, env).filter(|p| !p.is_empty());
    let handed = handed_folder(env);
    let plane = handed.is_some();
    // The repo skills of the clone the chat works in, as each briefed harness reads them: the
    // briefing lists one copy of a name, the narrower (SK-6, decision 51).
    let repo: Vec<OutsideSkill> = match (plane, workspace, cwd) {
        (true, Some(ws), Some(cwd)) => Harness::ALL
            .iter()
            .filter(|harness| harness.skills() == Route::Briefing)
            .flat_map(|harness| repo_skills_of_chat(*harness, root, ws, cwd))
            .collect(),
        _ => Vec::new(),
    };
    let set = ask(
        Some(Path::new(&dir)),
        plane.then_some(PlaneSkills {
            plane: root,
            workspace,
            repo: &repo,
            routed: None,
        }),
        persona.as_deref(),
        Route::Briefing,
        None,
        // Routed where the folder was laid out: a skill hidden there is not in it.
        None,
    );
    // Only what purlis hands: the harness lists its own and the repo's itself. A plane skill is
    // named by the `SKILL.md` purlis wrote for it, and one it refused is not named at all.
    let skills: Vec<Skill> = set
        .effective
        .into_iter()
        .filter(|e| e.route != Route::Own)
        .filter_map(|e| {
            if !e.origin.of_the_plane() {
                return Some(e.skill);
            }
            let handed = handed.as_ref()?;
            let path = handed
                .join(DIR_IN_BUNDLE)
                .join(&e.skill.name)
                .join(purlis_skills::FILE);
            purlis_skills::read_skill_md(handed, &path).ok()?;
            Some(Skill { path, ..e.skill })
        })
        .collect();
    // A skill a router hid was not laid out, so it is not named above; the index laid out in its
    // place is (SK-15, ADR 0102 §2).
    let mut skills = skills;
    if let Some(handed) = &handed {
        let path = handed
            .join(DIR_IN_BUNDLE)
            .join(INDEX)
            .join(purlis_skills::FILE);
        if purlis_skills::read_skill_md(handed, &path).is_ok() {
            skills.push(Skill {
                name: INDEX.to_owned(),
                description: purlis_skills::router::INDEX_DESCRIPTION.to_owned(),
                path,
            });
        }
    }
    listing(&skills)
}

/// The set's folder a briefed chat was handed its plane skills in ([`PLANE_LISTED_ENV`]), where
/// it names a folder.
fn handed_folder(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    env(PLANE_LISTED_ENV)
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
        .filter(|folder| folder.is_absolute())
}

/// **The skills a briefed chat may start by reading their `SKILL.md`** (SK-3): every skill in
/// the directory it was started with in [`LISTED_ENV`], and the plane's in the set's folder
/// [`PLANE_LISTED_ENV`] names, each by the name of its folder there, as the briefing names it.
/// Empty for a chat that was not briefed: a harness that loads its skills itself starts them by
/// its own tool.
pub fn readable_by_a_briefed_chat(env: &dyn Fn(&str) -> Option<String>) -> Vec<Skill> {
    let Some(dir) = env(LISTED_ENV).filter(|dir| !dir.is_empty()) else {
        return Vec::new();
    };
    let mut skills = read(Path::new(&dir));
    if let Some(handed) = handed_folder(env) {
        skills.extend(read(&handed.join(DIR_IN_BUNDLE)).into_iter().map(|skill| {
            Skill {
                name: skill
                    .path
                    .parent()
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or(skill.name.clone()),
                ..skill
            }
        }));
    }
    skills
}

/// **A running chat, as Use skill… asks for its skills** (SK-12, #2775): where it works, as whom,
/// on which harness, and whether the `skill-studio` flag lets the plane's skills into its set.
#[derive(Debug, Clone, Copy)]
pub struct Running<'a> {
    /// The plane's root.
    pub plane: &'a Path,
    /// The workspace it works in, or `None` at the plane root.
    pub workspace: Option<&'a str>,
    /// The persona it was started as.
    pub persona: Option<&'a str>,
    pub harness: Harness,
    /// The built-in skills' directory, where the app found it.
    pub built_in: Option<&'a Path>,
    /// Whether its set holds the plane's skills: the flag, as at a chat's start.
    pub plane_skills: bool,
    /// Where its plane skills came from, as its start read them once
    /// ([`crate::hoststart::ChatSkills::read`]): its scope (a Try it's own, SK-9) and the repo
    /// skills of its clone, so its set is routed as the one it was handed, with nothing read
    /// again (#3245). `None`, or one that read none (`on` off), for the plane's root and no clone.
    pub start: Option<&'a crate::hoststart::ChatSkills>,
}

impl Running<'_> {
    /// Its set, routed as its start routed it ([`lay_out_for_chat`]): from the same built-in
    /// skills, harness, scope, workspace, persona and repo skills (#3245).
    fn set(&self) -> SkillSet {
        // A start that read no plane skills (`on` off, or a runner's chat, whose host read
        // them) holds no scope of its own: the plane's root, and no clone.
        let start = self.start.filter(|start| start.on);
        let tried = start.is_some_and(|start| start.tried);
        let at = self.plane_skills.then(|| PlaneSkills {
            plane: start.map_or(self.plane, |start| start.root.as_path()),
            workspace: self.workspace.filter(|_| !tried),
            repo: start.map_or(&[], |start| start.repo.as_slice()),
            routed: start.and_then(|start| start.routed.as_ref()),
        });
        routed(
            self.built_in,
            at,
            self.persona.filter(|_| !tried),
            self.harness,
        )
    }
}

/// One skill Use skill… offers a chat: one of its set, with what it declares and the name its
/// harness knows it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub effective: Effective,
    pub declared: Declared,
    /// Whether it asks for anything before it is typed: arguments it names, or a hint at what to
    /// pass. The one place this is decided; the window draws the boxes by it.
    pub takes_arguments: bool,
    /// The name the chat's harness knows it by, which a run of it records
    /// ([`Harness::skill_name`]).
    pub known_as: String,
    /// Why a router hid it from the chat's listing (SK-15), where it did: its harness was not
    /// handed it, so Use skill… asks the chat to load it with `skill_load` instead.
    pub hidden: Option<Hidden>,
}

/// **What Use skill… offers a chat, and its skill budget** (SK-12): every skill of its set, the
/// manual-only ones among them, by name, and what the set spends of its harness's budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSkills {
    /// The chat's harness, which the skills are typed in the syntax of.
    pub harness: Harness,
    /// The workspace the chat works in, `None` at the plane root: where a run it starts is.
    pub workspace: Option<String>,
    pub offered: Vec<Offered>,
    pub spent: Spent,
    /// What is still over the budget once the router hid all it may (ADR 0102 §4): the pins, or
    /// what no pin decides. The budget then says which.
    pub still_over: Option<StillOver>,
    /// Which of the chat's workspace and persona pin a skill (SK-15d): where unpinning is done
    /// when the pins alone are past the budget.
    pub pinned_by: Vec<PinnedBy>,
    /// Where the chat reads none of its pins because its workspace's and its persona's together
    /// pass the most it reads (#3391): both pinners, and how many each holds.
    pub pins_together: Option<PinsTogether>,
}

/// **The skills of chat `chat`, as Use skill… offers them** (SK-12): exactly its set, worked out
/// as its start worked it out (the plane's skills only where the flag let them in, as
/// [`of_a_chat`] works them out), each with what it declares, and the budget the set spends on
/// its harness.
pub fn for_use_skill(chat: &Running<'_>) -> ChatSkills {
    // Routed as its start routed it, so a skill its harness was not handed is marked.
    let set = chat.set();
    let still_over = set.routing.as_ref().and_then(|routing| routing.still_over);
    let hidden = |name: &str| set.hidden_by_router(name).cloned();
    let offered: Vec<Offered> = set
        .effective
        .iter()
        .cloned()
        .map(|effective| {
            let declared = declared(&effective.skill);
            Offered {
                hidden: hidden(&effective.skill.name),
                takes_arguments: !declared.arguments.is_empty() || declared.hint.is_some(),
                known_as: chat
                    .harness
                    .skill_name(&effective.skill.name, effective.origin),
                declared,
                effective,
            }
        })
        .collect();
    let spent = purlis_skills::budget::spent(
        offered
            .iter()
            .map(|one| (&one.effective.skill, &one.declared)),
        chat.harness.skill_budget(),
    );
    ChatSkills {
        harness: chat.harness,
        workspace: chat.workspace.map(str::to_owned),
        offered,
        spent,
        still_over,
        pinned_by: if chat.plane_skills {
            pinners_of_a_chat(chat.plane, chat.workspace, chat.persona)
        } else {
            Vec::new()
        },
        pins_together: chat
            .plane_skills
            .then(|| pins_together(chat.plane, chat.workspace, chat.persona))
            .flatten(),
    }
}

/// What Use skill… types into a chat to start one of its skills ([`ChatSkills::typed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typing {
    /// The text, in the chat's harness's syntax, on one line.
    pub text: String,
    /// The run it starts once its prompt is sent, its id minted now: the name the harness knows
    /// the skill by, which copy it is, and how its start is confirmed
    /// ([`crate::skillspan::Typed`]), or that the load starts it.
    pub start: Start,
}

/// **How the run of a skill Use skill… typed starts.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// Once its prompt is sent: the run is held for it ([`crate::skillspan::Typed`]).
    Held(crate::skillspan::Typed),
    /// When the chat loads it with `skill_load`, which opens its run: a skill a router hid,
    /// whose harness was not handed it (SK-15).
    ByLoad,
}

impl ChatSkills {
    /// **The text that starts skill `name` of this chat's set**, with `values` for its arguments
    /// in the order it declares them ([`Given::of`]), and the run it starts, its text digested
    /// under the chat's `salt` ([`crate::skillspan::PromptSalt`]), or why there is none: a skill
    /// that is not in the chat's set is never typed.
    pub fn typed(
        &self,
        name: &str,
        values: &[String],
        salt: &crate::skillspan::PromptSalt,
    ) -> Result<Typing, String> {
        let offered = self
            .offered
            .iter()
            .find(|one| one.effective.skill.name == name)
            .ok_or_else(|| {
                format!("{name} is not one of this chat's skills, so nothing was typed")
            })?;
        let given = Given::of(&offered.declared, values);
        if offered.hidden.is_some() {
            return Ok(Typing {
                text: load_text(&offered.effective.skill.name, &given),
                start: Start::ByLoad,
            });
        }
        let text = self
            .harness
            .skill_start_text(&crate::harness::SkillToStart {
                name: &offered.effective.skill.name,
                origin: offered.effective.origin,
                given: &given,
            });
        let held = Start::Held(crate::skillspan::Typed {
            skill: offered.known_as.clone(),
            run: ulid::Ulid::generate(),
            workspace: self.workspace.clone(),
            level: Some(runs::Level {
                origin: offered.effective.origin,
                local: offered.effective.local,
            }),
            text: crate::skillspan::PromptMarks::digest(&text, salt),
            confirmed: self.harness.typed_start(),
        });
        Ok(Typing { text, start: held })
    }
}

/// **What Use skill… types to start `name`, a skill a router hid** (SK-15, ADR 0102 §7): its
/// harness was not handed it, so the chat is asked, in words, to load it with purlis's
/// `skill_load` tool, with `given` in plain words. The same on every harness.
fn load_text(name: &str, given: &Given) -> String {
    match given.in_words() {
        None => format!("Load the skill {name} with skill_load and follow it."),
        Some(words) => {
            format!("Load the skill {name} with skill_load and follow it, with {words}")
        }
    }
}

/// **The stamp of the plane skills a running chat has now** (#2770). The window takes one as a chat
/// starts and again when the project's skills change; where they differ, the chat's harness does
/// not take the change in by itself ([`Harness::skills_reload`]) and its tab offers a restart.
/// An empty stamp where the plane's skills do not reach it (#3445): a chat they did not reach
/// at its start was handed none of them.
///
/// Routed as the chat's start routed it ([`Running`]), so what its router hides, and so its
/// index, is what the chat was handed: the built-in skills count against its budget (SK-15), and
/// so do the repo skills of its clone (#3245). Whether the plane's skills reach it is
/// [`Running::plane_skills`], the flag as read once at its start, never read again here.
pub fn stamp_now(chat: &Running<'_>) -> Stamp {
    if !chat.plane_skills {
        return Stamp::default();
    }
    stamp_of(chat)
}

/// [`stamp_now`] whatever the flag says: for a caller that asked the flag once for many chats.
pub fn stamp_of(chat: &Running<'_>) -> Stamp {
    Stamp::of(&chat.set(), chat.harness.skills_reload())
}

/// **The set of a chat on `harness` as its listing is routed** (SK-15): one ask, from the
/// built-in skills in `built_in`, the plane's at `at` where they reach it, and its persona, by
/// its harness's route and within its budget. The one routing the folder a chat is handed
/// ([`lay_out_for_chat`]), what its adapter arms it with ([`for_chat`]), Use skill…
/// ([`for_use_skill`]) and its restart's stamp ([`stamp_of`]) are worked out by, so they agree
/// (#3245).
fn routed(
    built_in: Option<&Path>,
    at: Option<PlaneSkills<'_>>,
    persona: Option<&str>,
    harness: Harness,
) -> SkillSet {
    ask(
        built_in,
        at,
        persona,
        harness.skills(),
        None,
        Some(harness.skill_budget()),
    )
}

/// **The set of a chat on `harness` before it is routed, and its pins** (SK-15b): what its
/// router extension is asked about at its start ([`router::at_start`]), from the same skills
/// [`routed`] routes.
pub(crate) fn unrouted(
    built_in: Option<&Path>,
    at: PlaneSkills<'_>,
    persona: Option<&str>,
    harness: Harness,
) -> (SkillSet, Vec<String>) {
    let set = ask(built_in, Some(at), persona, harness.skills(), None, None);
    (set, pins_or_none(at.plane, at.workspace, persona))
}

/// One ask of purlis-skills, from a directory of built-in skills, the plane with the workspace
/// the chat works in (where the plane's skills are let in), the persona a chat runs as (whose
/// `skills:` list is read where the plane's skills are let in), the chat's route, the harness
/// skills on this machine where they were looked for, and the chat's skill budget where its
/// listing is routed (SK-15): by the built-in router, with the pins of its workspace and its
/// persona ([`pins_or_none`]).
fn ask(
    built_in: Option<&Path>,
    plane: Option<PlaneSkills<'_>>,
    persona: Option<&str>,
    route: Route,
    harness: Option<&[OnMachine]>,
    budget: Option<Budget>,
) -> SkillSet {
    let kept = |folder: &Path| match plane {
        Some(at) => crate::profiles::kept_local(at.plane, folder),
        None => Err("there is no plane to ask git about".to_owned()),
    };
    let off = plane
        .and_then(|at| Some(turned_off(at.plane, at.workspace?)))
        .unwrap_or_default();
    let names = plane
        .zip(persona)
        .map(|(at, persona)| names_of(at.plane, persona))
        .unwrap_or_default();
    // A workspace or persona folder a running sandboxed chat could have made (SK-9).
    let made = plane.map(|at| levels::made_since(at.plane));
    let set = purlis_skills::skill_set(
        plane.map(|at| at.plane),
        &Machine {
            built_in,
            kept_local: Some(&kept),
            harness,
            made_since: made
                .as_deref()
                .map(|made| made as &purlis_skills::MadeSince<'_>),
        },
        &Chat {
            workspace: plane.and_then(|at| at.workspace),
            persona,
            names: &names,
            turned_off: &off,
            harness: HarnessSkills { route },
        },
    );
    let set = match plane.map(|at| at.repo) {
        Some(repo) if !repo.is_empty() => set.with_outside(route, repo.iter().cloned()),
        _ => set,
    };
    match budget {
        Some(budget) => {
            let pins = plane
                .map(|at| pins_or_none(at.plane, at.workspace, persona))
                .unwrap_or_default();
            match plane.and_then(|at| at.routed) {
                Some(routed) => routed.apply(set, budget, &pins),
                None => set.routed(budget, &pins),
            }
        }
        None => set,
    }
}

/// **What the persona `persona`'s `skills:` list names** (SK-5, #2768), in the plane at `plane`:
/// each entry it writes, as a [`Reference`], in order. An entry that names nothing is dropped.
pub fn names_of(plane: &Path, persona: &str) -> Vec<Reference> {
    crate::personaverbs::declared_skills(plane, persona)
        .iter()
        .filter_map(|written| Reference::parse(written))
        .collect()
}

/// **The skills a chat starts without, as its tab's Notice says them** (SK-5, #2768; SK-8,
/// #2771, decision 41: never a silent drop): the ones its persona names that are not here, each
/// as the persona's `skills:` list writes it, and the ones that could not be loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotHere {
    /// A harness skill the chat's harness does not have on this machine.
    pub missing: Vec<String>,
    /// A built-in skill purlis has none of: a typo, or a name from another version of purlis.
    pub no_built_in: Vec<String>,
    /// A plane skill the chat would have loaded that could not be: not a whole skill, or one
    /// of two folders at its level that name it.
    pub broken: Vec<Broken>,
    /// A plane skill the repo the chat works in leaves out (SK-53, #3213): a narrower copy
    /// there takes its name, or purlis could not read a skills folder of the clone whole
    /// ([`purlis_skills::SkillSet::left_out_by_a_repo`]).
    pub by_a_repo: Vec<Broken>,
    /// A skill of the repo the chat works in named like one of purlis's built-in skills (SK-37,
    /// #3027; [`purlis_skills::SkillSet::beside_a_built_in`]): the chat's harness loads it beside
    /// purlis's, and the chat may use either. By name, once each.
    pub beside_a_built_in: Vec<String>,
}

/// A skill a chat would have loaded that could not be loaded, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    /// Its name, or its folder's where it names none.
    pub name: String,
    /// Its level, and whether it is a local skill.
    pub origin: Origin,
    pub local: bool,
    pub why: BrokenWhy,
}

/// Why a skill a chat would have loaded could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokenWhy {
    /// The set leaves it out: not a whole skill, or a twin at its level.
    LeftOut(Reason),
    /// purlis hands it to no chat ([`purlis_skills::Refusal`]).
    Refused(purlis_skills::Refusal),
    /// The repo the chat works in leaves it out ([`Reason::of_a_repo`], SK-53): said as the
    /// chat's own tab says it ([`Reason::to_its_chat`], #3234).
    ByARepo(Reason),
}

impl std::fmt::Display for BrokenWhy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LeftOut(reason) => write!(f, "{reason}"),
            Self::Refused(why) => write!(f, "purlis hands it to no chat: {why}"),
            Self::ByARepo(reason) => f.write_str(&reason.to_its_chat()),
        }
    }
}

/// **The skills a chat as `persona` (or on none) starts without** (SK-5, #2768; SK-8), in the
/// plane at `plane`, at its root or in `workspace`, started in `cwd`, on `harness` with the
/// chat's environment `env` and the built-in skills in `built_in`: [`not_here_beside`], with the
/// repo skills of the clone `cwd` is in read as a start reads them ([`repo_skills_of_chat`]).
/// The host, which has read them already to arm the chat, hands them on instead
/// ([`crate::hoststart::skills_not_here`], #3234).
pub fn not_here(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    built_in: Option<&Path>,
    harness: crate::harness::Harness,
    env: &crate::harness_plugin::Env<'_>,
    cwd: Option<&Path>,
) -> Option<NotHere> {
    let repo = match (workspace, cwd) {
        (Some(ws), Some(cwd)) => repo_skills_of_chat(harness, plane, ws, cwd),
        _ => Vec::new(),
    };
    not_here_beside(plane, workspace, persona, built_in, harness, env, &repo)
}

/// [`not_here`], beside the repo skills `repo` of the clone the chat works in, as its start read
/// them. A broken plane skill is one only where the chat would have loaded it: a project skill
/// whose grant does not reach the chat's workspace is no loss. A plane skill that repo leaves
/// out is said too (SK-53), and a repo skill named like a built-in one (SK-37). `None` where it
/// has every one, or purlis could not tell.
pub fn not_here_beside(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    built_in: Option<&Path>,
    harness: crate::harness::Harness,
    env: &crate::harness_plugin::Env<'_>,
    repo: &[OutsideSkill],
) -> Option<NotHere> {
    let here = on_machine(harness, env);
    let at = PlaneSkills {
        plane,
        workspace,
        repo,
        routed: None,
    };
    let set = ask(
        built_in,
        Some(at),
        persona,
        harness.skills(),
        Some(&here),
        None,
    );
    let reaches = |left: &LeftOut| {
        left.origin != Origin::Project
            || workspace.is_none_or(|ws| grant_of(&left.folder).is_ok_and(|g| g.reaches(ws)))
    };
    let mut not = NotHere::default();
    let honours = harness.skill_frontmatter();
    for effective in set.from_the_plane() {
        if let Err(why) = handout::of(effective, &honours) {
            not.broken.push(Broken {
                name: effective.skill.name.clone(),
                origin: effective.origin,
                local: effective.local,
                why: BrokenWhy::Refused(why),
            });
        }
    }
    not.by_a_repo = set
        .left_out_by_a_repo()
        .map(|left| Broken {
            name: left.name.clone(),
            origin: left.origin,
            local: left.local,
            why: BrokenWhy::ByARepo(left.reason.clone()),
        })
        .collect();
    for clash in set.beside_a_built_in() {
        if !not.beside_a_built_in.contains(&clash.name) {
            not.beside_a_built_in.push(clash.name.clone());
        }
    }
    for left in set.left_out {
        match left.reason {
            Reason::NotOnThisMachine => not.missing.push(left.name),
            Reason::NoSuchBuiltIn => not.no_built_in.push(left.name),
            Reason::NotASkill(_) | Reason::Twin { .. } if reaches(&left) => {
                not.broken.push(Broken {
                    name: left.name,
                    origin: left.origin,
                    local: left.local,
                    why: BrokenWhy::LeftOut(left.reason),
                });
            }
            _ => {}
        }
    }
    (not != NotHere::default()).then_some(not)
}

/// **The project skills the workspace `workspace` turned off** (#2767): `settings.skills.off`,
/// as the Workspace settings layer reads its `workspace.json`. Empty where it names none, or
/// holds something other than a list of names, which that layer's save refuses to write.
pub fn turned_off(plane: &Path, workspace: &str) -> Vec<String> {
    names_in_settings(plane, workspace, crate::settings::workspace::SKILLS_OFF)
}

/// The list of names `settings.skills.<key>` of the workspace `workspace` holds, or none.
pub(super) fn names_in_settings(plane: &Path, workspace: &str, key: &str) -> Vec<String> {
    use crate::settings::workspace::SKILLS;
    crate::settings::workspace::read(plane, workspace)
        .and_then(|table| {
            let names = table.get(SKILLS)?.get(key)?.as_array()?;
            names
                .iter()
                .map(|name| name.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .unwrap_or_default()
}

/// The folder of the project skill named `name` in the plane at `plane`: in its `skills/`, else
/// in its `skills.local/`, or `None` where neither holds one.
pub fn project_folder(plane: &Path, name: &str) -> Option<PathBuf> {
    [false, true]
        .into_iter()
        .find_map(|local| named_at(plane, &Whose::Project, local, name))
}

/// **`purlis skill grant <name> <grant>`** (#2767): writes `grant` as where the project skill
/// `name` reaches, and says its folder. Refused, writing nothing, on a project purlis opens
/// read-only, for a name no project skill has, and for a workspace the plane does not have, so a
/// typo never grants a name nobody has.
pub fn grant(plane: &Path, name: &str, grant: &Grant) -> Result<PathBuf, String> {
    crate::settings::writable(plane)?;
    if let Grant::Only(names) = grant {
        let held = crate::workspaces::Plane::open(plane);
        for ws in names {
            if !held.workspace(ws).is_ok_and(|one| one.dir().is_dir()) {
                return Err(format!(
                    "this project has no workspace {ws:?}, so nothing was granted: \
                     `purlis workspace list` lists them"
                ));
            }
        }
    }
    let folder = project_folder(plane, name).ok_or_else(|| no_project_skill(name))?;
    set_grant(&folder, grant)?;
    Ok(folder)
}

/// **Turns the project skill `name` off (`on` false) or back on in `workspace`** (#2767): its
/// `settings.skills.off`, through the Workspace level's own save. A workspace can only take
/// away what a grant gave, so a skill whose grant does not reach `workspace` is refused either
/// way, saying the grant that would. `Ok(false)` where it already was so, and nothing is written.
pub fn switch(plane: &Path, workspace: &str, name: &str, on: bool) -> Result<bool, String> {
    let held = crate::workspaces::Plane::open(plane);
    if !held
        .workspace(workspace)
        .is_ok_and(|one| one.dir().is_dir())
    {
        return Err(format!(
            "this project has no workspace {workspace:?}: `purlis workspace list` lists them"
        ));
    }
    let folder = project_folder(plane, name).ok_or_else(|| no_project_skill(name))?;
    let grant = grant_of(&folder).unwrap_or(Grant::Root);
    if !grant.reaches(workspace) {
        return Err(not_granted(name, workspace, &grant, on));
    }
    let mut off = turned_off(plane, workspace);
    let was = off.iter().any(|one| one == name);
    if was != on {
        return Ok(false);
    }
    if on {
        off.retain(|one| one != name);
    } else {
        off.push(name.to_owned());
    }
    set_turned_off(plane, workspace, off)?;
    Ok(true)
}

/// **Why the project skill `name`, granted `grant`, cannot be turned on (`on`) or off in
/// `workspace`**, which the grant does not reach: the grant that would. What `purlis skill grant
/// --on` and the Skill studio's switch both say (#2767, #2770).
pub fn not_granted(name: &str, workspace: &str, grant: &Grant, on: bool) -> String {
    let what = if on {
        "cannot be turned on there: a workspace only turns off what the project granted it"
    } else {
        "has nothing to turn off there"
    };
    format!(
        "{name} is not granted to {workspace}, so it {what}. Its grant is {grant}: a grant to \
         {workspace}, or to all, would reach it"
    )
}

/// Writes `off` as the project skills `workspace` turned off, through the Workspace level's own
/// save: none removes the list.
pub(crate) fn set_turned_off(
    plane: &Path,
    workspace: &str,
    off: Vec<String>,
) -> Result<(), String> {
    set_names_in_settings(
        plane,
        workspace,
        crate::settings::workspace::SKILLS_OFF,
        off,
    )
}

/// Writes `names` as `settings.skills.<key>` of the workspace `workspace`, through the Workspace
/// level's own save, or takes the key out for none.
pub(super) fn set_names_in_settings(
    plane: &Path,
    workspace: &str,
    key: &str,
    names: Vec<String>,
) -> Result<(), String> {
    let value = (!names.is_empty()).then_some(crate::settings::Value::List(names));
    set_in_settings(plane, workspace, key, value)
}

/// The workspace `workspace`'s `settings.skills.<key>` set to `value`, or taken out for none,
/// through the Workspace level's own save.
fn set_in_settings(
    plane: &Path,
    workspace: &str,
    key: &str,
    value: Option<crate::settings::Value>,
) -> Result<(), String> {
    let read = crate::settings::workspace::read_file(plane, workspace)?;
    let base = read.exists.then_some(read.text.as_str());
    let path = vec![
        crate::settings::Step::Key(crate::settings::workspace::SKILLS.to_owned()),
        crate::settings::Step::Key(key.to_owned()),
    ];
    crate::settings::workspace::save(
        plane,
        workspace,
        base,
        &[crate::settings::Edit { path, value }],
    )
    .map(|_| ())
    .map_err(|why| why.join(" "))
}

/// **Writes the workspace `workspace`'s `settings.skills.<key>`** as the text `text`, or takes
/// the key out for none, through the Workspace level's own save (#3692).
pub(super) fn set_text_in_settings(
    plane: &Path,
    workspace: &str,
    key: &str,
    text: Option<String>,
) -> Result<(), String> {
    set_in_settings(
        plane,
        workspace,
        key,
        text.map(crate::settings::Value::Text),
    )
}

fn no_project_skill(name: &str) -> String {
    format!(
        "no project skill is named {name:?}: a project skill is a folder in skills/ or \
         skills.local/ at the project's root, and `purlis skill list` lists them"
    )
}

/// **Share** (#2767): moves the local skill `name` of the level `at` (the plane root's, a
/// workspace's or a persona's) into that level's shared `skills/`, so the plane's save commits
/// it. Refused on a project purlis opens read-only, whose committed files it does not write.
pub fn share_here(plane: &Path, at: &Whose, name: &str) -> Result<Shared, String> {
    crate::settings::writable(plane)?;
    share(plane, at, name)
}

/// The `requires` feature of a project whose skills the `project-skills` fix moved from
/// `.claude/skills` into `skills/` (#2784): a build that has it hands every chat the project's
/// skills by their route, and copies no `.claude/skills` into a clone
/// ([`crate::doctor::fix::project_skills`]).
pub const PROJECT_SKILLS_FEATURE: &str = "project-skills";

/// **Whether the project's skills reach the chats of the project at `plane`** (ADR 0099 as
/// amended 2026-10-22, the dispatcher's ruling on #2784): where `studio`, the Skill studio's
/// flag, is on, and on every machine once the project requires [`PROJECT_SKILLS_FEATURE`]. The
/// flag shows the studio; the manifest says where the project's skills live. A project whose
/// skills moved out of `.claude/skills` gets no copy of it in a clone, so a teammate with the
/// flag off would otherwise start every chat without them.
pub fn reach_chats(plane: &Path, studio: bool) -> bool {
    studio || crate::compat::requires(plane, PROJECT_SKILLS_FEATURE)
}

/// [`reach_chats`], with this machine's flag ([`studio_on_here`]).
pub fn reach_chats_here(plane: &Path) -> bool {
    reach_chats(plane, studio_on_here())
}

/// The flag the Skill studio's slices land behind until its finish line (#2762, SK-23), and is
/// then removed. It hides nothing yet.
pub const STUDIO_FLAG: &str = "skill-studio";

/// The variable that turns [`STUDIO_FLAG`] on (`1`) or off (`0`), over what this machine says.
/// Any other value is not read.
pub const STUDIO_ENV: &str = "PURLIS_SKILL_STUDIO";

/// Whether [`STUDIO_FLAG`] is on: what this machine's store says
/// ([`crate::machine::Flags::skill_studio`], off unless turned on, on every channel), and
/// [`STUDIO_ENV`] decides over it.
pub fn studio_on(flags: &crate::machine::Flags, env: &dyn Fn(&str) -> Option<String>) -> bool {
    match crate::envvar::lookup(STUDIO_ENV, env).as_deref() {
        Some("1") => true,
        Some("0") => false,
        _ => flags.skill_studio,
    }
}

/// **What the Skill studio and `purlis skill list` show** (SK-2): the skill set of a chat in the
/// plane at `plane`, at its root (`workspace` `None`) or in `workspace`, as `persona` or on none
/// (SK-5), with the built-in skills from `built_in`. Every skill with its level, and every one
/// left out with its reason. Read as a Claude Code chat's: the route is not what they show. A
/// harness skill a persona names is looked for in every harness the project's profiles run
/// ([`harnesses_used`]): there where any has it, unchecked where none has it and one cannot list
/// its plugin's skills, and left out only where none has it.
pub fn in_the_studio(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> SkillSet {
    in_the_studio_on(built_in, plane, workspace, persona, Route::Plugin)
}

/// [`in_the_studio`] on `route`: what a harness on a route that names built-in skills their own
/// ([`Route::names_built_ins_its_own`]) is shown its clashes against, each built-in skill by the
/// name it knows it by (SK-37).
fn in_the_studio_on(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    route: Route,
) -> SkillSet {
    let env = crate::harness_plugin::Env::of(&[]);
    studio_set(built_in, plane, workspace, persona, route, Some(&env))
}

/// [`in_the_studio_on`], with the harness skills looked for under `env`, or taken as there for
/// `None` (nowhere to look): what `persona lint` asks with the home it was given (#2934).
pub(crate) fn studio_set(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    route: Route,
    env: Option<&crate::harness_plugin::Env<'_>>,
) -> SkillSet {
    let at = PlaneSkills {
        plane,
        workspace,
        repo: &[],
        routed: None,
    };
    let here = persona.zip(env).map(|(_, env)| {
        harnesses_used(plane)
            .into_iter()
            .flat_map(|harness| on_machine(harness, env))
            .collect::<Vec<_>>()
    });
    // Not routed here: each harness's own set is, by its own budget ([`studio_list`]).
    ask(built_in, Some(at), persona, route, here.as_deref(), None)
}

/// **Every harness the project's profiles run**, in the registry's order, or Claude Code alone
/// for a project that offers none on this machine: what the studio checks a persona's harness
/// skills against.
pub fn harnesses_used(plane: &Path) -> Vec<crate::harness::Harness> {
    let kinds: Vec<String> = crate::personaprofile::offers_shown(plane)
        .into_iter()
        .map(|offer| offer.kind)
        .collect();
    harnesses_of(&kinds)
}

/// The harnesses profiles of `kinds` run, in the registry's order, or Claude Code alone for none.
fn harnesses_of(kinds: &[String]) -> Vec<crate::harness::Harness> {
    let used: Vec<crate::harness::Harness> = crate::harness::Harness::ALL
        .iter()
        .copied()
        .filter(|harness| {
            kinds
                .iter()
                .any(|kind| crate::harness::Harness::of_kind(kind) == Some(*harness))
        })
        .collect();
    if used.is_empty() {
        vec![crate::harness::Harness::ClaudeCode]
    } else {
        used
    }
}

/// **The route a chat's skills take, by its harness** as the chat names it in
/// [`crate::hookwire::HARNESS_ENV`] (ADR 0063 as amended 2026-10-20): a harness purlis ships
/// takes its own ([`Harness::skills`], a chat that names none being Claude Code's,
/// [`Harness::of_chat`]); a harness of `declarations` whose declaration (ADR 0073) says it takes
/// MCP servers ([`crate::harness_declaration::MCP_SERVERS`]) and which purlis has no adapter
/// for, so no skill format purlis can hand skills by, takes [`Route::Mcp`]. Any other has none.
pub fn route_of(
    harness: Option<&str>,
    declarations: &crate::harness_declaration::Declarations,
) -> Option<Route> {
    if let Some(shipped) = Harness::of_chat(harness) {
        return Some(shipped.skills());
    }
    declarations
        .get(harness?)
        .filter(|declared| {
            declared
                .capability(crate::harness_declaration::MCP_SERVERS)
                .holds()
        })
        .map(|_| Route::Mcp)
}

/// **Which copy of the skill a chat started as `skill`, a name as its harness gave it, is of**,
/// and the workspace the chat works in (SK-13): what a run's start carries, so a chat is shown
/// the runs of its own copies alone. The copy is the one the chat's set holds by that name
/// ([`purlis_skills::by_name`]), worked out on the route of the chat's `harness` (Claude Code's
/// where it names none), with the built-in skills purlis ships beside this binary; `None` for a
/// name the set does not hold, such as a harness's own skill.
pub fn whose(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    harness: Option<Harness>,
    skill: &str,
) -> runs::Whose {
    whose_with(
        built_in_here().as_deref(),
        plane,
        workspace,
        persona,
        harness,
        skill,
    )
}

/// [`whose`], with the built-in skills in `built_in`.
fn whose_with(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    harness: Option<Harness>,
    skill: &str,
) -> runs::Whose {
    // A skill started in purlis's own plugin (`purlis:secrets`) is the built-in copy, whether
    // or not this binary has the bundle beside it: a plane skill never takes a built-in name.
    let built_in_by_name = skill
        .split_once(':')
        .is_some_and(|(plugin, _)| crate::names::PLUGIN_NAME.recognises(plugin));
    let level = if built_in_by_name {
        Some(runs::Level {
            origin: Origin::BuiltIn,
            local: false,
        })
    } else {
        // On the chat's own route: on opencode's the set holds a built-in skill as
        // `purlis-<name>` and leaves a plane skill of that name out, so the name is purlis's
        // there and nowhere else (a plane or clone skill named so elsewhere is its own).
        let route = harness.map_or(Route::Plugin, Harness::skills);
        let set = of_a_chat(built_in, plane, workspace, persona, route);
        purlis_skills::by_name(&set, skill).map(|effective| runs::Level {
            origin: effective.origin,
            local: effective.local,
        })
    };
    runs::Whose {
        workspace: workspace.map(str::to_owned),
        level,
    }
}

/// **The skill set of one chat, as purlis's MCP server's skill tools work it out** (SK-13): a
/// chat in the plane at `plane`, at its root (`workspace` `None`) or in `workspace`, running as
/// `persona`, its skills taking `route`, with the built-in skills from `built_in`. The plane's
/// skills are in it: the tools are there only while the `skill-studio` flag lets the plane's
/// skills into a chat ([`studio_on`]). Worked out at each call, as the chat's place is.
pub fn of_a_chat(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    route: Route,
) -> SkillSet {
    let at = PlaneSkills {
        plane,
        workspace,
        repo: &[],
        routed: None,
    };
    // The harness's own installed skills are the harness's to hand over, never these tools'.
    // Not routed: a skill a router hides is the chat's all the same (ADR 0102 §7).
    ask(built_in, Some(at), persona, route, None, None)
}

/// **One harness in the Skill studio** (SK-6, #2769): the skills from outside the plane it finds
/// itself, beside the plane's. Its plugins are its own to turn on or off (#3467): a plugin's
/// skills are listed read-only, under the plugin's name ([`purlis_skills::Owner::Plugin`]).
pub struct HarnessInTheStudio {
    pub harness: Harness,
    /// What a chat on it gets: the plane's skills ([`in_the_studio`]) with the ones from outside
    /// the plane ([`SkillSet::with_outside`]), each clash settled as this harness's route lets
    /// purlis settle it.
    pub set: SkillSet,
    /// The skills from outside the plane, with whose each is and how it switches.
    pub outside: Vec<OutsideSkill>,
    /// Their studio rows (`outside_rows`): what the window and the command line list under
    /// the harness (SK-34).
    pub rows: Vec<StudioRow>,
    /// The studio rows of the plane's skills a repo of the workspace leaves out of a chat on it
    /// in that clone, each with why ([`purlis_skills::SkillSet::left_out_by_a_repo`], SK-53):
    /// the plane's own rows are a chat's in no clone, so this is where the loss is said.
    pub repo_left_out: Vec<StudioRow>,
    /// **Its own catalogue, measured** ([`own_catalogue`], #3324): `None` where the list was read
    /// only for a count of needs ([`Reading::Needs`]), which shows no figure.
    pub own: Option<OwnCatalogue>,
}

/// **What a harness's own catalogue holds** (#3324; ADR 0102 as amended 2026-10-31): the skills
/// it finds itself in the person's own folders and in its plugins, which it lists to its model
/// against its own limit (Codex keeps about 2% of the context for them). purlis's skill budget
/// counts only what purlis hands a chat (its built-in skills, the plane's, and the repo skills of
/// the clone it works in), and purlis never trims what it does not own, so this is a figure of
/// its own beside that budget, and counts against nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OwnCatalogue {
    /// The skills it lists: each turned on for the project's chats, and not manual-only.
    pub skills: usize,
    /// Characters of what it lists of them, measured as purlis measures a chat's set on this
    /// harness ([`purlis_skills::budget::spent`]).
    pub chars: usize,
    /// Its manual-only skills, which it keeps out of the listing, each as it reads manual-only:
    /// by the frontmatter key where it reads that ([`ManualOnly::Read`]), else by a file of its
    /// own beside the `SKILL.md` ([`kept_manual_beside`]).
    pub manual: usize,
}

impl OwnCatalogue {
    /// **What a chat's budget line says of it** on the harness the person calls `title`, beside
    /// purlis's own figure: `Codex's own, not counted: 2 skills, 1,310 characters, 1 manual-only
    /// skill`. The window and `purlis skill list` say it in these words. `None` for a harness
    /// that finds none of its own.
    pub fn said(&self, title: &str) -> Option<String> {
        use crate::counts::{counted, plural};
        let manual = (self.manual > 0)
            .then(|| counted(self.manual, "manual-only skill", "manual-only skills"));
        let listed = (self.skills > 0).then(|| {
            format!(
                "{}, {} {}",
                counted(self.skills, "skill", "skills"),
                grouped(self.chars),
                plural(self.chars, "character", "characters")
            )
        });
        let parts: Vec<String> = listed.into_iter().chain(manual).collect();
        (!parts.is_empty()).then(|| format!("{title}'s own, not counted: {}", parts.join(", ")))
    }
}

/// `n` with its thousands set apart by commas, as the window writes a count (`1,310`).
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// **`harness`'s own catalogue** ([`OwnCatalogue`]) of `outside`, the skills from outside the
/// plane it finds: the person's own and its plugins' ([`Origin::Harness`]), each one turned off
/// for the project's chats, each one purlis could not read, and each of a plugin of purlis's own
/// left out. A repo's skills are not
/// in it: purlis's budget counts those already, as the chat's set holds them.
fn own_catalogue(harness: Harness, outside: &[OutsideSkill]) -> OwnCatalogue {
    let reads_the_key = harness.skill_frontmatter().manual == ManualOnly::Read;
    let listed: Vec<(&Skill, Declared)> = outside
        .iter()
        .filter(|one| one.origin == Origin::Harness && !one.off && one.unread.is_none())
        // purlis's own plugin's skills are its built-in ones, and a plugin it pins off is never
        // in a chat: neither is the harness's own.
        .filter(|one| match &one.owner {
            Owner::Plugin(name) => {
                !crate::harness_plugin::purlis_s_own_named(harness.adapter().plugins(), name)
            }
            Owner::You | Owner::Repo(_) => true,
        })
        .map(|one| {
            let mut said = declared(&one.skill);
            said.manual = (reads_the_key && said.manual)
                || kept_manual_beside(harness, (&one.root, &one.skill.path), one.origin);
            (&one.skill, said)
        })
        .collect();
    let spent = purlis_skills::budget::spent(
        listed.iter().map(|(skill, said)| (*skill, said)),
        harness.skill_budget(),
    );
    OwnCatalogue {
        skills: spent.skills,
        chars: spent.chars,
        manual: spent.manual,
    }
}

impl HarnessInTheStudio {
    /// **The clashes of a row this harness's chats in a repo start without**
    /// ([`Self::repo_left_out`], #3461): the repo's copy that took the plane copy's name. What
    /// `purlis skill list --json` and `show --json` put on that row, as they put each clash on
    /// the row of the copy purlis hands ([`StudioList::clashes_of`]), so a reader of either has
    /// whose copy won without reading the row's left-out line. None for a row left out because
    /// a clone could not be read whole.
    pub fn clashes_of_left_out<'a>(
        &'a self,
        row: &'a StudioRow,
    ) -> impl Iterator<Item = &'a Clash> + 'a {
        self.set.clashes.iter().filter(move |clash| {
            clash.outcome == Outcome::RepoWins
                && clash.name == row.name
                && clash.plane.origin == row.origin
                && clash.plane.local == row.local
        })
    }
}

/// **Every harness in the Skill studio** (SK-6, #2769), for a chat in the plane at `plane`, at
/// its root or in `workspace` (whose clones' own skills are listed), whose plane skills are
/// `set` ([`in_the_studio`]), or `set_on` a harness's own route where it names built-in skills
/// their own, for a persona whose `skills:` list says `names`, and with `env` read
/// for where each harness keeps its own: one per harness purlis knows, in the registry's order.
/// Read from the harnesses' own files, never written.
#[allow(clippy::too_many_arguments)]
fn harnesses_in_the_studio(
    set: &SkillSet,
    set_on: &dyn Fn(Route) -> SkillSet,
    plane: &Path,
    workspace: Option<&str>,
    names: &[Reference],
    pins: &[String],
    env: &crate::harness_plugin::Env<'_>,
    known: &Known,
    reading: Reading,
) -> Vec<HarnessInTheStudio> {
    let choices = crate::harness_plugin::Choices::read_in(plane, workspace);
    let clones = workspace
        .and_then(|ws| crate::repos::clones(plane, ws).ok())
        .map(|clones| clones.repos)
        .unwrap_or_default();
    crate::harness_plugin::adapters()
        .filter_map(|adapter| {
            // Every adapter is a harness's ([`crate::harness_plugin::adapters`]).
            let harness = Harness::of_kind(adapter.harness())?;
            let mut outside = crate::harness_plugin::harness_skills(adapter, env, &choices);
            outside.extend(crate::harness_plugin::repo_skills(
                adapter, &choices, &clones,
            ));
            // Each built-in skill by the name this harness knows it by (`purlis-<name>` on
            // opencode's route), so a copy of that name is the clash, and a bare one is not.
            let route = harness.skills();
            let set = if route.names_built_ins_its_own() {
                set_on(route)
            } else {
                set.clone()
            };
            let set = set.with_outside(route, outside.clone());
            let by_a_repo = SkillSet {
                left_out: set.left_out_by_a_repo().cloned().collect(),
                ..SkillSet::default()
            };
            let mut one = HarnessInTheStudio {
                harness,
                repo_left_out: rows_of(&by_a_repo, plane, names, reading),
                // Routed by this harness's own budget, with what it finds itself counted (SK-15).
                set: set.routed(harness.skill_budget(), pins),
                own: (reading == Reading::Whole).then(|| own_catalogue(harness, &outside)),
                outside,
                rows: Vec::new(),
            };
            one.rows = outside_rows(&one, plane, known, reading);
            Some(one)
        })
        .collect()
}

/// **The repo skills the harness of a chat started in `cwd` reads** (SK-6, #2769): those of the
/// clone `cwd` is in ([`clones_of_chat`]), in `workspace` of the plane at `plane`, each turned off
/// where the project's choices say so. Nothing for a chat in no clone. The host's half: it reads
/// the clone, and an adapter is handed the answer ([`PlaneSkills::repo`]).
pub fn repo_skills_of_chat(
    harness: Harness,
    plane: &Path,
    workspace: &str,
    cwd: &Path,
) -> Vec<OutsideSkill> {
    let mine = clones_of_chat(plane, workspace, cwd);
    if mine.is_empty() {
        return Vec::new();
    }
    let choices = crate::harness_plugin::Choices::read_in(plane, Some(workspace));
    crate::harness_plugin::repo_skills(harness.adapter().plugins(), &choices, &mine)
}

/// **The clones of `workspace` in the plane at `plane` a chat started in `cwd` works in**: each
/// whose folder `cwd` is in, whose repo skills its harness reads ([`repo_skills_of_chat`]), and
/// what the Skills view narrows a chat's repo skills to (#3462). None for a chat in no clone.
pub fn clones_of_chat(plane: &Path, workspace: &str, cwd: &Path) -> Vec<crate::repos::Repo> {
    let Ok(clones) = crate::repos::clones(plane, workspace) else {
        return Vec::new();
    };
    let here = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    clones
        .repos
        .into_iter()
        .filter(|repo| {
            let at = repo
                .path
                .canonicalize()
                .unwrap_or_else(|_| repo.path.clone());
            here.starts_with(at)
        })
        .collect()
}

/// One skill as the Skill studio and `purlis skill list` and `show` show it: every copy of a name,
/// the one a chat gets and each left out with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioRow {
    /// Its name, or its folder's where it names none.
    pub name: String,
    pub origin: Origin,
    /// Whether it is a local skill, kept on this machine in its level's `skills.local/` (#2767).
    pub local: bool,
    /// Where a project skill reaches (`root`, `all`, or the workspaces' names), or `None` for a
    /// skill of another level. A grant that cannot be read reaches the root alone, and says so
    /// in the left-out line of a workspace's chat.
    pub grant: Option<String>,
    /// Why a chat does not get it, in one line, or `None` where it does.
    pub left_out: Option<String>,
    /// What it does: its `description`, empty for a copy left out.
    pub description: String,
    /// Its folder, relative to the plane where it is inside it; empty for a harness skill.
    pub folder: String,
    /// How a persona's `skills:` list names it (SK-5): `purlis:<name>` for a built-in skill,
    /// `<plugin>:<name>` for a plugin's, else its name.
    pub written: String,
    /// Whether the persona the set is a chat as names it in its `skills:` list.
    pub named: bool,
    /// A harness skill the persona names that purlis could not check this machine for: its
    /// plugin's skills cannot be listed.
    pub unchecked: bool,
    /// For a skill from outside the plane (SK-6, #2769): whose it is, and how its harness can
    /// leave it out. `None` for the plane's and the built-in skills.
    pub outside: Option<OutsideRow>,
    /// For a skill left out because a chat could have made its level's folder (SK-9): that
    /// folder, relative to the plane, which the person may accept ([`levels::accept`]).
    pub accept: Option<String>,
    /// Its compatibility badge on each harness that would load it (SK-8, #2771): every harness
    /// for a skill a chat gets from purlis, its own harness for one from outside the plane, and
    /// none for a skill left out.
    pub compat: Vec<HarnessCompat>,
    /// What the built-in lints say of a skill a chat gets, as anywhere but the editor says it
    /// (SK-8): that it widens the chat, a skill it names there is none of, a program it calls
    /// that this machine lacks, a script it names that its folder lacks. Filled for a skill from
    /// outside the plane, and by [`studio_list`] for the plane's own too (#3780). Empty for the
    /// built-in skills, which nobody here edits, and for a copy left out.
    pub lint: Vec<String>,
    /// Its lifecycle state (SK-19, #2782): in use, deprecated or archived. Always in use for a
    /// skill that is not the plane's, and for one whose file beside it cannot be read.
    pub lifecycle: Lifecycle,
    /// Who looks after it, informational (SK-19). Empty for a skill that is not the plane's.
    pub owners: Vec<String>,
    /// Each harness whose chat a router hid it from the listing of, and why (SK-15, ADR 0102
    /// §7), each by that harness's own budget: the chat still has it, and loads it with
    /// `skill_load`. Empty where every harness lists it.
    pub hidden: Vec<HiddenOn>,
    /// Whether the chat's workspace or persona pins it, so no router hides it (ADR 0102 §4).
    pub pinned: bool,
}

/// **A skill a router hid on one harness's chats** (SK-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenOn {
    pub harness: Harness,
    pub hidden: Hidden,
}

/// What a studio row says of a skill from outside the plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutsideRow {
    pub owner: Owner,
    pub switch: Switch,
}

/// **The studio's rows of the skills from outside the plane `harness` finds** (SK-6, #2769),
/// each by the name the harness knows it by, on or left out as its set says, with whose it is
/// and how it switches. In the order they were found.
fn outside_rows(
    harness: &HarnessInTheStudio,
    plane: &Path,
    known: &Known,
    reading: Reading,
) -> Vec<StudioRow> {
    harness
        .outside
        .iter()
        .map(|found| {
            let folder = found.skill.path.parent().unwrap_or(&found.skill.path);
            let left_out = harness
                .set
                .left_out
                .iter()
                .find(|l| l.name == found.id && l.origin == found.origin && l.folder == folder)
                .map(|l| l.reason.to_string());
            // One purlis did not read is never linted or badged (SK-44): its description says why.
            let (compat, lint) = if left_out.is_none() && found.unread.is_none() {
                (
                    if reading == Reading::Whole {
                        badges(
                            (&found.root, &found.skill.path),
                            found.origin,
                            &[harness.harness],
                        )
                    } else {
                        Vec::new()
                    },
                    outside_lint(known, found, harness.harness),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            StudioRow {
                name: found.id.clone(),
                origin: found.origin,
                local: false,
                grant: None,
                left_out,
                description: found.skill.description.clone(),
                folder: folder
                    .strip_prefix(plane)
                    .unwrap_or(folder)
                    .to_string_lossy()
                    .into_owned(),
                written: found.id.clone(),
                named: false,
                unchecked: false,
                outside: Some(OutsideRow {
                    owner: found.owner.clone(),
                    switch: found.switch.clone(),
                }),
                accept: None,
                compat,
                lint,
                lifecycle: Lifecycle::Active,
                owners: Vec::new(),
                hidden: Vec::new(),
                pinned: false,
            }
        })
        .collect()
}

/// **What the built-in lints check a skill against** (SK-44, #3133): the names of the plane's
/// skills, every level's, and of the built-in ones. Read once per list, never once per skill
/// linted: a clone's skills cost one read of the plane, whatever their number.
struct Known {
    plane: std::collections::HashSet<String>,
    built_in: std::collections::HashSet<String>,
}

impl Known {
    /// What the plane at `plane` and the built-in skills in `built_in`, the bundle's `skills/`,
    /// have: the same built-in skills the set beside it is worked out with.
    fn of(plane: &Path, built_in: Option<&Path>) -> Self {
        // Every level, each persona's among them (SK-32).
        Self {
            plane: verbs::Level::every(plane)
                .iter()
                .filter_map(|level| level.dir(plane))
                .flat_map(|dir| read(&dir))
                .map(|skill| skill.name)
                .collect(),
            built_in: built_in_names(built_in).into_iter().collect(),
        }
    }
}

/// **The live lint of the skill `name` at `level` of the plane at `plane`, as its `SKILL.md`
/// would say `text`** (SK-8, #2771): every finding of the built-in lints
/// ([`purlis_skills::BuiltInLints`]), checked against the plane's skills, the built-in ones and
/// this machine's programs, with the skill's other files read where it is already there. The one
/// lint the editor and the command line show; a save waits only on a finding that
/// [`LintLevel::Blocks`].
pub fn lint(plane: &Path, level: &verbs::Level, name: &str, text: &str) -> Vec<Finding> {
    lint_with(plane, level, name, text, &[])
}

/// [`lint`], and then what each of `more` finds: the lints of the extensions that declare
/// `skill-lint` ([`studio::Studio::lints`]), asked through the interface purlis's own answers by
/// (ADR 0101 §1). Theirs follow purlis's, and only warn.
pub fn lint_with(
    plane: &Path,
    level: &verbs::Level,
    name: &str,
    text: &str,
    more: &[&dyn purlis_skills::SkillLint],
) -> Vec<Finding> {
    let path = verbs::text_of(plane, level, name)
        .ok()
        .map(|(path, _)| path);
    let known = Known::of(plane, built_in_here().as_deref());
    linted(
        &known,
        &known.built_in,
        path.as_deref().map(|path| (plane, path)),
        name,
        text,
        level.origin(),
        None,
        more,
    )
}

/// **What the built-in lints say of a skill from outside the plane** (SK-8): a harness or repo
/// skill, read where it lies by the one reader that follows no link, each finding as anywhere
/// but the editor says it ([`Finding::shown`]), on `harness`, which knows purlis's own skills by
/// its route's names ([`Route::built_in_names`]). What its studio row shows.
fn outside_lint(known: &Known, skill: &OutsideSkill, harness: Harness) -> Vec<String> {
    let known_as = harness
        .skills()
        .built_in_names(known.built_in.iter().map(String::as_str));
    let findings =
        match purlis_skills::read_skill_md_of(&skill.root, &skill.skill.path, skill.origin) {
            Ok(text) => linted(
                known,
                &known_as,
                Some((&skill.root, &skill.skill.path)),
                &skill.skill.name,
                &text,
                skill.origin,
                match &skill.owner {
                    Owner::Plugin(plugin) => Some(plugin.as_str()),
                    Owner::You | Owner::Repo(_) => None,
                },
                &[],
            ),
            Err(why) => vec![Finding {
                severity: purlis_skills::Severity::Warns,
                lint: purlis_skills::Lint::Refused(why),
            }],
        };
    findings.iter().filter_map(Finding::shown).collect()
}

/// The built-in lints of the skill `name` from `origin`, whose `SKILL.md` says `text` and, where
/// it is already there, lies at `skill_md`: checked against the `known` skills and this
/// machine's programs, with its other files read, each of at most [`purlis_skills::MOST_BYTES`].
/// `skill_md` is the root its folder is held from, and the path. `plugin` is the plugin a
/// harness skill comes in ([`purlis_skills::SkillFiles::plugin`]), and `known_as` the names its
/// program knows purlis's own skills by ([`purlis_skills::BuiltInLints::known_as`]).
#[allow(clippy::too_many_arguments)]
fn linted(
    known: &Known,
    known_as: &std::collections::HashSet<String>,
    skill_md: Option<(&Path, &Path)>,
    name: &str,
    text: &str,
    origin: Origin,
    plugin: Option<&str>,
    more: &[&dyn purlis_skills::SkillLint],
) -> Vec<Finding> {
    use purlis_skills::SkillLint;
    // Read through the skill's folder held from its root, never through a link (SK-36).
    let folder =
        skill_md.and_then(|(root, path)| purlis_skills::SkillFolder::open(root, path).ok());
    // Every file of it by its path inside it, `/` between its parts, text or not (#3780).
    let listed: Option<Vec<(std::path::PathBuf, String)>> = folder.as_ref().map(|folder| {
        folder
            .entries()
            .into_iter()
            .filter_map(|entry| match entry {
                purlis_skills::Entry::File(rel) => {
                    let said = rel
                        .components()
                        .map(|part| part.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/");
                    Some((rel, said))
                }
                purlis_skills::Entry::Folder(_) => None,
            })
            .collect()
    });
    let others: Vec<(String, String)> = folder
        .as_ref()
        .zip(listed.as_ref())
        .map(|(folder, listed)| {
            listed
                .iter()
                .filter_map(|(rel, said)| {
                    let text = folder.read(rel, purlis_skills::MOST_BYTES)?;
                    Some((said.clone(), text))
                })
                .collect()
        })
        .unwrap_or_default();
    // Only the plane's own skill is checked for the scripts it names: a chat is handed its folder,
    // so its `scripts/` is that folder's. A repo's or a harness's skill may mean the repo's own.
    let listed: Option<Vec<String>> = listed
        .filter(|_| origin.of_the_plane())
        .map(|listed| listed.into_iter().map(|(_, said)| said).collect());
    let files = purlis_skills::SkillFiles {
        name,
        text,
        others: &others,
        origin,
        plugin,
        listed: listed.as_deref(),
    };
    let mut findings = purlis_skills::BuiltInLints {
        namespace: LINKED_PLUGIN,
        plane: &known.plane,
        built_in: &known.built_in,
        known_as,
        has: &crate::programs::on_path,
    }
    .findings(&files);
    // The listing stops at a link and at its depth and number of files: a script past them that
    // is there after all is no finding.
    if let Some(dir) =
        skill_md.and_then(|(root, path)| root.join(path).parent().map(Path::to_owned))
    {
        findings.retain(|one| match &one.lint {
            purlis_skills::Lint::NoSuchScript(said) => dir.join(said).symlink_metadata().is_err(),
            _ => true,
        });
    }
    for lint in more {
        findings.extend(lint.findings(&files));
    }
    findings
}

/// **A skill's compatibility badge on one harness** (SK-8, #2771): what the studio draws for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessCompat {
    pub harness: Harness,
    pub compat: Compat,
}

/// **A file a harness reads beside a `SKILL.md` to keep a skill it finds itself out of its
/// model's choice** (SK-39), as its adapter says ([`Harness::manual_beside`]).
#[derive(Debug, Clone, Copy)]
pub struct ManualBeside {
    /// Its path inside the skill's folder.
    pub file: &'static str,
    /// Whether a text of it says the skill is manual-only, failing closed: `false` for anything
    /// the harness might not read so.
    pub says: fn(&str) -> bool,
}

/// The badges of the skill from `origin` whose `SKILL.md` is at `path` on each of `harnesses`
/// ([`purlis_skills::compat`]), read by the one reader that follows no link: one that cannot be
/// read is refused, or, from outside the plane, may widen the chat. A skill the harness finds
/// itself is manual-only where the file its harness reads beside it says so
/// ([`kept_manual_beside`]).
fn badges(
    (root, path): (&Path, &Path),
    origin: Origin,
    harnesses: &[Harness],
) -> Vec<HarnessCompat> {
    let text = purlis_skills::read_skill_md_of(root, path, origin);
    harnesses
        .iter()
        .map(|harness| HarnessCompat {
            harness: *harness,
            compat: match &text {
                Ok(text) => {
                    let badge = compat(text, origin, &harness.skill_frontmatter());
                    if kept_manual_beside(*harness, (root, path), origin) {
                        badge.kept_manual()
                    } else {
                        badge
                    }
                }
                Err(why) => Compat::refused(why.clone(), origin),
            },
        })
        .collect()
}

/// **Whether `harness` keeps the skill from `origin` whose `SKILL.md` is at `path` under `root`
/// out of its model's choice by a file of its own beside it** (SK-39): only a skill it finds
/// itself, never one purlis hands it, which it does not read so. The file is read through the
/// skill's folder held from its root, never through a link, and at most
/// [`purlis_skills::MOST_BYTES`] of it: one that cannot be read so says nothing.
fn kept_manual_beside(harness: Harness, (root, path): (&Path, &Path), origin: Origin) -> bool {
    let Some(beside) = harness.manual_beside() else {
        return false;
    };
    if origin.handed() {
        return false;
    }
    purlis_skills::SkillFolder::open(root, path)
        .ok()
        .and_then(|folder| folder.read(Path::new(beside.file), purlis_skills::MOST_BYTES))
        .is_some_and(|text| (beside.says)(&text))
}

/// **Each studio row of a harness skill a persona names, with its badge and lint on every
/// harness that finds it** (SK-8): the rows [`studio_rows`] draws from a persona's `skills:`
/// list carry no file of their own, so the skill each harness found by that name gives them one.
fn with_harness_badges(rows: &mut [StudioRow], harnesses: &[HarnessInTheStudio], known: &Known) {
    let found: Vec<(Harness, &[OutsideSkill])> = harnesses
        .iter()
        .map(|one| (one.harness, one.outside.as_slice()))
        .collect();
    badge_named(rows, &found, known);
}

/// [`with_harness_badges`], with what each harness found.
fn badge_named(rows: &mut [StudioRow], found: &[(Harness, &[OutsideSkill])], known: &Known) {
    for row in rows
        .iter_mut()
        .filter(|row| row.origin == Origin::Harness && row.outside.is_none())
    {
        for (harness, outside) in found {
            if let Some(skill) = outside
                .iter()
                .find(|skill| skill.id == row.name && skill.unread.is_none())
            {
                row.compat.extend(badges(
                    (&skill.root, &skill.skill.path),
                    skill.origin,
                    &[*harness],
                ));
                if row.lint.is_empty() {
                    row.lint = outside_lint(known, skill, *harness);
                }
            }
        }
    }
}

/// **Everything the Skill studio lists for one chat** (SK-34, #3008): the rows of the plane's
/// and the built-in skills (`studio_rows`, each badged on the harnesses that find a copy of
/// it), and each harness with the skills it finds itself and their rows
/// (`harnesses_in_the_studio`). What the window and `purlis skill list` both list, so the two
/// never disagree (spec #2762 decision 26).
pub struct StudioList {
    pub rows: Vec<StudioRow>,
    pub harnesses: Vec<HarnessInTheStudio>,
    /// The harnesses the project or this machine declares that take no skills at all (SK-30),
    /// each with its one line ([`not_supported`]).
    pub not_supported: Vec<NotSupported>,
}

/// **A declared harness skills are not supported on** (SK-30, #2985; decision 18: nothing is
/// silently half-applied): one purlis ships no adapter for, so it has no skill format purlis can
/// hand skills by, and whose declaration says it takes no MCP servers, so the fallback route
/// ([`Route::Mcp`]) cannot reach it either. Its chats get no skills, and the studio, its badge
/// and the chat say so in [`Self::said`]'s words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotSupported {
    /// The word a profile's `kind` names it by.
    pub name: String,
    /// What the person calls it.
    pub title: String,
    /// The one line: why its chats get no skills.
    pub said: String,
}

/// [`NotSupported`] of `declared`, or `None` where its chats can get skills: a harness purlis
/// ships, or one that takes MCP servers, which the fallback route reaches, as [`route_of`] says.
pub fn not_supported_on(
    declared: &crate::harness_declaration::Declaration,
) -> Option<NotSupported> {
    if Harness::of_kind(&declared.name).is_some() || declared.replaces().is_some() {
        return None;
    }
    if declared
        .capability(crate::harness_declaration::MCP_SERVERS)
        .holds()
    {
        return None;
    }
    Some(NotSupported {
        name: declared.name.clone(),
        title: declared.title.clone(),
        said: format!(
            "Skills are not supported on {}: it has no skill format purlis can hand skills \
             by, and its declaration does not say it takes MCP servers, so its chats get no \
             skills.",
            declared.title
        ),
    })
}

/// **Every declared harness skills are not supported on** ([`not_supported_on`]), in the order
/// they are declared.
pub fn not_supported(declarations: &crate::harness_declaration::Declarations) -> Vec<NotSupported> {
    declarations.added().filter_map(not_supported_on).collect()
}

impl StudioList {
    /// **The clashes of a plane or built-in row** (SK-46, #3149; decision 51): each skill from
    /// outside the plane with its name, harness by harness, as the studio marks the row of the
    /// copy purlis hands. What `purlis skill list` and the window both say of it, each in
    /// [`clash_said`]'s words. A row left out (a twin at the same level) is no copy purlis hands,
    /// so it has none.
    pub fn clashes_of<'a>(
        &'a self,
        row: &'a StudioRow,
    ) -> impl Iterator<Item = (Harness, &'a Clash)> + 'a {
        let handed = row.left_out.is_none();
        self.harnesses
            .iter()
            .filter(move |_| handed)
            .flat_map(move |one| {
                one.set
                    .clashes
                    .iter()
                    .filter(move |clash| {
                        clash.name == row.name
                            && clash.plane.origin == row.origin
                            && clash.plane.local == row.local
                    })
                    .map(move |clash| (one.harness, clash))
            })
    }
}

/// **A clash as the studio and `purlis skill list` say it** (SK-46, #3149; ADR 0099 §3): whose
/// the other copy is, and what a chat on `harness` there gets. One sentence, said on the row of
/// purlis's copy, so the window and the command line never disagree.
pub fn clash_said(harness: Harness, clash: &Clash) -> String {
    let title = harness.title();
    let whose = match &clash.owner {
        Owner::Repo(repo) => format!("the repo {repo}"),
        Owner::You => format!("your own {title} skills"),
        Owner::Plugin(plugin) => format!("{plugin} in {title}"),
    };
    match clash.outcome {
        Outcome::RepoWins => format!(
            "{whose} has a skill of this name too, and {title} chats there get the repo's copy \
             alone"
        ),
        Outcome::EitherOne => format!(
            "{whose} has a skill of this name too, and {title} chats there get both under one \
             name and may use either: rename that one"
        ),
        Outcome::BothLoad => {
            format!("{whose} has a skill of this name too, and a chat there gets both")
        }
    }
}

/// **The studio's list for a chat in the plane at `plane`** (SK-34): at its root or in
/// `workspace`, as `persona` or on none, with the built-in skills in `built_in` and `env` read
/// for where each harness keeps its own. Read only.
pub fn studio_list(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    env: &crate::harness_plugin::Env<'_>,
) -> StudioList {
    studio_list_for(Reading::Whole, built_in, plane, workspace, persona, env)
}

/// **What a studio list is read for** (#3781).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// Everything the studio draws.
    Whole,
    /// Only what Needs you's items are worked out from, for a count of them: every row, with
    /// what is left out and why, and the lints and clashes the items read; no badge, no
    /// router's word and no lint of a harness skill a persona names, which no item reads. Its
    /// items are the whole list's.
    Needs,
}

/// [`studio_list`], read for `reading`.
pub fn studio_list_for(
    reading: Reading,
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    env: &crate::harness_plugin::Env<'_>,
) -> StudioList {
    let set = in_the_studio(built_in, plane, workspace, persona);
    let names = persona
        .map(|persona| names_of(plane, persona))
        .unwrap_or_default();
    let known = Known::of(plane, built_in);
    let pins = pins_or_none(plane, workspace, persona);
    let set_on = |route| in_the_studio_on(built_in, plane, workspace, persona, route);
    let harnesses = harnesses_in_the_studio(
        &set, &set_on, plane, workspace, &names, &pins, env, &known, reading,
    );
    let mut rows = rows_of(&set, plane, &names, reading);
    with_plane_lint(&mut rows, &set, &known);
    if reading == Reading::Whole {
        with_harness_badges(&mut rows, &harnesses, &known);
        with_routing(&mut rows, &harnesses);
    }
    StudioList {
        rows,
        harnesses,
        not_supported: not_supported(&crate::harness_declaration::read(plane)),
    }
}

/// **What the built-in lints say of each of the plane's own skills a chat gets** (#3780), as
/// anywhere but the editor says it: the problems in a skill Needs you holds. Linted once per
/// list, against the [`Known`] the list already made, and never on a chat's open path.
fn with_plane_lint(rows: &mut [StudioRow], set: &SkillSet, known: &Known) {
    for got in set.effective.iter().filter(|got| got.origin.of_the_plane()) {
        let Some(row) = rows.iter_mut().find(|row| {
            row.left_out.is_none()
                && row.origin == got.origin
                && row.local == got.local
                && row.name == got.skill.name
        }) else {
            continue;
        };
        // One that cannot be read is refused, which its badge already says.
        let Ok(text) = purlis_skills::read_skill_md_of(&got.root, &got.skill.path, got.origin)
        else {
            continue;
        };
        row.lint = linted(
            known,
            &known.built_in,
            Some((&got.root, &got.skill.path)),
            &got.skill.name,
            &text,
            got.origin,
            None,
            &[],
        )
        .iter()
        .filter_map(Finding::shown)
        .collect();
    }
}

/// **One persona's own skills, as the Skills view lists them** (#3266, #3462): the rows of its
/// own level, as a chat as it gets them in any workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonaRows {
    pub persona: String,
    pub rows: Vec<StudioRow>,
}

/// **Each persona's own skills** (#3266, the Skills view): the plane's personas that have a
/// folder of their own, in [`verbs::Level::every`]'s order, each with the rows of its own level
/// only. A persona's levels are the narrowest, read before any wider one, so no workspace changes
/// them: each is read alone ([`purlis_skills::persona_set`], #3462), without the project's or a
/// workspace's folders or git's word on their local skills, and without the harnesses' listing
/// [`studio_list`] adds, which the view does not draw for a persona.
pub fn persona_rows(built_in: Option<&Path>, plane: &Path) -> Vec<PersonaRows> {
    persona_rows_with(built_in, plane, &|folder| {
        crate::profiles::kept_local(plane, folder)
    })
}

/// [`persona_rows`], asking `kept` whether git keeps a local skill on this machine.
fn persona_rows_with(
    built_in: Option<&Path>,
    plane: &Path,
    kept: &purlis_skills::KeptLocal<'_>,
) -> Vec<PersonaRows> {
    let mut personas: Vec<String> = Vec::new();
    for level in verbs::Level::every(plane) {
        if let purlis_skills::Whose::Persona(name) = level.at
            && !personas.contains(&name)
        {
            personas.push(name);
        }
    }
    // A persona folder a running sandboxed chat could have made (SK-9), asked once for them all.
    let made = levels::made_since(plane);
    let machine = Machine {
        built_in,
        kept_local: Some(kept),
        harness: None,
        made_since: Some(&*made as &purlis_skills::MadeSince<'_>),
    };
    personas
        .into_iter()
        .map(|persona| {
            let set = purlis_skills::persona_set(plane, &machine, &persona, Route::Plugin);
            let names = names_of(plane, &persona);
            let rows = studio_rows(&set, plane, &names)
                .into_iter()
                .filter(|row| row.origin == Origin::Persona)
                .collect();
            PersonaRows { persona, rows }
        })
        .collect()
}

/// **What each harness's router made of the rows' skills** (SK-15): on a row the chat gets,
/// every harness whose own set, routed by its own budget, hides it, and whether a pin keeps it.
fn with_routing(rows: &mut [StudioRow], harnesses: &[HarnessInTheStudio]) {
    for row in rows.iter_mut().filter(|row| row.left_out.is_none()) {
        row.hidden = harnesses
            .iter()
            .filter_map(|one| {
                one.set.hidden_by_router(&row.name).map(|hidden| HiddenOn {
                    harness: one.harness,
                    hidden: hidden.clone(),
                })
            })
            .collect();
        row.pinned = harnesses.iter().any(|one| one.set.is_pinned(&row.name));
    }
}

/// What is said of a harness skill a persona names that purlis could not check this machine for
/// ([`StudioRow::unchecked`]): its plugin is installed, and its skills cannot be listed.
pub const UNCHECKED: &str =
    "purlis can't check whether this machine has it: its plugin's skills can't be listed";

/// **The studio's rows of `set`**, a set of a chat in the plane at `plane` as a persona whose
/// `skills:` list says `names` (empty for a chat on no persona): every skill it gets, every one
/// its harness loads for the persona, and every one left out, by name, each folder relative to
/// the plane. Of the copies of one name, the one the chat gets comes first, then each copy left
/// out from the widest level to the narrowest, as the set lists them (#2767). The one list the
/// window and the command line draw.
pub(crate) fn studio_rows(set: &SkillSet, plane: &Path, names: &[Reference]) -> Vec<StudioRow> {
    rows_of(set, plane, names, Reading::Whole)
}

/// [`studio_rows`], read for `reading`: badged only for the whole list.
fn rows_of(set: &SkillSet, plane: &Path, names: &[Reference], reading: Reading) -> Vec<StudioRow> {
    let relative = |path: &Path| {
        path.strip_prefix(plane)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    };
    let grant = |origin: Origin, folder: &Path| {
        (origin == Origin::Project).then(|| grant_of(folder).unwrap_or(Grant::Root).to_string())
    };
    let written = |origin: Origin, name: &str| match origin {
        Origin::BuiltIn => Reference::BuiltIn(name.to_owned()).to_string(),
        _ => name.to_owned(),
    };
    let named = |said: &str| names.iter().any(|one| one.to_string() == said);
    let mut rows: Vec<StudioRow> = set
        .effective
        .iter()
        .map(|e| {
            let folder = e.skill.path.parent().unwrap_or(&e.skill.path);
            let written = written(e.origin, &e.skill.name);
            StudioRow {
                name: e.skill.name.clone(),
                origin: e.origin,
                local: e.local,
                grant: grant(e.origin, folder),
                left_out: None,
                description: e.skill.description.clone(),
                folder: relative(folder),
                named: named(&written),
                written,
                unchecked: false,
                outside: None,
                accept: None,
                compat: if reading == Reading::Whole {
                    badges((&e.root, &e.skill.path), e.origin, &Harness::ALL)
                } else {
                    Vec::new()
                },
                lint: Vec::new(),
                lifecycle: e
                    .deprecated
                    .clone()
                    .map_or(Lifecycle::Active, Lifecycle::Deprecated),
                owners: owners(e.origin, folder),
                // Each harness's router, by its own budget ([`with_routing`]).
                hidden: Vec::new(),
                pinned: false,
            }
        })
        .chain(set.from_the_harness.iter().map(|h| {
            let written = h.reference.to_string();
            StudioRow {
                name: written.clone(),
                origin: Origin::Harness,
                local: false,
                grant: None,
                left_out: None,
                description: String::new(),
                folder: String::new(),
                named: true,
                written,
                unchecked: !h.checked,
                outside: None,
                accept: None,
                compat: Vec::new(),
                lint: Vec::new(),
                lifecycle: Lifecycle::Active,
                owners: Vec::new(),
                hidden: Vec::new(),
                pinned: false,
            }
        }))
        .chain(set.left_out.iter().map(|l| {
            // A left-out copy a persona names is the reference itself (`purlis:<name>` for a
            // built-in one there is none of); every other copy is named by its own name.
            let written = if matches!(l.reason, Reason::NoSuchBuiltIn) {
                l.name.clone()
            } else {
                written(l.origin, &l.name)
            };
            StudioRow {
                name: l.name.clone(),
                origin: l.origin,
                local: l.local,
                grant: grant(l.origin, &l.folder),
                left_out: Some(l.reason.to_string()),
                description: String::new(),
                folder: relative(&l.folder),
                named: named(&written),
                written,
                unchecked: false,
                outside: None,
                accept: matches!(l.reason, Reason::MadeWhileAChatRan(_))
                    .then(|| levels::to_accept(plane, &l.folder))
                    .flatten(),
                compat: Vec::new(),
                lint: Vec::new(),
                lifecycle: if matches!(l.reason, Reason::Archived) {
                    Lifecycle::Archived
                } else if l.origin.of_the_plane() {
                    about_of(&l.folder)
                        .map(|about| about.lifecycle)
                        .unwrap_or_default()
                } else {
                    Lifecycle::Active
                },
                owners: owners(l.origin, &l.folder),
                hidden: Vec::new(),
                pinned: false,
            }
        }))
        .collect();
    // A deprecated skill is shown after the rest (decision 62), with every copy of its name.
    let deprecated: std::collections::HashSet<String> = rows
        .iter()
        .filter(|row| row.left_out.is_none() && matches!(row.lifecycle, Lifecycle::Deprecated(_)))
        .map(|row| row.name.clone())
        .collect();
    // Stable: the set already lists each name's left-out copies widest first.
    rows.sort_by(|a, b| {
        deprecated
            .contains(&a.name)
            .cmp(&deprecated.contains(&b.name))
            .then(a.name.cmp(&b.name))
            .then(a.left_out.is_some().cmp(&b.left_out.is_some()))
    });
    rows
}

/// The owners the plane's skill of `origin` in `folder` names, or none for a skill of another
/// level or a file beside it that cannot be read.
fn owners(origin: Origin, folder: &Path) -> Vec<String> {
    if origin.of_the_plane() {
        about_of(folder)
            .map(|about| about.owners)
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// One skill run as the studio lists it: the run, and the skill runs it nests in and holds
/// (SK-19): a skill a chat started while another's run was open is a child span of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StudioRun {
    #[serde(flatten)]
    pub run: crate::skilluse::Run,
    /// The skill whose run this one started inside, where it is a child.
    pub inside: Option<String>,
    /// The skills whose runs started inside this one, each once, in the order they started.
    pub ran: Vec<String>,
}

/// **The studio's runs** (SK-3): every run the plane at `plane`'s skill log holds, newest first,
/// of skill `skill` where one is named (by its name, with or without the namespace a harness
/// used it by), at most `limit`, each with the run it nests in and the runs it holds (SK-19). The
/// one list a skill's Runs panel and `purlis skill runs` draw.
pub fn studio_runs(plane: &Path, skill: Option<&str>, limit: usize) -> Vec<StudioRun> {
    let runs = crate::skilluse::runs(plane);
    let mut skill_of: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut children: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for run in &runs {
        if let Some(id) = &run.id {
            skill_of.insert(id, &run.skill);
        }
    }
    // Oldest first, so a parent's children are in the order they started.
    for run in runs.iter().rev() {
        if let Some(parent) = run.end.as_ref().and_then(|end| end.parent.as_deref()) {
            let held = children.entry(parent).or_default();
            if !held.contains(&run.skill.as_str()) {
                held.push(&run.skill);
            }
        }
    }
    runs.iter()
        .filter(|run| skill.is_none_or(|name| run.is_of(name)))
        .take(limit)
        .map(|run| StudioRun {
            inside: run
                .end
                .as_ref()
                .and_then(|end| end.parent.as_deref())
                .and_then(|parent| skill_of.get(parent))
                .map(|skill| (*skill).to_owned()),
            ran: run
                .id
                .as_deref()
                .and_then(|id| children.get(id))
                .map(|held| held.iter().map(|skill| (*skill).to_owned()).collect())
                .unwrap_or_default(),
            run: run.clone(),
        })
        .collect()
}

/// The built-in skills' directory of the plugin installed beside this binary, where there is one
/// ([`crate::plugin_install::bundle_beside`]): what the Skill studio and `purlis skill` list,
/// found the one way in the window and on the command line.
pub fn built_in_here() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    crate::plugin_install::bundle_beside(&exe).and_then(|plugin| in_bundle(&plugin))
}

/// **Turns the Skill studio's flag on or off on this machine** (`flags.skill-studio` in
/// `machine.json`), as Settings › You › Previews does. [`STUDIO_ENV`] still decides over it
/// where it is set.
pub fn set_studio_on(config_root: &Path, on: bool) -> Result<(), String> {
    crate::machine::update(config_root, |store| store.flags.skill_studio = on)
        .map(|_| ())
        .map_err(|why| format!("purlis could not keep it, so nothing was changed: {why}"))
}

/// [`studio_on`] for this process: this machine's store, and this process's environment. Asked
/// at every chat's start, so it never holds the fence: a fenced test build reads a store outside
/// its fence as holding no flag, as [`crate::profilestore::listed_here`] does.
pub fn studio_on_here() -> bool {
    let flags = crate::machine::config_root_unheld()
        .filter(|config| {
            !crate::fence::FENCED || crate::fence::inside(config, &crate::fence::fence())
        })
        .map(|config| crate::machine::read(&config).store.flags)
        .unwrap_or_default();
    studio_on(&flags, &|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests;
