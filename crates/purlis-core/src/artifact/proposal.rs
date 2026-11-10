// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat proposes a pin; only the person pins** (#5963, ADR 0122 §1, spec #5948 story 13).
//!
//! A chat or a skill may ask for an artifact folder of its own workspace to be pinned, with the
//! `artifact_propose_pin` tool or `purlis artifact pin` run in the chat. Neither writes the
//! pins file: the ask goes to the app (`hookwire::Ask::ProposePin`), which holds it for the
//! person beside the chat's other proposed changes, and only their press of Pin to workspace
//! in purlis's own dialog pins it. A dismissed or unanswered proposal changes nothing.
//!
//! **What is shown and what is pinned are judged here, by [`super::checked`]**, the rules
//! [`super::pin`] writes by: a folder that would be refused (outside `artifacts/`, holding
//! something shaped like a secret, over the size cap) is refused when it is proposed, so the
//! person is never asked about a pin that cannot be made. The workspace is never the chat's to
//! name: it is the one the app recorded for the chat.
//!
//! **A skill that ships the page names itself** (#5974): the app then places the skill's
//! template in the workspace ([`super::template`]) before the proposal is judged.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::{Checked, Pin, Pinned, Request};
use crate::chattools::Tool;

/// The tool a chat proposes a pin with.
pub const TOOL: &str = "artifact_propose_pin";

/// The tool, for the one catalogue ([`crate::catalogue::every_tool`]).
pub static TOOLS: [Tool; 1] = [Tool {
    name: TOOL,
    description: "Propose pinning an artifact folder of this chat's workspace (a page in \
                  artifacts/<folder>/index.html) to the workspace, so it gets an icon on the \
                  activity bar. Nothing is pinned by this: the person is shown it in purlis's \
                  window, which says who will see it, and only their press pins it. You are told \
                  on your next turn whether they pinned or dismissed it. `purlis artifact pin \
                  <folder>` run in this chat proposes the same way. A skill that ships the page \
                  names itself as `skill`, and purlis places the page first.",
    schema: || {
        json!({
            "type": "object",
            "properties": {
                "folder": {
                    "type": "string",
                    "description": "The folder's name inside the workspace's artifacts/.",
                },
                "name": {
                    "type": "string",
                    "description": "A name for its icon, which the person may change \
                                    (default: the page's title).",
                },
                "icon": {
                    "type": "string",
                    "description": "One of purlis's icons, which the person may change \
                                    (default: chart).",
                },
                "command": {
                    "type": "string",
                    "description": "The command that writes the artifact's data, which \
                                    Regenerate runs.",
                },
                "cwd": {
                    "type": "string",
                    "description": "That command's working folder, relative to the workspace.",
                },
                "skill": {
                    "type": "string",
                    "description": "A skill of this chat that ships the page as its \
                                    artifacts/<folder>/: it is first copied, without its \
                                    data files, into the workspace's artifacts/<folder>/, \
                                    and a copy of the same page is left there as it is.",
                },
            },
            "required": ["folder"],
            "additionalProperties": false,
        })
    },
    read_only: false,
}];

/// **A pin a chat proposed**: what [`super::Request`] would ask, before the person answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposed {
    pub folder: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// **The skill of the chat that ships the page** (#5974), as its `artifacts/<folder>/`:
    /// placed in the workspace ([`super::template::place_from_skill`]) before it is judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
}

impl Proposed {
    /// The tool's arguments, as a proposal; refused for anything the schema does not take.
    pub fn from_args(args: &Map<String, Value>) -> Result<Self, String> {
        let text = |key: &str| match args.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(_) => Err(format!("{key} is text")),
        };
        if let Some(key) = args.keys().find(|key| {
            !["folder", "name", "icon", "command", "cwd", "skill"].contains(&key.as_str())
        }) {
            return Err(format!(
                "{} is not something {TOOL} takes",
                crate::personas::one_line(key)
            ));
        }
        let folder = text("folder")?
            .filter(|folder| !folder.trim().is_empty())
            .ok_or("folder is the name of a folder in the workspace's artifacts/")?;
        Ok(Self {
            folder,
            name: text("name")?,
            icon: text("icon")?,
            command: text("command")?,
            cwd: text("cwd")?,
            skill: text("skill")?,
        })
    }

    /// The request [`super::pin`] would be asked, with the person's `name` and `icon` where
    /// they gave them.
    pub fn request(&self, name: Option<&str>, icon: Option<&str>) -> Request {
        Request {
            folder: self.folder.clone(),
            name: name.map(str::to_owned).or_else(|| self.name.clone()),
            icon: icon.map(str::to_owned).or_else(|| self.icon.clone()),
            command: self.command.clone(),
            cwd: self.cwd.clone(),
        }
    }

    /// What it is called where the person reads it: the folder and the workspace, in words.
    pub fn title(&self, workspace: &str) -> String {
        format!(
            "pin artifacts/{}/ to {}",
            crate::personas::one_line(&self.folder),
            crate::personas::one_line(workspace)
        )
    }
}

/// **A proposed pin as the person is shown it**: what it would pin, the folder's pin now if it
/// has one, and a state that says whether either changed by the time they answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub checked: Checked,
    /// The folder's pin now, where it has one: the proposal then changes it.
    pub pinned: Option<Pin>,
    /// What the folder's pin is now, in a word: another at the answer makes the proposal stale.
    pub state: String,
}

/// **Judges `proposed` in the workspace at `workspace`**, writing nothing: why not, in purlis's
/// sentence, where [`super::pin`] would refuse it.
pub fn shown(workspace: &Path, proposed: &Proposed) -> Result<Shown, String> {
    let checked = super::checked(workspace, &proposed.request(None, None))?;
    let pinned = super::read(workspace)?
        .pins
        .into_iter()
        .find(|pin| pin.folder == proposed.folder);
    let state = pinned.as_ref().map_or_else(String::new, |pin| {
        format!(
            "{}\n{}\n{}\n{}",
            pin.id,
            pin.name,
            pin.icon,
            pin.command.as_deref().unwrap_or_default()
        )
    });
    Ok(Shown {
        checked,
        pinned,
        state,
    })
}

/// What the person's Pin to workspace made of a proposal.
#[derive(Debug, Clone)]
pub enum Answered {
    Pinned(Box<Pinned>),
    /// The folder's pin changed since the proposal was shown: nothing was written.
    Stale,
}

/// **Pins `proposed` as the person answered it**, with their `name` and `icon`: judged again
/// first, and nothing written where the folder's pin changed since it was shown as `state`.
/// The command and working folder are the proposal's, which the person was shown.
pub fn pin_as_answered(
    root: &Path,
    workspace: &Path,
    proposed: &Proposed,
    state: &str,
    name: &str,
    icon: &str,
) -> Result<Answered, String> {
    if shown(workspace, proposed)?.state != state {
        return Ok(Answered::Stale);
    }
    super::pin(root, workspace, &proposed.request(Some(name), Some(icon)))
        .map(|pinned| Answered::Pinned(Box::new(pinned)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_with(folder: &str, page: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let at = dir.path().join(super::super::DIR).join(folder);
        std::fs::create_dir_all(&at).unwrap();
        std::fs::write(at.join(super::super::PAGE), page).unwrap();
        dir
    }

    fn proposed(folder: &str) -> Proposed {
        Proposed {
            folder: folder.into(),
            name: None,
            icon: None,
            command: Some("purlis skill run backlog-sync".into()),
            cwd: None,
            skill: None,
        }
    }

    fn pins_file(workspace: &Path) -> std::path::PathBuf {
        workspace.join(super::super::DIR).join(super::super::PINS)
    }

    #[test]
    fn a_proposal_is_shown_with_the_pages_title_and_writes_no_pins_file() {
        let ws = workspace_with("state", "<title>Backlog state</title>");
        let shown = shown(ws.path(), &proposed("state")).unwrap();
        assert_eq!(shown.checked.name, "Backlog state");
        assert_eq!(
            shown.checked.command.as_deref(),
            Some("purlis skill run backlog-sync")
        );
        assert!(shown.pinned.is_none());
        assert!(!pins_file(ws.path()).exists(), "showing it pinned nothing");
    }

    #[test]
    fn a_proposal_pin_would_refuse_is_refused_when_proposed() {
        let ws = workspace_with(
            "leaky",
            &format!(
                "<title>x</title><p>{}</p>",
                ["ghp", "_0123456789abcdefABCDEFghij0123456789"].concat()
            ),
        );
        let why = shown(ws.path(), &proposed("leaky")).unwrap_err();
        assert!(why.contains("shaped like a secret"), "{why}");
        let why = shown(ws.path(), &proposed("../elsewhere")).unwrap_err();
        assert!(!why.is_empty());
        assert!(!pins_file(ws.path()).exists());
    }

    #[test]
    fn the_persons_answer_pins_it_with_their_name_and_icon_and_the_proposals_command() {
        let ws = workspace_with("state", "<title>Backlog state</title>");
        let root = ws.path();
        let state = shown(root, &proposed("state")).unwrap().state;
        let answered =
            pin_as_answered(root, root, &proposed("state"), &state, "Backlog", "rocket").unwrap();
        let Answered::Pinned(pinned) = answered else {
            panic!("pinned: {answered:?}")
        };
        let Pinned::New(pin) = *pinned else {
            panic!("pinned anew")
        };
        assert_eq!(
            (pin.name.as_str(), pin.icon.as_str()),
            ("Backlog", "rocket")
        );
        assert_eq!(
            pin.command.as_deref(),
            Some("purlis skill run backlog-sync")
        );
        assert_eq!(super::super::read(root).unwrap().pins.len(), 1);
    }

    #[test]
    fn an_answer_to_a_proposal_whose_folder_was_pinned_since_is_stale_and_writes_nothing() {
        let ws = workspace_with("state", "<title>Backlog state</title>");
        let root = ws.path();
        let state = shown(root, &proposed("state")).unwrap().state;
        super::super::pin(root, root, &proposed("state").request(Some("Mine"), None)).unwrap();
        let before = std::fs::read_to_string(pins_file(root)).unwrap();
        let answered =
            pin_as_answered(root, root, &proposed("state"), &state, "Theirs", "chart").unwrap();
        assert!(matches!(answered, Answered::Stale), "{answered:?}");
        assert_eq!(std::fs::read_to_string(pins_file(root)).unwrap(), before);
    }

    #[test]
    fn the_tools_arguments_are_a_proposal_and_nothing_else() {
        let args = |value: Value| value.as_object().unwrap().clone();
        let one = Proposed::from_args(&args(json!({"folder": "state", "icon": "chart"}))).unwrap();
        assert_eq!(one.folder, "state");
        assert_eq!(one.icon.as_deref(), Some("chart"));
        let shipped =
            Proposed::from_args(&args(json!({"folder": "state", "skill": "backlog-sync"})))
                .unwrap();
        assert_eq!(shipped.skill.as_deref(), Some("backlog-sync"));
        assert!(Proposed::from_args(&args(json!({}))).is_err());
        assert!(Proposed::from_args(&args(json!({"folder": "s", "workspace": "x"}))).is_err());
        assert!(Proposed::from_args(&args(json!({"folder": 3}))).is_err());
    }
}
