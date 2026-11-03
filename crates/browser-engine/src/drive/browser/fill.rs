// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A secret purlis fills** (#5115, spec #5100): one vault value into one field of a browser
//! the agent is using, under the agent's own lease, so the person's take-over cuts it as it cuts
//! any call of the agent's.
//!
//! **The engine is the one judge at the moment of filling** (SF-2, SF-13). The field is the one
//! the agent's reference names now; the origin it is judged by is that of the document of the
//! frame that holds it, as the engine recorded that document, never what the page says. Where
//! it is not the origin the person approved, nothing is filled, and the refusal says why. Only a
//! one-line field that takes typing is filled (SF-5, one-line only in this slice): its kind is
//! read by a function run in purlis's own world, so nothing the page's scripts redefine changes
//! it.
//!
//! **The value is kept out before anything else**, for the browser's life, as a login's are
//! ([`super::super::kept::Kept`]): every later snapshot, find, wait and screenshot of every page
//! of the browser leaves it out. The field is blanked before the value goes in, so it is painted
//! over and its tree reads keep what it holds.
//!
//! **Right before the value goes in, and right after**, the frame must still show the same
//! document on the approved origin, with the field focused in it. Before, where it does not,
//! nothing was typed and the fill is refused. After, where it does not, or the field doesn't
//! hold exactly what was typed, the value may have gone elsewhere, so purlis ends the whole
//! browser before the agent has anything more of it.

use serde_json::{Value, json};

use super::super::kept::Kept;
use super::super::tree::NodeKey;
use super::Browser;
use super::login::{FIELD_HAS_FOCUS, FIELD_LENGTH, FOCUS_FIELD, origin_of};
use super::takeover::Carrying;
use crate::drive::{
    Answer, Content, Driver, FieldInfo, FieldKind, MOST_LABEL_CHARS, Ref, Refusal, SecretFill, lock,
};

/// The function that says what kind of field it runs on, and what the page labels it: a
/// constant, run in purlis's own world on the field, so the page's own redefinitions of what it
/// reads are never the ones it calls.
pub(crate) const FIELD_KIND: &str = r"function () {
  const oneLine = ['password', 'text', 'email', 'number', 'tel', 'url', 'search'];
  let kind = 'other';
  if (this instanceof HTMLInputElement) {
    if (oneLine.includes(this.type) && !this.disabled && !this.readOnly) kind = 'one_line';
  } else if (this instanceof HTMLTextAreaElement) {
    kind = 'multi_line';
  } else if (this instanceof HTMLElement && this.isContentEditable) {
    kind = 'editable';
  }
  const said = (text) => (typeof text === 'string' ? text : '');
  let label = '';
  if (this.labels && this.labels.length > 0) label = said(this.labels[0].textContent);
  if (!label.trim()) label = said(this.getAttribute('aria-label'));
  if (!label.trim()) label = said(this.getAttribute('placeholder'));
  if (!label.trim()) label = said(this.getAttribute('name'));
  return { kind, label };
}";

/// A field as resolved for a fill: where it is, and its object in purlis's own world there.
struct Resolved {
    page: String,
    page_session: String,
    node_session: String,
    frame: String,
    key: NodeKey,
}

impl Browser {
    /// **What the field `target` names is** (#5115): the origin of its frame's document as the
    /// engine records it, its label and its kind. Nothing is typed.
    pub(super) fn describe_field(&self, target: &Ref) -> Result<Answer, Refusal> {
        let at = self.resolved(target)?;
        let (_, origin) = self.frame_now(&at).ok_or_else(|| stale(target))?;
        let (kind, label) = self.kind_of(&at, target)?;
        Ok(self.answer_now(
            &at.page,
            Content::Field(FieldInfo {
                origin,
                label: plain_label(&label),
                kind,
            }),
        ))
    }

    /// **purlis puts `fill`'s value into its field** (module docs), as the agent's call under the
    /// lease of `epoch`: refused unless the agent holds that lease now, and cut if the person
    /// takes the browser over before it answers. No answer or refusal carries the value.
    pub fn fill_secret(&self, epoch: u64, fill: &SecretFill) -> Result<Answer, Refusal> {
        self.agent_holds(epoch)?;
        let _one = lock(&self.acting);
        // No read the agent's calls left running is kept past an act.
        lock(&self.reading).forget();
        let answer = {
            let _carrying = Carrying::under(Driver::Agent, epoch);
            let filled = self.filling(fill);
            if self.cut() {
                Err(Refusal::Interrupted)
            } else {
                filled
            }
        };
        answer.map(|answer| self.with_asked(answer))
    }

    fn filling(&self, fill: &SecretFill) -> Result<Answer, Refusal> {
        // What it fills is kept out before anything else, for the browser's life.
        let mut held = Kept::default();
        held.run(&[fill.value.as_str()]);
        lock(&self.inner.state).keep_out_everywhere(&held);
        let target = &fill.target;
        let origin = fill.origin.as_str();
        let at = self.resolved(target)?;
        let Some((document, here)) = self.frame_now(&at) else {
            return Err(stale(target));
        };
        // SF-13: the field's own frame, at the moment of filling.
        if here.as_deref() != Some(origin) {
            return Err(not_filled(format!(
                "{target} is in a document on {}, not on {origin}, where the person allowed it, \
                 so nothing was filled",
                here.as_deref().unwrap_or("no web origin")
            )));
        }
        let (kind, _) = self.kind_of(&at, target)?;
        if kind != FieldKind::OneLine {
            return Err(not_filled(format!(
                "{target} is not a one-line field that takes typing ({}), so nothing was filled: \
                 only a one-line field takes a secret",
                said_kind(kind)
            )));
        }
        self.blank_node(&at.page, at.key.clone())?;
        let object = self.field_object(&at).ok_or_else(|| {
            not_filled(format!(
                "purlis could not reach {target} on {origin}, so nothing was filled"
            ))
        })?;
        let filled = self.typed_into(&at, &object, document, fill);
        let on = Some(at.node_session.as_str());
        let _ = self
            .devtools()
            .call("Runtime.releaseObject", json!({ "objectId": object }), on);
        // Focus is emulated for the fill alone.
        for session in [&at.page_session, &at.node_session] {
            let _ = self.devtools().call(
                "Emulation.setFocusEmulationEnabled",
                json!({ "enabled": false }),
                Some(session),
            );
        }
        filled?;
        // The field holds what was just typed: read again, so it is kept out from now on.
        let _ = self.snapshot_of(&at.page);
        let mut answer = self.answer_now(&at.page, Content::Nothing);
        answer.note = Some(format!(
            "purlis filled {} into {target} on {origin}; the value is kept out of everything you \
             read",
            fill.key
        ));
        Ok(answer)
    }

    /// Focuses `object`, checks, types the value, and checks again (module docs).
    fn typed_into(
        &self,
        at: &Resolved,
        object: &str,
        document: u64,
        fill: &SecretFill,
    ) -> Result<(), Refusal> {
        let target = &fill.target;
        let origin = fill.origin.as_str();
        let on = Some(at.node_session.as_str());
        let could_not = || {
            not_filled(format!(
                "purlis could not focus {target} on {origin}, so nothing was filled"
            ))
        };
        // A page purlis drives behaves as one with the focus, whatever the window shows.
        for session in [&at.page_session, &at.node_session] {
            let _ = self.devtools().call(
                "Emulation.setFocusEmulationEnabled",
                json!({ "enabled": true }),
                Some(session),
            );
        }
        self.gated(|| {
            self.devtools().call(
                "Runtime.callFunctionOn",
                json!({ "objectId": object, "functionDeclaration": FOCUS_FIELD }),
                on,
            )
        })?
        .map_err(|_| could_not())?;
        if !self.still_there(at, document, origin, object) {
            return Err(not_filled(format!(
                "the page changed as purlis was about to fill {target} on {origin}, so nothing \
                 was filled: take a new snapshot"
            )));
        }
        self.gated(|| {
            self.devtools().call(
                "Input.insertText",
                json!({ "text": fill.value.as_str() }),
                Some(&at.page_session),
            )
        })?
        .map_err(|_| self.ended_mid_fill(target, origin))?;
        if !self.still_there(at, document, origin, object)
            || !self.holds_as_typed_in(object, fill.value.as_str(), on)
        {
            return Err(self.ended_mid_fill(target, origin));
        }
        Ok(())
    }

    /// `target`'s element, where it is now: refused as a call on it would be.
    fn resolved(&self, target: &Ref) -> Result<Resolved, Refusal> {
        let at = self.locate(target)?;
        Ok(Resolved {
            key: NodeKey {
                frame: at.frame.clone(),
                backend: at.backend,
            },
            page: at.target,
            page_session: at.page_session,
            node_session: at.node_session,
            frame: at.frame,
        })
    }

    /// The document `at`'s frame shows now, by its number, and that document's origin, as the
    /// engine recorded it.
    fn frame_now(&self, at: &Resolved) -> Option<(u64, Option<String>)> {
        let state = lock(&self.inner.state);
        let frame = state.page(&at.page)?.frames.get(&at.frame)?;
        Some((frame.generation, origin_of(&frame.url)))
    }

    /// `at`'s element in purlis's own world in its frame: an object of that world.
    fn field_object(&self, at: &Resolved) -> Option<String> {
        let on = Some(at.node_session.as_str());
        let resolved = self
            .in_world(&at.node_session, &at.frame, |world| {
                self.devtools().call(
                    "DOM.resolveNode",
                    json!({ "backendNodeId": at.key.backend, "executionContextId": world.id }),
                    on,
                )
            })
            .ok()?;
        resolved
            .pointer("/object/objectId")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    /// What kind of field `at`'s element is, and what the page labels it, read in purlis's own
    /// world.
    fn kind_of(&self, at: &Resolved, target: &Ref) -> Result<(FieldKind, String), Refusal> {
        let object = self.field_object(at).ok_or_else(|| stale(target))?;
        let on = Some(at.node_session.as_str());
        let read = self.devtools().call(
            "Runtime.callFunctionOn",
            json!({
                "objectId": object,
                "functionDeclaration": FIELD_KIND,
                "returnByValue": true,
            }),
            on,
        );
        let _ = self
            .devtools()
            .call("Runtime.releaseObject", json!({ "objectId": object }), on);
        let read = read.map_err(|_| Refusal::Unknown)?;
        let value = read.pointer("/result/value").ok_or(Refusal::Unknown)?;
        let kind = match value.get("kind").and_then(Value::as_str) {
            Some("one_line") => FieldKind::OneLine,
            Some("multi_line") => FieldKind::MultiLine,
            Some("editable") => FieldKind::Editable,
            _ => FieldKind::Other,
        };
        let label = value
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        Ok((kind, label))
    }

    /// Whether `at`'s frame still shows `document`, on `origin`, with `object` focused in it and
    /// its document focused: read from the field itself.
    fn still_there(&self, at: &Resolved, document: u64, origin: &str, object: &str) -> bool {
        if self.frame_now(at) != Some((document, Some(origin.to_owned()))) {
            return false;
        }
        let focused = self
            .devtools()
            .call(
                "Runtime.callFunctionOn",
                json!({
                    "objectId": object,
                    "functionDeclaration": FIELD_HAS_FOCUS,
                    "returnByValue": true,
                }),
                Some(&at.node_session),
            )
            .ok()
            .and_then(|read| read.pointer("/result/value").and_then(Value::as_bool))
            == Some(true);
        focused && self.frame_now(at) == Some((document, Some(origin.to_owned())))
    }

    /// Whether `object` holds exactly as much as purlis typed into it, read from the field.
    fn holds_as_typed_in(&self, object: &str, value: &str, on: Option<&str>) -> bool {
        let typed = value.encode_utf16().count() as i64;
        self.devtools()
            .call(
                "Runtime.callFunctionOn",
                json!({
                    "objectId": object,
                    "functionDeclaration": FIELD_LENGTH,
                    "returnByValue": true,
                }),
                on,
            )
            .ok()
            .and_then(|read| read.pointer("/result/value").and_then(Value::as_i64))
            == Some(typed)
    }

    /// **Ends the whole browser**: the value may have gone somewhere other than its field.
    fn ended_mid_fill(&self, target: &Ref, origin: &str) -> Refusal {
        lock(&self.inner.state).ended = true;
        self.inner.changed.notify_all();
        let _ = self.devtools().call("Browser.close", json!({}), None);
        #[cfg(unix)]
        self.stop(std::time::Duration::ZERO);
        not_filled(format!(
            "the page changed while purlis filled {target} on {origin}, so purlis ended this \
             browser: open a new one with browser_new"
        ))
    }
}

/// The page's label made plain: one line, no characters a person doesn't see, at most
/// [`MOST_LABEL_CHARS`] characters.
fn plain_label(label: &str) -> String {
    let seen: String = label
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !purlis_unicode::invisible(*c))
        .collect();
    let words: Vec<&str> = seen.split_whitespace().collect();
    words
        .join(" ")
        .chars()
        .take(MOST_LABEL_CHARS)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// A kind of field, in words.
fn said_kind(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::OneLine => "a one-line field",
        FieldKind::MultiLine => "a multi-line text box",
        FieldKind::Editable => "an editable area of the page",
        FieldKind::Other => "not a field that takes typing",
    }
}

fn not_filled(why: String) -> Refusal {
    Refusal::NotFilled(why)
}

fn stale(target: &Ref) -> Refusal {
    Refusal::StaleRef(target.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_one_plain_line_at_most_its_length() {
        assert_eq!(plain_label("  API\n key\u{200b} "), "API key");
        let long = "k".repeat(MOST_LABEL_CHARS + 20);
        assert_eq!(plain_label(&long).chars().count(), MOST_LABEL_CHARS);
    }
}
