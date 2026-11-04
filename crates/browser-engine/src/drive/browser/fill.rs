// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A secret purlis fills** (#5115, spec #5100): one vault value into one field of a browser
//! the agent is using, under the agent's own lease, so the person's take-over cuts it as it cuts
//! any call of the agent's.
//!
//! **The engine is the one judge at the moment of filling** (SF-2, SF-13). The field is the one
//! the agent's reference names now; the origin it is judged by is that of the document of the
//! frame that holds it, as the engine recorded that document, never what the page says. Where
//! it is not the origin the person approved, nothing is filled, and the refusal says why. A
//! one-line field that takes typing is filled as approved; a multi-line text box or an editable
//! area only where the person answered a notice for this very fill (SF-5, #5120). Its kind is
//! read by a function run in purlis's own world as it fills, so nothing the page's scripts
//! redefine changes it, and the write checks it again in the same step.
//!
//! **The value is kept out before anything else**, for the browser's life, as a login's are
//! ([`super::super::kept::Kept`]): every later snapshot, find, wait and screenshot of every page
//! of the browser leaves it out. The field is blanked before the value goes in, so it is painted
//! over and its tree reads keep what it holds.
//!
//! **The value goes into that one element, never where the focus is**: a function run in
//! purlis's own world in the field's frame checks, in the same step as it writes, that the
//! element is still in its document and that document is still on the approved origin, and
//! writes nothing where either is not. No key is pressed and no focus is moved, so nothing on the
//! page can steer the value into another field or frame between a check and the write. A field
//! that doesn't keep the value whole is emptied, and nothing is filled.
//!
//! **Once a value may be in a page, the agent copies, cuts and drags nothing in that browser**,
//! for its life: what a copy or a drag would take could be the value, or part of it.
//!
//! **While the page holding it lives, the browser is held to its site** (#5117; the person's
//! ruling SF-3 and decision SF-9 on #5096): the caller's [`FillHold`] holds it just before the
//! value goes in, and where it cannot, nothing goes in. It is released once the page loads
//! another document in its main frame or is closed; a route change that loads no document keeps
//! it. And the field itself gives nothing to a copy, a cut or a drag, whoever makes it: the
//! function that writes the value stops each of them on the field first, from purlis's own
//! world, where the page's scripts can't take the stop away.
//!
//! **A filled field takes no keys from the chat** (#5115): for as long as the hold lives, the
//! agent presses no key in the page (a key goes where the focus is, and moves it), and types and
//! chooses nothing anywhere in it, whichever page the call names. A key in the field would change
//! the value, and a page that shows what the field holds would show what is left of it; and the
//! field turns no wheel into a step of its value. What the agent reads is kept free of the value
//! whole, never of a part of it: a part taken out of what the page shows, text the agent wrote
//! among it, would tell the agent which part it was.

use std::sync::Arc;

use serde_json::{Value, json};

use super::super::kept::Kept;
use super::super::tree::NodeKey;
use super::Browser;
use super::takeover::Carrying;
use crate::drive::{
    Answer, Content, Driver, FieldInfo, FieldKind, FillHold, MOST_LABEL_CHARS, Ref, Refusal,
    SecretFill, lock,
};

/// The function that says what kind of field it runs on, and what the page labels it: a
/// constant, run in purlis's own world on the field, so the page's own redefinitions of what it
/// reads are never the ones it calls.
pub(crate) const FIELD_KIND: &str = r"function () {
  if (!(this instanceof Element)) return { kind: 'other', label: '' };
  const oneLine = ['password', 'text', 'email', 'number', 'tel', 'url'];
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

/// The function that writes a value into the field it is given, run in purlis's own world in
/// the field's frame: a constant. In the same step it checks that the field is still in its
/// document and that the document is on `origin`, answering `moved` where not and writing
/// nothing, so no focus and no moment between a check and a write decides where the value goes.
/// Before the value goes in, a copy, a cut and a drag started on the field are stopped for as
/// long as the field lives, and so is one anywhere in its document of a field or a selection that
/// holds the value (#5117): nothing is put on the clipboard or dragged, and nothing of the page
/// hears of them after. And the field is marked as one the browser keeps nothing of, so a
/// page left and gone back to doesn't come back with the value in it. The value goes in through the field's own setter, with the events a
/// page hears for typing, and the answer is how much the field then holds. A field that holds
/// less is emptied again.
///
/// A multi-line text box or an editable area is written only where `manyLines` says the person
/// was asked about this fill (#5120), and answers `kind` otherwise, in the same step, before
/// anything of the field is touched.
pub(crate) const WRITE_FIELD: &str = r"function (field, value, origin, manyLines) {
  if (!field.isConnected || field.ownerDocument.location.origin !== origin) return 'moved';
  let own = null;
  const oneLine = ['password', 'text', 'email', 'number', 'tel', 'url'];
  if (field instanceof HTMLInputElement) {
    if (!oneLine.includes(field.type) || field.disabled || field.readOnly) return 'kind';
    own = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
  } else if (manyLines === true && field instanceof HTMLTextAreaElement) {
    own = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value');
  } else if (manyLines === true && field instanceof HTMLElement && field.isContentEditable) {
    own = Object.getOwnPropertyDescriptor(Node.prototype, 'textContent');
  }
  if (own === null) return 'kind';
  const stop = (event) => {
    if (event.clipboardData) event.clipboardData.clearData();
    if (event.dataTransfer) event.dataTransfer.clearData();
    event.preventDefault();
    event.stopImmediatePropagation();
  };
  // And anywhere in its document, of any field holding the value or a selection that does: a
  // page may draw the field anew, or echo the value, while the hold lives.
  const holds = (text) => typeof text === 'string' && value.length > 0 && text.includes(value);
  const guard = (event) => {
    const at = event.composedPath()[0];
    if (holds(at && at.value) || holds(String(field.ownerDocument.getSelection()))) stop(event);
  };
  for (const kind of ['copy', 'cut', 'dragstart']) {
    field.addEventListener(kind, stop, true);
    field.ownerDocument.addEventListener(kind, guard, true);
  }
  // A wheel over a focused number field steps its value: not this one's.
  field.addEventListener('wheel', (event) => event.preventDefault(), { capture: true, passive: false });
  field.setAttribute('autocomplete', 'off');
  own.set.call(field, value);
  field.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText' }));
  field.dispatchEvent(new Event('change', { bubbles: true }));
  const held = own.get.call(field).length;
  if (held !== value.length) {
    own.set.call(field, '');
    field.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'deleteContent' }));
  }
  return held;
}";

/// A field as resolved for a fill: where it is, and its object in purlis's own world there.
struct Resolved {
    page: String,
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
    pub fn fill_secret(
        &self,
        epoch: u64,
        fill: &SecretFill,
        hold: Arc<dyn FillHold>,
    ) -> Result<Answer, Refusal> {
        self.agent_holds(epoch)?;
        let _one = lock(&self.acting);
        // No read the agent's calls left running is kept past an act.
        lock(&self.reading).forget();
        let answer = {
            let _carrying = Carrying::under(Driver::Agent, epoch);
            let filled = self.filling(fill, hold);
            if self.cut() {
                Err(Refusal::Interrupted)
            } else {
                filled
            }
        };
        answer.map(|answer| self.with_asked(answer))
    }

    fn filling(&self, fill: &SecretFill, hold: Arc<dyn FillHold>) -> Result<Answer, Refusal> {
        // What it fills is kept out before anything else, for the browser's life.
        let mut held = Kept::default();
        held.run(&spellings(&fill.value));
        lock(&self.inner.state).keep_out_everywhere(&held);
        let target = &fill.target;
        let origin = fill.origin.as_str();
        let at = self.resolved(target)?;
        let Some((document, here)) = self.frame_now(&at) else {
            return Err(stale(target));
        };
        // The document the page shows in its main frame: the hold lives as long as it does.
        let Some(page_document) = self.main_document(&at.page) else {
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
        // SF-5: read again as it fills, so a field that became multi-line since is caught.
        let (kind, _) = self.kind_of(&at, target)?;
        takes(kind, fill, target)?;
        self.blank_node(&at.page, at.key.clone())?;
        if self.frame_now(&at) != Some((document, Some(origin.to_owned()))) {
            return Err(changed(target, origin));
        }
        // The browser reaches only this site while the page lives, from before the value goes in
        // (#5117): where it can't be held, nothing goes in.
        self.inner
            .hold_while_it_lives(&at.page, page_document, hold)
            .map_err(|why| {
                not_filled(format!(
                    "purlis could not hold this browser to {origin} while the page is open \
                     ({why}), so nothing was filled: take a new snapshot"
                ))
            })?;
        // From the moment a value may be in a page, the agent copies and cuts nothing in this
        // browser: what it would copy could be the value.
        self.inner
            .copy_withheld
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.written(&at, fill)?;
        // The field holds what was just typed: read again, so it is kept out from now on.
        let _ = self.snapshot_of(&at.page);
        let mut answer = self.answer_now(&at.page, Content::Nothing);
        answer.note = Some(format!(
            "purlis filled {} into {target} on {origin}; the value is kept out of everything you \
             read. While this page is open, the browser reaches only {origin}, and you press no \
             key and type or choose nothing in it: click the page's own button to send the \
             form. Once the page loads another document or is closed, it \
             reaches as before",
            fill.key
        ));
        Ok(answer)
    }

    /// **Writes the value into `at`'s element itself** (module docs), by a function run in
    /// purlis's own world in its frame, which checks in the same step that the element is still
    /// in its document and that document is on the approved origin: no focus decides where it
    /// goes. A field that doesn't keep the value whole (a number field given letters) is
    /// emptied, and nothing is filled.
    fn written(&self, at: &Resolved, fill: &SecretFill) -> Result<(), Refusal> {
        let target = &fill.target;
        let origin = fill.origin.as_str();
        let on = Some(at.node_session.as_str());
        let wrote = self.in_world(&at.node_session, &at.frame, |world| {
            let resolved = self.devtools().call(
                "DOM.resolveNode",
                json!({ "backendNodeId": at.key.backend, "executionContextId": world.id }),
                on,
            )?;
            let Some(object) = resolved.pointer("/object/objectId").and_then(Value::as_str) else {
                return Ok(Value::Null);
            };
            let wrote = self.gated(|| {
                self.devtools().call(
                    "Runtime.callFunctionOn",
                    json!({
                        "uniqueContextId": world.unique,
                        "functionDeclaration": WRITE_FIELD,
                        "arguments": [
                            { "objectId": object },
                            { "value": fill.value.as_str() },
                            { "value": origin },
                            { "value": fill.multi_line },
                        ],
                        "returnByValue": true,
                    }),
                    on,
                )
            });
            let _ =
                self.devtools()
                    .call("Runtime.releaseObject", json!({ "objectId": object }), on);
            match wrote {
                Ok(done) => done,
                // Cut by a take-over: nothing was written.
                Err(_) => Ok(json!({ "result": { "value": "cut" } })),
            }
        });
        let said = wrote
            .ok()
            .and_then(|read| read.pointer("/result/value").cloned());
        let typed = fill.value.encode_utf16().count() as u64;
        match said {
            Some(Value::Number(held)) if held.as_u64() == Some(typed) => Ok(()),
            Some(Value::Number(_)) => Err(not_filled(format!(
                "{target} on {origin} did not keep the value whole, so purlis emptied it and \
                 nothing was filled: the field may take only numbers, or fewer characters"
            ))),
            Some(Value::String(cut)) if cut == "cut" => Err(Refusal::Interrupted),
            Some(Value::String(moved)) if moved == "moved" => Err(changed(target, origin)),
            Some(Value::String(kind)) if kind == "kind" => Err(asks_each_time(target)),
            _ => Err(not_filled(format!(
                "purlis could not reach {target} on {origin}, so nothing was filled: take a new \
                 snapshot"
            ))),
        }
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
            node_session: at.node_session,
            frame: at.frame,
        })
    }

    /// The document `at`'s frame shows now, by its number, and that document's origin, as the
    /// engine recorded it ([`super::State::document_now`]).
    fn frame_now(&self, at: &Resolved) -> Option<(u64, Option<String>)> {
        lock(&self.inner.state).document_now(&at.page, &at.frame)
    }

    /// The document page `page` shows in its main frame, by its number.
    fn main_document(&self, page: &str) -> Option<u64> {
        let state = lock(&self.inner.state);
        Some(state.page(page)?.frames.get(page)?.generation)
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
        // What the function could not read is no field a secret goes into.
        if read.get("exceptionDetails").is_some() {
            return Ok((FieldKind::Other, String::new()));
        }
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

/// **Whether a field of `kind` takes `fill`** (SF-5): a one-line field as approved; a multi-line
/// text box or an editable area only where the person answered a notice for this very fill
/// (#5120); nothing else.
fn takes(kind: FieldKind, fill: &SecretFill, target: &Ref) -> Result<(), Refusal> {
    match kind {
        FieldKind::OneLine => Ok(()),
        FieldKind::MultiLine | FieldKind::Editable if fill.multi_line => Ok(()),
        FieldKind::MultiLine | FieldKind::Editable => Err(asks_each_time(target)),
        FieldKind::Other => Err(not_filled(format!(
            "{target} is not a field that takes typing ({}), so nothing was filled: only a \
             one-line field, a multi-line text box or an editable area takes a secret",
            said_kind(kind)
        ))),
    }
}

/// What a fill is refused with where the field is multi-line or editable and nobody was asked
/// about this fill (#5120).
fn asks_each_time(target: &Ref) -> Refusal {
    not_filled(format!(
        "{target} is a multi-line text box or an editable area rather than a one-line field, so \
         nothing was filled: the person is asked each time before a secret goes into one. Call \
         browser_fill_secret again to ask them"
    ))
}

/// **The value, and how a page could spell it back** (#5116): as it is, percent-encoded as an
/// address carries it, and form-encoded as a form sent by its address does. A page that puts
/// the value into its address, or echoes it so spelled, shows the agent none of it. Each is
/// wiped as it goes.
fn spellings(value: &str) -> Vec<zeroize::Zeroizing<String>> {
    let encoded = |space: &str| {
        let mut out = String::with_capacity(value.len() * 3);
        for byte in value.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(char::from(byte));
                }
                b' ' => out.push_str(space),
                _ => out.push_str(&format!("%{byte:02X}")),
            }
        }
        zeroize::Zeroizing::new(out)
    };
    let mut all = vec![zeroize::Zeroizing::new(value.to_owned())];
    for one in [encoded("%20"), encoded("+")] {
        if !all.iter().any(|seen| **seen == *one) {
            all.push(one);
        }
    }
    all
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

/// What a fill is refused with where the field's document changed before the value went in.
fn changed(target: &Ref, origin: &str) -> Refusal {
    not_filled(format!(
        "the page changed as purlis was about to fill {target} on {origin}, so nothing was \
         filled: take a new snapshot"
    ))
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
    fn a_value_is_kept_out_as_an_address_or_a_form_would_spell_it() {
        let spelled: Vec<String> = spellings("sk+a/b c=")
            .iter()
            .map(|one| one.to_string())
            .collect();
        assert_eq!(
            spelled,
            ["sk+a/b c=", "sk%2Ba%2Fb%20c%3D", "sk%2Ba%2Fb+c%3D"]
        );
        assert_eq!(spellings("plain-word").len(), 1, "nothing spelled twice");
    }

    #[test]
    fn a_label_is_one_plain_line_at_most_its_length() {
        assert_eq!(plain_label("  API\n key\u{200b} "), "API key");
        let long = "k".repeat(MOST_LABEL_CHARS + 20);
        assert_eq!(plain_label(&long).chars().count(), MOST_LABEL_CHARS);
    }
}
