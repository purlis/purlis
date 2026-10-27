// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Brokered writes (ADR 0067 §2, spec #1330, #1333): the project's own files a chat asks the
//! app to write for it, because its sandbox does not let it write them.
//!
//! A `purlis` command run inside a chat the app started hands one of these to the app over the
//! chat's hook socket ([`crate::hookwire::Ask::Write`]), and the app performs it here, with the
//! same core functions the terminal's command uses. The chat's sandbox does not widen.
//!
//! **A bounded set of typed writes, never "run this command".** Each [`Write`] is one thing a
//! command does, with only what the model writes in it: the text, a title, whether it is
//! shared. **No write names a place.** The workspace and the persona are the app's record of
//! the chat whose token the line carries ([`Asker`]), so a chat writes its own workspace, its
//! own persona's memory and shared memory, and nothing it can name. The one exception is a
//! new workspace ([`Write::WorkspaceCreate`]), which names only one that is not there yet.
//!
//! **Nothing brokered changes what a chat runs under.** A persona's store is held by descriptor,
//! as a workspace's is (V74), and every file a write touches is asked of [`guard`]: never under
//! a name the sandbox denies as later code, never an ignore file, and never a manifest change
//! to `[sandbox]`, `[chat_env]` or `[dispatch]` (the #1341 review; #1439, #1437).

use std::path::{Path, PathBuf};

use crate::active::Place;

/// One write a chat asks the app to make. Serialized on the hook socket.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum Write {
    /// `purlis persona remember [--shared]` and the `persona_remember` tool: one persistent
    /// memory of the chat's persona, or of shared memory.
    PersonaRemember {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        shared: bool,
    },
    /// `purlis workspace remember` (and `note`): one memory of the chat's workspace.
    WorkspaceRemember {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// `purlis workspace todo "<text>"`: one todo in the chat's workspace.
    Todo { text: String },
    /// `purlis workspace vision "<text>"` and the `workspace_vision` tool: the `## Vision` of
    /// the chat's workspace, replaced (#1384).
    WorkspaceVision { text: String },
    /// The `workspace_section` tool: one entry added to a section of the chat's workspace's
    /// `workspace.md`, everything already there kept (#1384).
    WorkspaceSection { section: Section, text: String },
    /// `purlis persona forget` and `purlis workspace forget`: one memory of one of the chat's
    /// own stores deleted, with its index line (#1408).
    Forget { of: Store, slug: String },
    /// `purlis persona edit-memory` and `purlis workspace edit`: one memory rewritten in place,
    /// what is not given kept as the file has it (ADR 0065, #1408).
    EditMemory {
        of: Store,
        slug: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// `purlis persona archive-memory` and `purlis workspace archive`: one memory moved into
    /// its store's `archive/` (ADR 0065, #1408).
    ArchiveMemory { of: Store, slug: String },
    /// `purlis persona unarchive-memory` and `purlis workspace unarchive`: one archived memory
    /// moved back, under `restore_as` when given (ADR 0065, #1408).
    UnarchiveMemory {
        of: Store,
        slug: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restore_as: Option<String>,
    },
    /// `purlis persona move-memory` and `purlis workspace move-memory`: one memory moved from
    /// one of the chat's own stores to another (KN-3, #1408).
    MoveMemory {
        from: Store,
        slug: String,
        to: Store,
    },
    /// `purlis workspace optimize --apply`: the safe curation of the chat's own workspace's
    /// journal (#1408).
    WorkspaceOptimize { stale_days: i64 },
    /// `purlis persona optimize --apply`: the safe curation of the chat's own persona's memory
    /// ([`Store::Persona`]) or of shared memory ([`Store::Shared`]), and of shared memory too
    /// with `with_shared` (`--all`) (#1408).
    PersonaOptimize {
        of: Store,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        with_shared: bool,
        stale_days: i64,
    },
    /// `purlis workspace todo done|forget <slug>`: one todo of the chat's own workspace closed,
    /// after the closes a promote left unfinished (#1408).
    TodoClose {
        how: crate::wscmd::todo::Close,
        slug: String,
    },
    /// `purlis workspace create <name> [--vision]`: a new workspace, with its vision (ADR 0067
    /// §2's "creating a workspace", #1408). **The one write that names a place, and only one
    /// that is not there yet**: a workspace that is, or anything at its path, is refused, so a
    /// chat never reaches one it has through it. Repos to clone go as a git ask that makes the
    /// workspace the same way ([`crate::hookwire::GitWork::Create`], qw387); `--live` and
    /// `--use` are refused in a sandboxed chat before anything is asked.
    WorkspaceCreate {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        vision: Option<String>,
    },
    /// `purlis workspace snapshot [--description] [--force]` (#2046): the chat's own
    /// workspace's repos and branches, recorded in its `workspace.json`, which a chat's sandbox
    /// does not write.
    WorkspaceSnapshot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        force: bool,
    },
}

/// One of the chat's own memory stores (#1408). **Never a name**: which workspace and which
/// persona is the app's record of the chat ([`Asker`]), so a chat reaches its own stores and
/// shared memory, and no other workspace's or persona's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Store {
    /// The journal of the workspace the chat works in.
    Workspace,
    /// The memory of the persona the chat runs as.
    Persona,
    /// The memory every persona reads, `personas/_shared/memory/`.
    Shared,
}

/// What a brokered write did (#1408): one file written, or a command's lines said, as the
/// command run in a terminal would have said them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    /// The file it wrote ([`Write::PersonaRemember`] and the other writes of one file).
    Written(Written),
    /// What the command said, and its exit code: the memory verbs that change a store
    /// ([`Write::Forget`] and those after it). A refusal the command says is one of these too.
    Said {
        lines: Vec<crate::repocmd::Say>,
        code: u8,
    },
}

/// A section of `workspace.md` a chat adds entries to (#1384, spec #1330's "the decisions and
/// glossary sections"). Only these two: `## Vision` is replaced whole by its own write,
/// `## Sessions` is purlis's line, and `## Log` points at the memo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    /// `## Context & decisions`: a decision every later chat should act on.
    Decisions,
    /// `## Glossary`: a term this work coined.
    Glossary,
}

impl Section {
    /// Every section, in the order `workspace.md` has them.
    pub const ALL: [Self; 2] = [Self::Decisions, Self::Glossary];

    /// Its word in a tool's arguments and on the wire.
    pub fn word(self) -> &'static str {
        match self {
            Self::Decisions => "decisions",
            Self::Glossary => "glossary",
        }
    }

    /// The section by its word, or `None` for any other.
    pub fn named(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.word() == word)
    }

    /// Its heading in `workspace.md`, without the `## `.
    pub fn header(self) -> &'static str {
        match self {
            Self::Decisions => "Context & decisions",
            Self::Glossary => "Glossary",
        }
    }
}

/// The longest text a brokered write carries, in bytes. Well below what one line of the hook
/// socket holds even when every byte is a control character JSON spells in six, so a write is
/// always read whole and answered, never cut and dropped (#1333). Longer is refused, saying so.
pub const MOST_TEXT_BYTES: usize = 12 * 1024;

impl Write {
    /// Why this write is refused before anything is looked at: a text too long to carry, or a
    /// title that is not one line.
    pub fn check(&self) -> Result<(), String> {
        let (text, title, more): (&str, Option<&str>, usize) = match self {
            Self::PersonaRemember { text, title, .. } | Self::WorkspaceRemember { text, title } => {
                (text, title.as_deref(), 0)
            }
            Self::Todo { text }
            | Self::WorkspaceVision { text }
            | Self::WorkspaceSection { text, .. } => (text, None, 0),
            Self::EditMemory {
                slug, title, text, ..
            } => (
                text.as_deref().unwrap_or_default(),
                title.as_deref(),
                slug.len(),
            ),
            Self::UnarchiveMemory {
                slug, restore_as, ..
            } => (
                "",
                None,
                slug.len() + restore_as.as_deref().map_or(0, str::len),
            ),
            Self::Forget { slug, .. }
            | Self::ArchiveMemory { slug, .. }
            | Self::MoveMemory { slug, .. }
            | Self::TodoClose { slug, .. } => ("", None, slug.len()),
            Self::WorkspaceOptimize { .. } | Self::PersonaOptimize { .. } => ("", None, 0),
            Self::WorkspaceCreate { name, vision } => {
                (vision.as_deref().unwrap_or_default(), None, name.len())
            }
            Self::WorkspaceSnapshot { description, .. } => {
                (description.as_deref().unwrap_or_default(), None, 0)
            }
        };
        let bytes = text.len() + title.map_or(0, str::len) + more;
        if bytes > MOST_TEXT_BYTES {
            return Err(format!(
                "this is {bytes} bytes, and what purlis writes for a chat holds at most \
                 {MOST_TEXT_BYTES}: make it shorter, or split it in two"
            ));
        }
        if title.is_some_and(|title| title.chars().any(crate::memstore::breaks_a_line)) {
            return Err(
                "a title is one line, with no newline, line or paragraph separator, \
                        direction override or other control character in it"
                    .to_owned(),
            );
        }
        Ok(())
    }

    /// Whether this is a memory verb, answered with the lines its command says ([`Done::Said`])
    /// rather than the one file it wrote.
    pub fn says_lines(&self) -> bool {
        !matches!(
            self,
            Self::PersonaRemember { .. }
                | Self::WorkspaceRemember { .. }
                | Self::Todo { .. }
                | Self::WorkspaceVision { .. }
                | Self::WorkspaceSection { .. }
        )
    }

    /// The write's word in the trace.
    fn word(&self) -> &'static str {
        match self {
            Self::PersonaRemember { .. } => "persona_remember",
            Self::WorkspaceRemember { .. } => "workspace_remember",
            Self::Todo { .. } => "todo",
            Self::WorkspaceVision { .. } => "workspace_vision",
            Self::WorkspaceSection { .. } => "workspace_section",
            Self::Forget { .. } => "forget",
            Self::EditMemory { .. } => "edit_memory",
            Self::ArchiveMemory { .. } => "archive_memory",
            Self::UnarchiveMemory { .. } => "unarchive_memory",
            Self::MoveMemory { .. } => "move_memory",
            Self::WorkspaceOptimize { .. } => "workspace_optimize",
            Self::PersonaOptimize { .. } => "persona_optimize",
            Self::TodoClose { .. } => "todo_close",
            Self::WorkspaceCreate { .. } => "workspace_create",
            Self::WorkspaceSnapshot { .. } => "workspace_snapshot",
        }
    }
}

/// The chat a write is made for, as the app records it — never as the request says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asker {
    /// The app's number for the chat.
    pub chat: u32,
    /// Where it works.
    pub place: Place,
    /// Its persona, if it runs as one.
    pub persona: Option<String>,
}

/// What a write wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The workspace or persona it went to (`_shared` for shared memory).
    pub to: String,
    /// The file, project-relative, with `/` between its parts.
    pub path: String,
}

/// Performs `write`, a write of one file, for `asker` in the project at `root`, or says why not.
/// Nothing is written when it refuses, and a memory verb, which answers in lines, is refused
/// here unmade: [`answer`] makes every write.
pub fn perform(
    root: &Path,
    asker: &Asker,
    write: &Write,
    now: chrono::NaiveDateTime,
) -> Result<Written, String> {
    if write.says_lines() {
        return Err(format!(
            "{} answers in lines, so it is made through brokered::answer: nothing was written",
            write.word()
        ));
    }
    match answer(root, asker, write, now)? {
        Done::Written(written) => Ok(written),
        Done::Said { .. } => Err("nothing was written".to_owned()),
    }
}

/// Makes `write` for `asker` in the project at `root`, or says why not. Nothing is written when
/// it refuses.
///
/// A write of one file answers [`Done::Written`]; a memory verb answers [`Done::Said`], the
/// lines its command says, a refusal it says among them.
pub fn answer(
    root: &Path,
    asker: &Asker,
    write: &Write,
    now: chrono::NaiveDateTime,
) -> Result<Done, String> {
    if let crate::compat::Compat::ReadOnly(why) = crate::compat::read(root) {
        return Err(format!("nothing was written: {why}"));
    }
    write.check()?;
    if let Some(said) = upkeep(root, asker, write, now)? {
        let (to, store) = said.credited;
        if said.changed {
            credit(root, asker, write, &to, &store, now);
        }
        return Ok(Done::Said {
            lines: said.lines,
            code: said.code,
        });
    }
    let written = match write {
        Write::PersonaRemember {
            text,
            title,
            shared,
        } => {
            let owner = if *shared {
                crate::contain::SHARED_PERSONA
            } else {
                asker.persona.as_deref().ok_or_else(|| {
                    format!(
                        "chat {} runs as no persona, so it has no persona memory of its own: \
                         remember it --shared, or in its workspace",
                        asker.chat
                    )
                })?
            };
            let path = remember_persona(root, owner, text, title.as_deref(), now)?;
            Written {
                to: owner.to_owned(),
                path: shown(root, &path),
            }
        }
        Write::WorkspaceVision { text } => {
            let who = format!("chat {}", asker.chat);
            set_vision(root, &asker.place, &who, text)?
        }
        Write::WorkspaceSection { section, text } => {
            let who = format!("chat {}", asker.chat);
            add_to_section(root, &asker.place, &who, *section, text)?
        }
        Write::WorkspaceRemember { text, title } => {
            let ws = own_workspace(root, asker)?;
            guard(root, &ws.dir().join("memory"), None)?;
            let path = ws
                .remember_titled(text, title.as_deref(), now)
                .map_err(|e| e.to_string())?;
            Written {
                to: ws.name().to_owned(),
                path: shown(root, &path),
            }
        }
        Write::Todo { text } => {
            let ws = own_workspace(root, asker)?;
            guard(root, &ws.dir().join("todos"), None)?;
            guard_store(
                root,
                &crate::memscope::Scope::Workspace(ws.name().to_owned()),
            )?;
            // The closes a promote left unfinished come first, as `ws todo` makes them (each
            // journals its close); a written answer carries no lines, so they are not said.
            let plane = crate::workspaces::Plane::open(root);
            crate::wscmd::todo::finish_promotes(&plane, &ws, now, &mut |_| {});
            let path = ws.record_todo(text, now).map_err(|e| e.to_string())?;
            Written {
                to: ws.name().to_owned(),
                path: shown(root, &path),
            }
        }
        // Answered by `upkeep` above; never reached, and refused rather than a panic.
        _ => return Err("this write is not one purlis makes this way".to_owned()),
    };
    credit(root, asker, write, &written.to, &written.path, now);
    Ok(Done::Written(written))
}

/// Credits `write` to the chat that asked, in the trace: what it went `to` and where.
fn credit(
    root: &Path,
    asker: &Asker,
    write: &Write,
    to: &str,
    path: &str,
    now: chrono::NaiveDateTime,
) {
    let chat = asker.chat.to_string();
    crate::trace::record(
        root,
        &chat,
        "brokered",
        &[
            ("chat", chat.as_str()),
            ("write", write.word()),
            ("to", to),
            ("path", path),
        ],
        now,
    );
}

/// What a memory verb said, whether it changed a store, and the store it is credited to (its
/// owner, and its folder, project-relative).
struct Upkept {
    lines: Vec<crate::repocmd::Say>,
    code: u8,
    changed: bool,
    credited: (String, String),
}

/// One of the chat's own stores, as the memory verbs name it: the scope, and the name a
/// sentence gives it (the workspace's, the persona's).
fn own_scope(
    root: &Path,
    asker: &Asker,
    store: Store,
) -> Result<(crate::memscope::Scope, String), String> {
    use crate::memscope::Scope;
    Ok(match store {
        Store::Workspace => {
            let ws = own_workspace(root, asker)?;
            let name = ws.name().to_owned();
            (Scope::Workspace(name.clone()), name)
        }
        Store::Persona => {
            let name = own_persona(asker)?;
            (Scope::Persona(name.clone()), name)
        }
        Store::Shared => (Scope::Shared, crate::personas::SHARED.to_owned()),
    })
}

/// The persona the chat runs as: a chat on none has no persona memory of its own.
fn own_persona(asker: &Asker) -> Result<String, String> {
    asker.persona.clone().ok_or_else(|| {
        format!(
            "chat {} runs as no persona, so it has no persona memory of its own: nothing was \
             written",
            asker.chat
        )
    })
}

/// Asks [`guard`] of a store's folder and of its archive: what a memory verb writes, renames or
/// deletes is in one of the two.
fn guard_store(root: &Path, scope: &crate::memscope::Scope) -> Result<String, String> {
    let dir = root.join(scope.store());
    guard(root, &dir, None)?;
    guard(root, &dir.join("archive"), None)?;
    Ok(scope.store())
}

/// Performs a memory verb ([`Write::Forget`] and those after it) for `asker`, with the same
/// core code the terminal's command runs, collecting what it says; `None` for a write of one
/// file, which [`perform`] makes itself.
fn upkeep(
    root: &Path,
    asker: &Asker,
    write: &Write,
    now: chrono::NaiveDateTime,
) -> Result<Option<Upkept>, String> {
    use crate::memscope::Scope;
    use crate::personaverbs::upkeep as persona;
    let plane = crate::workspaces::Plane::open(root);
    // One list both the command's lines and its changed stores' word go to, in order.
    let lines = std::cell::RefCell::new(Vec::new());
    let mut say = |line| lines.borrow_mut().push(line);
    let saved = || {
        if let Some(line) = crate::wscmd::memory::next_save(root) {
            lines
                .borrow_mut()
                .push(crate::repocmd::Say::Info(line.into()));
        }
    };
    let chat = asker.chat.to_string();
    // A persona verb names the persona (its sentences do); shared memory is named through the
    // chat's own, as `persona forget <own> <slug> --shared` names it.
    let persona_of = |store: Store| -> Result<Option<(String, bool)>, String> {
        Ok(match store {
            Store::Workspace => None,
            Store::Persona => Some((own_persona(asker)?, false)),
            Store::Shared => Some((own_persona(asker)?, true)),
        })
    };
    let (code, changed, credited) = match write {
        Write::Forget { of, slug } => {
            let (scope, owner) = own_scope(root, asker, *of)?;
            let store = guard_store(root, &scope)?;
            let (code, changed) = match persona_of(*of)? {
                None => crate::wscmd::memory::forget(&plane, &owner, slug, &mut say)?,
                Some((name, shared)) => persona::forget(
                    root,
                    &persona::Forget {
                        name: &name,
                        slug,
                        shared,
                        ephemeral: false,
                        session: &chat,
                    },
                    &mut say,
                ),
            };
            (code, changed, (owner, store))
        }
        Write::EditMemory {
            of,
            slug,
            title,
            text,
        } => {
            let (scope, owner) = own_scope(root, asker, *of)?;
            let store = guard_store(root, &scope)?;
            let (code, changed) = match persona_of(*of)? {
                None => crate::wscmd::memory::edit(
                    &plane,
                    &owner,
                    slug,
                    title.as_deref(),
                    text.as_deref(),
                    &mut say,
                )?,
                Some((name, shared)) => persona::edit_memory(
                    root,
                    &persona::Memory {
                        name: &name,
                        slug,
                        shared,
                    },
                    title.as_deref(),
                    text.as_deref(),
                    &mut say,
                ),
            };
            (code, changed, (owner, store))
        }
        Write::ArchiveMemory { of, slug } => {
            let (scope, owner) = own_scope(root, asker, *of)?;
            let store = guard_store(root, &scope)?;
            let (code, changed) = match persona_of(*of)? {
                None => crate::wscmd::memory::archive(&plane, &owner, slug, &mut say)?,
                Some((name, shared)) => persona::archive_memory(
                    root,
                    &persona::Memory {
                        name: &name,
                        slug,
                        shared,
                    },
                    &mut say,
                ),
            };
            (code, changed, (owner, store))
        }
        Write::UnarchiveMemory {
            of,
            slug,
            restore_as,
        } => {
            let (scope, owner) = own_scope(root, asker, *of)?;
            let store = guard_store(root, &scope)?;
            let (code, changed) = match persona_of(*of)? {
                None => crate::wscmd::memory::unarchive(
                    &plane,
                    &owner,
                    slug,
                    restore_as.as_deref(),
                    &mut say,
                )?,
                Some((name, shared)) => persona::unarchive_memory(
                    root,
                    &persona::Memory {
                        name: &name,
                        slug,
                        shared,
                    },
                    restore_as.as_deref(),
                    &mut say,
                ),
            };
            (code, changed, (owner, store))
        }
        Write::MoveMemory { from, slug, to } => {
            let (source, owner) = own_scope(root, asker, *from)?;
            let (target, _) = own_scope(root, asker, *to)?;
            let store = guard_store(root, &source)?;
            guard_store(root, &target)?;
            // The way back from shared memory is named through the chat's own persona.
            let persona = asker.persona.as_deref();
            let (code, changed) = crate::wscmd::memory::move_memory(
                &plane, &source, slug, &target, persona, now, &mut say,
            );
            (code, changed, (owner, store))
        }
        Write::WorkspaceOptimize { stale_days } => {
            let (scope, owner) = own_scope(root, asker, Store::Workspace)?;
            let store = guard_store(root, &scope)?;
            let mut changed = false;
            let code = crate::curate::optimize_workspaces(
                root,
                std::slice::from_ref(&owner),
                true,
                *stale_days,
                now.date(),
                &mut || {
                    changed = true;
                    saved();
                },
                &mut say,
            );
            (code, changed, (owner, store))
        }
        Write::PersonaOptimize {
            of,
            with_shared,
            stale_days,
        } => {
            let (scope, owner) = match of {
                Store::Workspace => {
                    return Err(
                        "persona optimize curates a persona's memory or shared memory, not a \
                         workspace's: nothing was written"
                            .to_owned(),
                    );
                }
                store => own_scope(root, asker, *store)?,
            };
            let store = guard_store(root, &scope)?;
            if *with_shared {
                guard_store(root, &Scope::Shared)?;
            }
            let mut changed = false;
            let code = persona::optimize(
                root,
                &persona::Optimize {
                    name: Some(&owner),
                    all: *with_shared,
                    apply: true,
                    stale_days: *stale_days,
                    today: now.date(),
                },
                &mut || {
                    changed = true;
                    saved();
                },
                &mut say,
            );
            (code, changed, (owner, store))
        }
        Write::TodoClose { how, slug } => {
            let ws = own_workspace(root, asker)?;
            guard(root, &ws.dir().join("todos"), None)?;
            guard_store(root, &Scope::Workspace(ws.name().to_owned()))?;
            crate::wscmd::todo::finish_promotes(&plane, &ws, now, &mut say);
            // A chat's: its `forget` reaches the open todos only (ADR 0106).
            let code = crate::wscmd::todo::close_as(
                &plane,
                &ws,
                *how,
                slug,
                now,
                crate::wscmd::todo::Asker::Chat,
                &mut say,
            );
            let owner = ws.name().to_owned();
            let store = format!("workspaces/{owner}/todos");
            (code, code == 0, (owner, store))
        }
        Write::WorkspaceSnapshot { description, force } => {
            let ws = own_workspace(root, asker)?;
            guard(root, &ws.dir().join(crate::settings::workspace::FILE), None)?;
            let when = chrono::TimeZone::from_local_datetime(&chrono::Local, &now)
                .single()
                .map_or_else(chrono::Utc::now, |local| local.with_timezone(&chrono::Utc));
            let code = crate::wscmd::snapshot::snapshot(
                &crate::wscmd::snapshot::Request {
                    root,
                    ws: ws.name(),
                    description: description.as_deref(),
                    force: *force,
                    now: when,
                },
                &mut say,
            );
            let owner = ws.name().to_owned();
            let store = format!("workspaces/{owner}/{}", crate::settings::workspace::FILE);
            (code, code == 0, (owner, store))
        }
        Write::WorkspaceCreate { name, vision } => {
            let code = create_workspace(root, name, vision.as_deref(), now, &mut say)?;
            (
                code,
                code == 0,
                (name.clone(), format!("workspaces/{name}")),
            )
        }
        _ => return Ok(None),
    };
    // What the terminal's command says once a store changed, said here for the chat's: the
    // curations say it for each store they change, and a todo's close as its journal entry.
    let said_already = matches!(
        write,
        Write::TodoClose { .. }
            | Write::WorkspaceOptimize { .. }
            | Write::PersonaOptimize { .. }
            | Write::WorkspaceCreate { .. }
            | Write::WorkspaceSnapshot { .. }
    );
    if changed && !said_already {
        saved();
    }
    Ok(Some(Upkept {
        lines: lines.into_inner(),
        code,
        changed,
        credited,
    }))
}

/// Makes workspace `name`, which must not be there, with `vision`, as `purlis workspace create`
/// makes it in a terminal (#1408): its exit code, its lines said to `say`. Nothing is created,
/// and the sentence says why, for a name that cannot be a workspace's, a workspace that is
/// there, anything else at its path, or a link on the way to it.
fn create_workspace(
    root: &Path,
    name: &str,
    vision: Option<&str>,
    now: chrono::NaiveDateTime,
    say: &mut dyn FnMut(crate::repocmd::Say),
) -> Result<u8, String> {
    let now = chrono::TimeZone::from_local_datetime(&chrono::Local, &now)
        .earliest()
        .map_or_else(chrono::Utc::now, |at| at.with_timezone(&chrono::Utc));
    create_workspace_at(root, name, vision, &[], None, now, say)
}

/// [`create_workspace`], stamped at `now`, with `repos` cloned into the new workspace, each
/// reaching only `hosts`: how a chat's `workspace create <name> <repo>…` is made
/// ([`crate::gitbroker`], qw387). Nothing is cloned where the workspace was not made.
pub(crate) fn create_workspace_at(
    root: &Path,
    name: &str,
    vision: Option<&str>,
    repos: &[String],
    hosts: Option<&[String]>,
    now: chrono::DateTime<chrono::Utc>,
    say: &mut dyn FnMut(crate::repocmd::Say),
) -> Result<u8, String> {
    let shown_name = crate::shown::short(name);
    let Some(dir) = crate::wscmd::workspace_dir(root, name) else {
        return Err(format!(
            "'{shown_name}' cannot name a workspace: nothing was created"
        ));
    };
    if crate::wscmd::workspace_dir_exists(root, name) {
        return Err(format!(
            "workspace '{shown_name}' is already there, and a chat creates only a new one: \
             nothing was created. Work in it as it is, or create one under another \
             name"
        ));
    }
    if let Err(why) = crate::contain::no_link_on_the_way(root, &dir) {
        return Err(format!("{why}: nothing was created"));
    }
    if let Err(why) = crate::contain::writable(root, &dir) {
        return Err(format!("{why}: nothing was created"));
    }
    guard(root, &dir, None).map_err(|why| format!("{why}: nothing was created"))?;
    Ok(crate::wscmd::create::create(
        &crate::wscmd::create::Request {
            root,
            name,
            vision,
            live: false,
            use_it: false,
            force: false,
            repos,
            hosts,
            now,
            ids: &crate::active::Ids::default(),
        },
        say,
    ))
}

/// One persistent memory of persona `owner` (or of [`crate::contain::SHARED_PERSONA`]), indexed:
/// the write `purlis persona remember` makes, here or brokered.
pub fn remember_persona(
    root: &Path,
    owner: &str,
    text: &str,
    title: Option<&str>,
    now: chrono::NaiveDateTime,
) -> Result<PathBuf, String> {
    if owner != crate::contain::SHARED_PERSONA
        && let Some(refused) = crate::personas::name_refusal(root, owner)
    {
        return Err(refused);
    }
    let text = crate::memstore::py_strip(text);
    if text.is_empty() {
        return Err("empty memory".to_owned());
    }
    let title = crate::personas::memory_title(text, title);
    let title = if title.is_empty() {
        crate::memstore::title_of(text)
    } else {
        title
    };
    guard(
        root,
        &root.join("personas").join(owner).join("memory"),
        None,
    )?;
    held_persona_write(root, owner, text, &title, now)
}

/// Writes one memory into persona `owner`'s store held by descriptor (V74): `personas/`, the
/// persona and `memory/` opened one at a time without following a link, the file and its index
/// line written relative to that, so a link planted on the way, or swapped in while it is
/// written, carries nothing anywhere else.
#[cfg(unix)]
fn held_persona_write(
    root: &Path,
    owner: &str,
    text: &str,
    title: &str,
    now: chrono::NaiveDateTime,
) -> Result<PathBuf, String> {
    use crate::held::{Make, Store, Who};
    let link = |below: &str| {
        format!(
            "personas/{below} could not be opened without following a link, so nothing was written"
        )
    };
    let personas = Store::hold(
        root,
        &Place::PlaneRoot,
        "personas",
        Make::Store,
        Who::Operator,
    )
    .map_err(String::from)?
    .ok_or_else(|| link(""))?;
    let shared = owner == crate::contain::SHARED_PERSONA;
    let persona = personas
        .sub(owner, shared)
        .map_err(String::from)?
        .ok_or_else(|| format!("no persona '{owner}'"))?;
    let memory = persona
        .sub("memory", true)
        .map_err(String::from)?
        .ok_or_else(|| link(&format!("{owner}/memory")))?;
    let _locked = memory.lock()?;
    let file = crate::memstore::write_in(
        &memory,
        &crate::memstore::NoGates,
        text,
        title,
        false,
        "persistent",
        true,
        now,
    )
    .map_err(|e| e.to_string())?;
    Ok(root.join("personas").join(owner).join("memory").join(file))
}

/// Fail closed: the stores are held through descriptors that refuse links, which only the
/// unix build does yet.
#[cfg(not(unix))]
fn held_persona_write(
    _root: &Path,
    _owner: &str,
    _text: &str,
    _title: &str,
    _now: chrono::NaiveDateTime,
) -> Result<PathBuf, String> {
    Err("persona memory is not written on this platform yet".to_owned())
}

/// Sets the `## Vision` of the workspace at `place` to `text`: the write `purlis workspace
/// vision` makes, brokered for a chat or made by purlis's MCP server where no app takes it
/// (#1384). `who` is the chat, as a refusal names it.
pub fn set_vision(root: &Path, place: &Place, who: &str, text: &str) -> Result<Written, String> {
    let text = crate::memstore::py_strip(text);
    if text.is_empty() {
        return Err("a vision has words: nothing was written".to_owned());
    }
    // Any `##` at the start of a line, however it is spaced: the reader ends the vision at
    // `^##\s` (a tab, a no-break space), the writer finds a section by `^##\s+<name>`, and a
    // Markdown renderer takes an indented one for a heading too. The writer's own end-of-
    // section scan, a literal `"## "`, is the narrowest of them and not the one to check.
    if crate::mdsection::split_lines(text).iter().any(|line| {
        line.trim_start_matches(crate::memstore::is_python_space)
            .starts_with("##")
    }) {
        return Err(
            "a line of a vision starts with `## `, or `##` spaced another way, which would \
             start a section of its own in workspace.md: nothing was written"
                .to_owned(),
        );
    }
    hides_nothing_after_it(text, "a vision")?;
    let ws = workspace_at(root, place, who, "a vision")?;
    let path = ws.dir().join("workspace.md");
    guard(root, &path, None)?;
    within_the_budget(root, &path, |now| {
        crate::mdsection::replace(now, "Vision", text).len()
    })?;
    ws.set_vision(text).map_err(|e| e.to_string())?;
    Ok(Written {
        to: ws.name().to_owned(),
        path: shown(root, &path),
    })
}

/// Adds `text` as one entry under `section` of the `workspace.md` of the workspace at `place`,
/// keeping what is there (#1384). `who` is the chat, as a refusal names it.
pub fn add_to_section(
    root: &Path,
    place: &Place,
    who: &str,
    section: Section,
    text: &str,
) -> Result<Written, String> {
    if crate::memstore::py_strip(text).is_empty() {
        return Err(format!(
            "an entry for ## {} has words: nothing was written",
            section.header()
        ));
    }
    hides_nothing_after_it(text, &format!("an entry for ## {}", section.header()))?;
    let ws = workspace_at(root, place, who, "a workspace.md")?;
    let path = ws.dir().join("workspace.md");
    guard(root, &path, None)?;
    // At least the entry's own bytes: it is added whole, as one list item.
    within_the_budget(root, &path, |now| now.len() + text.len())?;
    ws.add_to_section(section.header(), text)
        .map_err(|e| e.to_string())?;
    Ok(Written {
        to: ws.name().to_owned(),
        path: shown(root, &path),
    })
}

/// The most a chat's write grows a workspace's `workspace.md` to (#1598, D-1598-2). Every
/// later chat in the workspace reads the file whole, so it holds what each must act on, not
/// all a workspace learned: that is its memory. Well under the bound on any one project file
/// (`memstore::MAX_BYTES`). A file already past it, by a person's hand, is not refused for
/// that: only a write that would grow it is.
pub const WORKSPACE_MD_GROWS_TO_AT_MOST: usize = 64 * 1024;

/// Refuses a write that would take the `workspace.md` at `path` past
/// [`WORKSPACE_MD_GROWS_TO_AT_MOST`] and grow it: `next` is how long the file would be after
/// the write, from the text it holds now. A file not there yet holds none, and so does one
/// that is not read without following a link: the write itself refuses that one.
fn within_the_budget(
    root: &Path,
    path: &Path,
    next: impl FnOnce(&str) -> usize,
) -> Result<(), String> {
    let now = crate::contain::read_text_no_link(root, path).unwrap_or_default();
    let next = next(&now);
    if next <= WORKSPACE_MD_GROWS_TO_AT_MOST || next <= now.len() {
        return Ok(());
    }
    Err(format!(
        "workspace.md would be {next} bytes, past the {} KiB a chat may grow it to, since every \
         later chat here reads it whole: nothing was written. Keep it to what each later chat \
         must act on: record the rest as a workspace memory (purlis workspace remember), and \
         ask the person to fold older entries down",
        WORKSPACE_MD_GROWS_TO_AT_MOST / 1024
    ))
}

/// Refuses `text`, a vision or a section entry (`what`), where it opens an HTML comment, a
/// code fence or one of the raw HTML blocks that run to the end of the document until their
/// end comes (`<pre>`, `<script>`, `<style>`, `<textarea>`, `<?`, `<!` and a letter,
/// `<![CDATA[`), and does not close it (#1598): a Markdown renderer would hide every section of
/// `workspace.md` after it, as the app's drops raw HTML. A `<!--` inside a code fence or a code
/// span is code, not a comment. A fence or a raw block starts on a line indented at most three
/// spaces, as CommonMark reads one: a line indented further neither opens nor closes one.
fn hides_nothing_after_it(text: &str, what: &str) -> Result<(), String> {
    let hidden = |by: &str| {
        Err(format!(
            "{what} opens {by} and does not close it, which would hide every section of \
             workspace.md after it: nothing was written. Close it, or leave it out"
        ))
    };
    // A line with its indentation of at most three spaces taken off; `None` for one indented
    // further, where neither a fence nor a raw block starts.
    fn unindented(line: &str) -> Option<&str> {
        let rest = line.trim_start_matches(' ');
        (line.len() - rest.len() <= 3 && !rest.starts_with('\t')).then_some(rest)
    }
    // The fence a line opens: its character, how many, and what follows them.
    fn fence_of(line: &str) -> Option<(char, usize, &str)> {
        let line = unindented(line)?;
        let mark = line.chars().next().filter(|c| matches!(c, '`' | '~'))?;
        let many = line.chars().take_while(|c| *c == mark).count();
        (many >= 3).then(|| (mark, many, &line[many..]))
    }
    // The ends a raw HTML block that `line` opens waits for, of the kinds that run on to the
    // end of the document without one (CommonMark's kinds 1, 3, 4 and 5; a comment is read
    // below, wherever on a line it opens).
    fn raw_block_of(line: &str) -> Option<&'static [&'static str]> {
        let line = unindented(line)?.to_ascii_lowercase();
        let tag = |name: &str| {
            line.strip_prefix('<')
                .and_then(|rest| rest.strip_prefix(name))
                .is_some_and(|after| after.is_empty() || after.starts_with([' ', '\t', '>']))
        };
        if ["script", "pre", "style", "textarea"].into_iter().any(tag) {
            Some(&["</script>", "</pre>", "</style>", "</textarea>"])
        } else if line.starts_with("<![cdata[") {
            Some(&["]]>"])
        } else if line.starts_with("<?") {
            Some(&["?>"])
        } else if line
            .strip_prefix("<!")
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_alphabetic()))
        {
            Some(&[">"])
        } else {
            None
        }
    }
    let ends_in = |line: &str, ends: &[&str]| {
        let line = line.to_ascii_lowercase();
        ends.iter().any(|end| line.contains(end))
    };
    let mut fence: Option<(char, usize)> = None;
    let mut raw: Option<&'static [&'static str]> = None;
    let mut in_comment = false;
    for line in crate::mdsection::split_lines(text) {
        if let Some(ends) = raw {
            if ends_in(line, ends) {
                raw = None;
            }
            continue;
        }
        if !in_comment {
            if let Some((mark, many)) = fence {
                if fence_of(line).is_some_and(|(closing, more, rest)| {
                    closing == mark && more >= many && rest.trim().is_empty()
                }) {
                    fence = None;
                }
                continue;
            }
            if let Some((mark, many, _)) = fence_of(line) {
                fence = Some((mark, many));
                continue;
            }
            if let Some(ends) = raw_block_of(line) {
                if !ends_in(line, ends) {
                    raw = Some(ends);
                }
                continue;
            }
        }
        let read = without_code_spans(line);
        let mut rest = read.as_str();
        loop {
            let (looked_for, found) = if in_comment {
                ("-->", rest.find("-->"))
            } else {
                ("<!--", rest.find("<!--"))
            };
            let Some(at) = found else { break };
            rest = &rest[at + looked_for.len()..];
            in_comment = !in_comment;
        }
    }
    if fence.is_some() {
        return hidden("a code fence (``` or ~~~)");
    }
    if raw.is_some() {
        return hidden("a raw HTML block (<pre>, <script>, <style>, <textarea>, <? or <!)");
    }
    if in_comment {
        return hidden("an HTML comment (<!--)");
    }
    Ok(())
}

/// `line` with its code spans taken out: a run of backticks, what follows it, and the next run
/// of as many. A run with no match is kept, as Markdown keeps it.
fn without_code_spans(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find('`') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let many = after.chars().take_while(|c| *c == '`').count();
        let (run, inside) = after.split_at(many);
        // The next run of exactly `many` backticks closes the span.
        let mut closes = None;
        let mut from = 0;
        while let Some(found) = inside[from..].find(run) {
            let start = from + found;
            let length = inside[start..].chars().take_while(|c| *c == '`').count();
            if length == many {
                closes = Some(start + many);
                break;
            }
            from = start + length;
        }
        match closes {
            Some(end) => rest = &inside[end..],
            None => {
                out.push_str(run);
                rest = inside;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The workspace at `place`, which must be there: a brokered write never makes one. `who` and
/// `what` say, in a refusal, which chat has no workspace and what it has none of.
fn workspace_at(
    root: &Path,
    place: &Place,
    who: &str,
    what: &str,
) -> Result<crate::workspaces::Workspace, String> {
    let Place::Workspace(name) = place else {
        return Err(format!(
            "{who} works at the project root, not in a workspace, so it has no {what} to write"
        ));
    };
    let ws = crate::workspaces::Plane::open(root)
        .workspace(name)
        .map_err(|e| e.to_string())?;
    if !ws.dir().is_dir() {
        return Err(format!("no workspace '{name}' in this project"));
    }
    Ok(ws)
}

/// The chat's own workspace, which must be there: a brokered write never makes one.
fn own_workspace(root: &Path, asker: &Asker) -> Result<crate::workspaces::Workspace, String> {
    workspace_at(
        root,
        &asker.place,
        &format!("chat {}", asker.chat),
        "workspace memory or todos",
    )
}

fn shown(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The tables of a manifest a chat runs under, which no brokered write may change, besides
/// `[sandbox]`, which the sandbox's own rule judges ([`crate::sandbox::changes_a_sandbox_key`]):
/// its environment, and `[dispatch]`: the limits on what it may dispatch (#1439) and who may
/// dispatch to whom (#1437). One table, named once.
const RUNS_UNDER_BESIDES_SANDBOX: [&str; 2] = ["chat_env", crate::dispatchlimits::TABLE];

/// Whether a brokered write may write `target` in the project at `root`, with `after` the whole
/// text it would leave there when that is known; or the sentence saying why not.
///
/// Refused, with every name compared case-folded (a case-insensitive disk opens `.CLAUDE` as
/// `.claude`), and read through links:
///
/// - anything at or below a name the sandbox denies a chat as later code
///   ([`crate::sandbox::PLANTED`]: git's folder, hook managers', every harness's and editor's
///   project config, shell startup files), at any depth;
/// - any `.gitignore`;
/// - a manifest or local settings file whose `[sandbox]`, `[chat_env]` or `[dispatch]` would
///   change, or whose new text is not known.
///
/// What decides whether the local settings file is this machine's own (the index and the
/// ignore files) is what decides which sandbox a chat runs under, so it is refused with the
/// tables themselves (the #1341 review).
pub fn guard(root: &Path, target: &Path, after: Option<&str>) -> Result<(), String> {
    let lands = crate::contain::resolved(target).unwrap_or_else(|| target.to_path_buf());
    let base = crate::contain::resolved(root).unwrap_or_else(|| root.to_path_buf());
    let inside = lands.strip_prefix(&base).unwrap_or(&lands);
    let said = || shown(&base, &lands);
    let parts: Vec<String> = inside
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let manifests: Vec<String> = [crate::names::PLANE_MANIFEST, crate::names::LOCAL_SETTINGS]
        .iter()
        .flat_map(|n| std::iter::once(n.write).chain(n.reads.iter().copied()))
        .map(str::to_lowercase)
        .collect();
    for (at, part) in parts.iter().enumerate() {
        if part == ".gitignore" {
            return Err(format!(
                "{} is not purlis's to write for a chat: an ignore file decides whether this \
                 machine's settings, and its sandbox, are its own",
                said()
            ));
        }
        if manifests.contains(part) {
            let last = at + 1 == parts.len();
            let before = std::fs::read_to_string(&lands).ok();
            let changes = !last
                || after
                    .is_none_or(|after| changes_what_a_chat_runs_under(before.as_deref(), after));
            if changes {
                return Err(format!(
                    "{} is not purlis's to change for a chat this way: a chat never changes its \
                     sandbox settings ([sandbox]), its environment ([chat_env]) or its dispatch \
                     limits and grants ([dispatch])",
                    said()
                ));
            }
            continue;
        }
        if planted_name(part) {
            return Err(format!(
                "{} is not purlis's to write for a chat: what is under {part} is loaded by a \
                 program run later, outside the sandbox",
                said()
            ));
        }
    }
    Ok(())
}

/// Whether `name`, already lower-cased, is the first name of one the sandbox denies a chat as
/// later code ([`crate::sandbox::PLANTED`]).
fn planted_name(name: &str) -> bool {
    crate::sandbox::PLANTED.iter().any(|planted| {
        planted
            .path
            .split('/')
            .next()
            .is_some_and(|first| first.to_lowercase() == name)
    })
}

/// Whether `after` holds different `[sandbox]`, `[chat_env]` or `[dispatch]` tables from
/// `before` (none: no file). A text that is not TOML changes them: it cannot be read to say it does not.
fn changes_what_a_chat_runs_under(before: Option<&str>, after: &str) -> bool {
    runs_under_differs(before, Some(after))
}

/// **Whether two texts of a project file hold different `[sandbox]`, `[chat_env]` or
/// `[dispatch]` tables** (none: no file): what a chat's commit is held to (#1464,
/// [`crate::diffscan::Scan::stages_what_a_chat_runs_under`]) as a brokered write is. The same
/// text never differs; a text that is not TOML differs from any other, since it cannot be read
/// to say it does not.
///
/// `[sandbox]` is judged by the sandbox's own rule ([`crate::sandbox::changes_a_sandbox_key`],
/// #1408), so the two can never disagree about it; the other two tables are compared here.
pub fn runs_under_differs(one: Option<&str>, other: Option<&str>) -> bool {
    if one == other {
        return false;
    }
    // The rule reads "before" and "after"; which is which does not change whether they differ,
    // and a side with no file is the "before" it takes as none.
    let sandbox = match (one, other) {
        (before, Some(after)) => crate::sandbox::changes_a_sandbox_key(before, after),
        (Some(after), None) => crate::sandbox::changes_a_sandbox_key(None, after),
        (None, None) => false,
    };
    if sandbox {
        return true;
    }
    let tables = |text: Option<&str>| -> Option<Vec<Option<toml::Value>>> {
        let Some(text) = text else {
            return Some(vec![None; RUNS_UNDER_BESIDES_SANDBOX.len()]);
        };
        let top = text.parse::<toml::Table>().ok()?;
        Some(
            RUNS_UNDER_BESIDES_SANDBOX
                .iter()
                .map(|t| top.get(*t).cloned())
                .collect(),
        )
    };
    match (tables(one), tables(other)) {
        (Some(one), Some(other)) => one != other,
        _ => true,
    }
}

/// How many brokered writes one chat may make in a window: a chat that writes memory in a loop
/// is stopped, and told so, long before it fills the project.
#[derive(Debug)]
pub struct Rate {
    most: usize,
    per: std::time::Duration,
    seen: std::sync::Mutex<
        std::collections::HashMap<u32, std::collections::VecDeque<std::time::Instant>>,
    >,
}

/// The cap the app holds every chat to: 60 brokered writes a minute. A session record, a few
/// memories and todos a turn are far under it.
pub const WRITES_A_MINUTE: usize = 60;

impl Default for Rate {
    fn default() -> Self {
        Self::new(WRITES_A_MINUTE, std::time::Duration::from_secs(60))
    }
}

impl Rate {
    pub fn new(most: usize, per: std::time::Duration) -> Self {
        Self {
            most,
            per,
            seen: std::sync::Mutex::default(),
        }
    }

    /// Counts one write by `chat` at `now`, or says why it is refused.
    pub fn allow(&self, chat: u32, now: std::time::Instant) -> Result<(), String> {
        if self.took(chat, now) {
            return Ok(());
        }
        Err(format!(
            "chat {chat} has asked purlis for {} writes in the last {} seconds, which is its \
             cap; nothing was written. Wait a little, and write fewer, larger memories",
            self.most,
            self.per.as_secs()
        ))
    }

    /// How many it lets through within [`Self::per`].
    pub fn most(&self) -> usize {
        self.most
    }

    /// The while [`Self::most`] is counted over.
    pub fn per(&self) -> std::time::Duration {
        self.per
    }

    /// Counts one by `chat` at `now`, answering whether it was within the cap. One past it is
    /// not counted: it was refused.
    pub(crate) fn took(&self, chat: u32, now: std::time::Instant) -> bool {
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Every chat's old writes go, and a chat with none left goes with them, so the map
        // holds only chats that wrote within the window.
        seen.retain(|_, times| {
            while times
                .front()
                .is_some_and(|at| now.saturating_duration_since(*at) >= self.per)
            {
                times.pop_front();
            }
            !times.is_empty()
        });
        let times = seen.entry(chat).or_default();
        if times.len() >= self.most {
            return false;
        }
        times.push_back(now);
        true
    }

    /// Lets go of what is counted for `chat`, which has closed.
    pub fn forget(&self, chat: u32) {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&chat);
    }

    /// How many chats are counted now.
    pub fn counted(&self) -> usize {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

/// How many session records one chat may ask the app to write (#1361): a few in a while, and a
/// bounded number in all. Each one is a file in the project, a commit and a Notice in the
/// window, so a chat that asks in a loop is stopped, and told why, long before it fills any of
/// them. Held apart from [`Rate`]: a record is one a session, where memories are many a turn.
#[derive(Debug)]
pub struct Records {
    most: usize,
    rate: Rate,
    asked: std::sync::Mutex<std::collections::HashMap<u32, usize>>,
}

/// The most session records one chat may ask for while it is open. A session writes one at its
/// end, and perhaps one midway: this is many sessions' worth, so only a loop reaches it.
pub const RECORDS_A_CHAT: usize = 30;

/// How many session records one chat may ask for within [`RECORDS_WITHIN`]: room for a record
/// refused once for its text and asked again, and one more.
pub const RECORDS_A_WHILE: usize = 5;

/// The while [`RECORDS_A_WHILE`] is counted over.
pub const RECORDS_WITHIN: std::time::Duration = std::time::Duration::from_secs(10 * 60);

impl Default for Records {
    fn default() -> Self {
        Self::new(RECORDS_A_CHAT, RECORDS_A_WHILE, RECORDS_WITHIN)
    }
}

impl Records {
    pub fn new(most: usize, a_while: usize, within: std::time::Duration) -> Self {
        Self {
            most,
            rate: Rate::new(a_while, within),
            asked: std::sync::Mutex::default(),
        }
    }

    /// Counts one session record `chat` asked for at `now`, or says why it is refused. A
    /// refused one is not counted: nothing was written.
    pub fn allow(&self, chat: u32, now: std::time::Instant) -> Result<(), String> {
        let mut asked = self
            .asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let so_far = asked.get(&chat).copied().unwrap_or(0);
        if so_far >= self.most {
            return Err(format!(
                "chat {chat} has asked purlis for {} session records, which is the most one chat \
                 may write; nothing was written. A record sums up a whole session: start a new \
                 chat for new work, and it writes its own",
                self.most
            ));
        }
        if !self.rate.took(chat, now) {
            return Err(format!(
                "chat {chat} has asked purlis for {} session records in the last {} minutes, \
                 which is its cap; nothing was written. Wait a little: a record sums up the \
                 session, so write it once, at its end",
                self.rate.most,
                self.rate.per.as_secs() / 60
            ));
        }
        asked.insert(chat, so_far + 1);
        Ok(())
    }

    /// Lets go of what is counted for `chat`, which has closed.
    pub fn forget(&self, chat: u32) {
        self.rate.forget(chat);
        self.asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&chat);
    }
}

#[cfg(test)]
#[path = "brokered_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "brokered_memory_tests.rs"]
mod memory_tests;

#[cfg(test)]
#[path = "brokered_create_tests.rs"]
mod create_tests;
