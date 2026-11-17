// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Claude Code's `permissions.deny`**, as the `--settings` purlis wrote hold its native tools
//! (`sandbox::claude`): Claude Code's own process applies them, before any hook and before any
//! I/O, so the fake applies them there too, and only a call they let through meets the wrap.
//!
//! The forms purlis writes: a bare tool (`WebFetch`, `WebSearch`), which takes the tool away
//! from the model altogether, and `Read(<path>)` / `Edit(<path>)` with a gitignore-style path:
//! `//abs` is absolute, `~/` is the home folder, and anything else is read against the chat's
//! folder. `Edit` rules hold every tool that writes a file; `Read` rules every tool that reads one.
//!
//! **Matched as Claude Code matches them**, as gitignore matches a line (the `ignore` library at
//! its defaults, read in its bundled CLI, and measured on the pinned version by
//! `the_smaller_rules_deny_every_spelling_the_old_rules_denied`): a rule holds a path when it
//! matches the path or any folder above it, and letters match in either case. purlis writes each
//! held path once on that footing (#6961), so a fake that matched only the path itself, in its
//! own case, would let through what Claude Code refuses.

use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct Rules {
    /// Tools denied whole.
    tools: Vec<String>,
    read: Vec<glob::Pattern>,
    edit: Vec<glob::Pattern>,
}

/// Why Claude Code refused a call, as it tells the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The tool is not offered at all.
    NoSuchTool,
    /// A path rule holds it.
    Path,
}

impl Rules {
    /// The rules of the `--settings` value `settings` (its JSON, or a file holding it), for a
    /// chat in `cwd`.
    pub fn of_settings(settings: Option<&str>, cwd: &Path) -> Rules {
        let Some(settings) = settings else {
            return Rules::default();
        };
        let text = if settings.trim_start().starts_with('{') {
            settings.to_owned()
        } else {
            std::fs::read_to_string(settings).unwrap_or_default()
        };
        let doc: Value = serde_json::from_str(&text).unwrap_or_default();
        let mut rules = Rules::default();
        for rule in doc
            .pointer("/permissions/deny")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let Some((tool, spec)) = rule.strip_suffix(')').and_then(|r| r.split_once('(')) else {
                rules.tools.push(rule.to_owned());
                continue;
            };
            let Some(pattern) = pattern_of(spec, cwd) else {
                continue;
            };
            match tool {
                "Read" => rules.read.push(pattern),
                "Edit" => rules.edit.push(pattern),
                _ => {}
            }
        }
        rules
    }

    /// Whether Claude Code refuses `tool` on `path` (a file tool's), and how.
    pub fn refuses(&self, tool: &str, path: Option<&str>) -> Option<Refused> {
        if self.tools.iter().any(|t| t == tool) {
            return Some(Refused::NoSuchTool);
        }
        let path = path?;
        let rules = match tool {
            "Read" | "Grep" | "Glob" => &self.read,
            "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => &self.edit,
            _ => return None,
        };
        let options = glob::MatchOptions {
            case_sensitive: false,
            require_literal_separator: true,
            require_literal_leading_dot: false,
        };
        // The path and every folder above it, under each of its spellings.
        let held: Vec<PathBuf> = spellings(Path::new(path))
            .into_iter()
            .flat_map(|spelled| {
                spelled
                    .ancestors()
                    .filter(|it| it.parent().is_some())
                    .map(Path::to_path_buf)
                    .collect::<Vec<_>>()
            })
            .collect();
        rules
            .iter()
            .any(|rule| held.iter().any(|p| rule.matches_path_with(p, options)))
            .then_some(Refused::Path)
    }
}

fn pattern_of(spec: &str, cwd: &Path) -> Option<glob::Pattern> {
    let absolute = if let Some(abs) = spec.strip_prefix("//") {
        format!("/{abs}")
    } else if let Some(home) = spec.strip_prefix("~/") {
        format!("{}/{home}", std::env::var("HOME").unwrap_or_default())
    } else {
        format!("{}/{}", cwd.display(), spec.trim_start_matches('/'))
    };
    glob::Pattern::new(&absolute).ok()
}

/// The path as given, and with its folder's links resolved, as Claude Code checks both.
fn spellings(path: &Path) -> Vec<PathBuf> {
    let mut out = vec![path.to_path_buf()];
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name())
        && let Ok(real) = parent.canonicalize()
    {
        let real = real.join(name);
        if real != path {
            out.push(real);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(deny: &[&str]) -> Rules {
        let settings = serde_json::json!({"permissions": {"deny": deny}}).to_string();
        Rules::of_settings(Some(&settings), Path::new("/work"))
    }

    #[test]
    fn an_absolute_rule_holds_its_folder_and_everything_below() {
        let r = rules(&["Edit(//outside)", "Edit(//outside/**)"]);
        assert_eq!(
            r.refuses("Write", Some("/outside/a/b.txt")),
            Some(Refused::Path)
        );
        assert_eq!(r.refuses("Edit", Some("/outside")), Some(Refused::Path));
        assert_eq!(r.refuses("Write", Some("/work/a.txt")), None);
        assert_eq!(
            r.refuses("Read", Some("/outside/a.txt")),
            None,
            "an Edit rule holds no read"
        );
    }

    #[test]
    fn a_vault_rule_holds_a_vault_at_any_depth_in_either_case() {
        let r = rules(&[
            "Read(//**/.purlis/vaults/**)",
            "Read(//**/.[pP][uU]rlis/vaults/**)",
        ]);
        assert_eq!(
            r.refuses("Read", Some("/work/.purlis/vaults/v.json")),
            Some(Refused::Path)
        );
        assert_eq!(
            r.refuses("Read", Some("/work/.pUrlis/vaults/v.json")),
            Some(Refused::Path)
        );
        assert_eq!(r.refuses("Read", Some("/work/purlis.toml")), None);
    }

    #[test]
    fn a_relative_rule_is_read_against_the_chats_folder() {
        let r = rules(&["Edit(**/.git/hooks/**)"]);
        assert_eq!(
            r.refuses("Write", Some("/work/sub/.git/hooks/pre-commit")),
            Some(Refused::Path)
        );
        assert_eq!(r.refuses("Write", Some("/elsewhere/.git/hooks/x")), None);
    }

    /// What #6961 writes: a held folder once, with no `/**` beside it, and no either-case copy.
    #[test]
    fn a_folder_rule_holds_what_is_below_it_in_either_case() {
        let r = rules(&["Read(//work/.purlis/vaults)", "Edit(//outside)"]);
        assert_eq!(
            r.refuses("Read", Some("/work/.purlis/vaults/v.json")),
            Some(Refused::Path)
        );
        assert_eq!(
            r.refuses("Read", Some("/WORK/.PURLIS/Vaults/v.json")),
            Some(Refused::Path)
        );
        assert_eq!(
            r.refuses("Write", Some("/outside/a/b.txt")),
            Some(Refused::Path)
        );
        assert_eq!(
            r.refuses("Read", Some("/work/.purlis/vaults-notes.txt")),
            None
        );
        assert_eq!(r.refuses("Write", Some("/outsider/a.txt")), None);
    }

    #[test]
    fn a_bare_tool_is_taken_away_whole() {
        let r = rules(&["WebFetch", "WebSearch"]);
        assert_eq!(r.refuses("WebFetch", None), Some(Refused::NoSuchTool));
        assert_eq!(r.refuses("Bash", None), None);
    }
}
