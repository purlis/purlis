// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What DevTools write into a page, kept out as the person's typing is** (#2028).
//!
//! Each call the front end sends that writes into the page (code it runs, an attribute, a
//! node's text or markup, a stored item) gives the texts it writes: a string the code spells
//! out, an attribute's value, the text of the markup. Each of at least [`KEPT_FROM`]
//! characters is kept out of every later answer the agent gets, as a run of the person's keys
//! is. Fields are read from themselves at the hand-back besides (`inspect.rs`).

use serde_json::Value;

/// The shortest text kept out: a code of six characters is, a selector's word is not.
pub(crate) const KEPT_FROM: usize = 6;

/// The texts the front end's call of `method` with `params` writes into the page.
pub(crate) fn written(method: &str, params: &Value) -> Vec<String> {
    let text = |key: &str| params.get(key).and_then(Value::as_str).unwrap_or_default();
    let mut texts = match method {
        "Runtime.evaluate" | "Runtime.compileScript" | "Debugger.evaluateOnCallFrame" => {
            in_code(text("expression"))
        }
        "Runtime.callFunctionOn" => {
            let mut texts = in_code(text("functionDeclaration"));
            if let Some(arguments) = params.get("arguments") {
                strings_in(arguments, &mut texts);
            }
            texts
        }
        "Debugger.setScriptSource" => in_code(text("scriptSource")),
        "Page.addScriptToEvaluateOnNewDocument" => in_code(text("source")),
        "Page.handleJavaScriptDialog" => vec![text("promptText").to_owned()],
        "Input.dispatchKeyEvent" => vec![text("text").to_owned()],
        "Runtime.setVariableValue" | "Debugger.setVariableValue" | "Debugger.setReturnValue" => {
            let mut texts = Vec::new();
            for key in ["newValue", "value"] {
                if let Some(value) = params.get(key) {
                    strings_in(value, &mut texts);
                }
            }
            texts
        }
        "DOM.setAttributeValue" => vec![text("value").to_owned()],
        "DOM.setNodeValue" => vec![text("value").to_owned()],
        "DOM.setNodeName" => vec![text("name").to_owned()],
        "DOM.setAttributesAsText" => {
            let mut texts = in_markup(text("text"));
            texts.push(text("text").to_owned());
            texts
        }
        "DOM.setOuterHTML" => in_markup(text("outerHTML")),
        "Page.setDocumentContent" => in_markup(text("html")),
        "DOMStorage.setDOMStorageItem" => vec![text("key").to_owned(), text("value").to_owned()],
        "Input.insertText" | "Input.imeSetComposition" => vec![text("text").to_owned()],
        "CSS.setStyleTexts"
        | "CSS.setStyleSheetText"
        | "CSS.setPropertyText"
        | "CSS.setRuleSelector"
        | "CSS.addRule" => {
            let mut all = Vec::new();
            strings_in(params, &mut all);
            all.iter().flat_map(|one| in_code(one)).collect()
        }
        _ => Vec::new(),
    };
    texts.retain(|one| one.chars().count() >= KEPT_FROM);
    texts.sort();
    texts.dedup();
    texts
}

/// Every string anywhere in `value`.
fn strings_in(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::String(text) => into.push(text.clone()),
        Value::Array(values) => values.iter().for_each(|one| strings_in(one, into)),
        Value::Object(map) => map.values().for_each(|one| strings_in(one, into)),
        _ => {}
    }
}

/// The strings code spells out: each quoted string, and each run of a template's text, read
/// as the code means them (`\n` a line's end, `\u{41}` an `A`).
pub(crate) fn in_code(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut texts = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        match chars[at] {
            // A comment is not written into a page.
            '/' if chars.get(at + 1) == Some(&'/') => {
                while at < chars.len() && chars[at] != '\n' {
                    at += 1;
                }
            }
            '/' if chars.get(at + 1) == Some(&'*') => {
                at += 2;
                while at + 1 < chars.len() && !(chars[at] == '*' && chars[at + 1] == '/') {
                    at += 1;
                }
                at += 2;
            }
            quote @ ('\'' | '"' | '`') => {
                let mut text = String::new();
                at += 1;
                while at < chars.len() && chars[at] != quote {
                    if chars[at] == '\\' && at + 1 < chars.len() {
                        at += 1;
                        let (read, used) = escaped(&chars[at..]);
                        text.push_str(&read);
                        at += used;
                        continue;
                    }
                    // A template's `${...}`: what it holds is code, read on its own.
                    if quote == '`' && chars[at] == '$' && chars.get(at + 1) == Some(&'{') {
                        texts.push(std::mem::take(&mut text));
                        let mut depth = 0;
                        let start = at + 2;
                        at += 1;
                        while at < chars.len() {
                            match chars[at] {
                                '{' => depth += 1,
                                '}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            at += 1;
                        }
                        let inner: String = chars[start..at.min(chars.len())].iter().collect();
                        texts.extend(in_code(&inner));
                        at += 1;
                        continue;
                    }
                    text.push(chars[at]);
                    at += 1;
                }
                texts.push(text);
                at += 1;
            }
            _ => at += 1,
        }
    }
    texts.retain(|one| !one.is_empty());
    texts
}

/// One escape after its backslash: what it means, and how many characters it took.
fn escaped(after: &[char]) -> (String, usize) {
    let hex = |digits: &[char]| {
        u32::from_str_radix(&digits.iter().collect::<String>(), 16)
            .ok()
            .and_then(char::from_u32)
    };
    match after[0] {
        'n' => ("\n".into(), 1),
        't' => ("\t".into(), 1),
        'r' => ("\r".into(), 1),
        'x' if after.len() >= 3 => {
            hex(&after[1..3]).map_or((String::new(), 1), |c| (c.to_string(), 3))
        }
        'u' if after.get(1) == Some(&'{') => {
            let end = after
                .iter()
                .position(|&c| c == '}')
                .unwrap_or(after.len() - 1);
            hex(&after[2..end]).map_or((String::new(), 1), |c| (c.to_string(), end + 1))
        }
        'u' if after.len() >= 5 => {
            hex(&after[1..5]).map_or((String::new(), 1), |c| (c.to_string(), 5))
        }
        other => (other.to_string(), 1),
    }
}

/// The texts markup writes: what is between its tags, and each quoted attribute value.
pub(crate) fn in_markup(markup: &str) -> Vec<String> {
    let mut texts = Vec::new();
    let mut text = String::new();
    let mut chars = markup.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '<' {
            text.push(c);
            continue;
        }
        let between = text.trim().to_owned();
        if !between.is_empty() {
            texts.push(between);
        }
        text.clear();
        // Inside a tag: each quoted value.
        while let Some(c) = chars.next() {
            match c {
                '>' => break,
                quote @ ('"' | '\'') => {
                    let value: String = chars.by_ref().take_while(|&c| c != quote).collect();
                    texts.push(value);
                }
                _ => {}
            }
        }
    }
    let between = text.trim().to_owned();
    if !between.is_empty() {
        texts.push(between);
    }
    texts
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{in_code, in_markup, written};

    #[test]
    fn the_strings_code_spells_out_are_what_it_writes() {
        assert_eq!(
            in_code(r#"document.querySelector('#pw').value = "zq7kp-secret"; // not 'this'"#),
            vec!["#pw".to_owned(), "zq7kp-secret".to_owned()]
        );
        assert_eq!(in_code(r"x = 'a\'bA\x42'"), vec!["a'bAB".to_owned()]);
        assert_eq!(
            in_code("el.textContent = `code: ${'inner-1234'} done`"),
            vec![
                "code: ".to_owned(),
                "inner-1234".to_owned(),
                " done".to_owned()
            ]
        );
    }

    #[test]
    fn markup_writes_its_text_and_its_attribute_values() {
        assert_eq!(
            in_markup(r#"<p class="note-x">hello there <b title='tip-1'>you</b></p>"#),
            vec!["note-x", "hello there", "tip-1", "you"]
        );
    }

    #[test]
    fn each_write_of_devtools_gives_its_texts_short_ones_left() {
        assert_eq!(
            written(
                "Runtime.evaluate",
                &json!({ "expression": "f('#pw', 'otp-904211')" })
            ),
            vec!["otp-904211"]
        );
        assert_eq!(
            written(
                "DOM.setAttributeValue",
                &json!({ "nodeId": 3, "name": "value", "value": "hunter22" })
            ),
            vec!["hunter22"]
        );
        assert_eq!(
            written(
                "Runtime.callFunctionOn",
                &json!({ "functionDeclaration": "function (v) { this.value = v }", "arguments": [{ "value": "from-argument" }] })
            ),
            vec!["from-argument"]
        );
        assert_eq!(
            written(
                "DOMStorage.setDOMStorageItem",
                &json!({ "storageId": {}, "key": "token", "value": "eyJhbGciOi" })
            ),
            vec!["eyJhbGciOi"]
        );
        assert!(written("DOM.getDocument", &json!({ "depth": 1 })).is_empty());
        assert_eq!(
            written(
                "Page.addScriptToEvaluateOnNewDocument",
                &json!({ "source": "document.title = 'added-script-1'" })
            ),
            vec!["added-script-1"]
        );
        assert_eq!(
            written(
                "Page.handleJavaScriptDialog",
                &json!({ "accept": true, "promptText": "prompt-answer" })
            ),
            vec!["prompt-answer"]
        );
        assert_eq!(
            written(
                "Input.dispatchKeyEvent",
                &json!({ "type": "char", "text": "inserted-text" })
            ),
            vec!["inserted-text"]
        );
    }
}
