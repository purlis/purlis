// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How the light editor is drawn, from the theme's tokens** (`src/theme/`).
 *
 * CodeMirror puts its own stylesheet into the page at run time, outside every `@layer`, so a
 * rule in `App.css` (layer `charter`) can never outrank it (`literals.test.ts`, *every
 * stylesheet is in a layer*). Its colours are therefore replaced here, through
 * `EditorView.theme`, which CodeMirror scopes above its own, and every value is a theme
 * token's custom property: no colour is written in this file. `literals.test.ts` reads it
 * as it reads `App.css`.
 *
 * **The editor is drawn on the terminal's ground, and its code in the syntax tokens** (#2327).
 * The editor's ground and words are `terminal.background` and `terminal.foreground`; each
 * syntax role is its own token, with its italic and bold (`--syntax-<role>-font-style` and
 * `-font-weight`, written by `theme.apply`). Purlis Classic gives each role the terminal colour
 * the editor borrowed before, so it draws as it did.
 *
 * **Order is meaning here.** Two classes can land on one token (a heading's is inherited by what
 * it holds), and the later rule wins. `variable`, `operator` and `punctuation` come first, so
 * they colour only what nothing else does, as the editor's own colour did; emphasis, strong and
 * link come last, so they outrank a role's italic or bold, as they did.
 */
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

export const look = [
  EditorView.theme({
    "&": {
      color: "var(--terminal-foreground)",
      backgroundColor: "var(--terminal-background)",
      height: "100%",
    },
    ".cm-content": { caretColor: "var(--terminal-cursor)" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--terminal-cursor)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
      backgroundColor: "var(--terminal-selection)",
    },
    ".cm-gutters": {
      color: "var(--text-muted)",
      backgroundColor: "var(--surface-raised)",
      borderRight: "1px solid var(--border-subtle)",
    },
    ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "var(--surface-hover)" },
    // The merge view (ADR 0084 §2): a changed line is washed, and the words git's hunk
    // changed inside it are marked more strongly. Base is side A, head is side B. The merge
    // view's own rules name a light or a dark editor, one class more than a theme's rule
    // would; `.cm-editor` (every editor has it) makes up the class, and at equal reach a
    // theme's rule is the one CodeMirror puts after its base rules.
    "&.cm-merge-a .cm-changedLine, .cm-deletedChunk": {
      backgroundColor: "var(--diff-deleted)",
    },
    "&.cm-merge-b .cm-changedLine, .cm-inlineChangedLine": {
      backgroundColor: "var(--diff-inserted)",
    },
    "&.cm-editor.cm-merge-a .cm-changedText, &.cm-editor .cm-deletedChunk .cm-deletedText": {
      background: "var(--diff-deleted-text)",
    },
    "&.cm-editor.cm-merge-b .cm-changedText": {
      background: "var(--diff-inserted-text)",
    },
    "&.cm-merge-b .cm-deletedText": { background: "var(--diff-deleted-text)" },
    "&.cm-editor.cm-merge-a .cm-changedLineGutter, &.cm-editor .cm-deletedLineGutter": {
      background: "var(--danger-base)",
    },
    "&.cm-editor.cm-merge-b .cm-changedLineGutter": {
      background: "var(--state-success)",
    },
    ".cm-inlineChangedLineGutter": { background: "var(--accent-base)" },
    // Find in a file (#946, `find.ts`): every match washed in the terminal's own find colours,
    // the one the bar is on more strongly, as a chat's find marks them.
    ".cm-find-match": { backgroundColor: "var(--terminal-find-match)" },
    ".cm-find-match-active": { backgroundColor: "var(--terminal-find-match-active)" },
    "&.cm-editor .cm-collapsedLines": {
      color: "var(--text-muted)",
      background: "var(--surface-raised)",
    },
  }),
  syntaxHighlighting(
    HighlightStyle.define([
      {
        tag: tags.variableName,
        color: "var(--syntax-variable)",
        fontStyle: "var(--syntax-variable-font-style)",
        fontWeight: "var(--syntax-variable-font-weight)",
      },
      {
        tag: tags.operator,
        color: "var(--syntax-operator)",
        fontStyle: "var(--syntax-operator-font-style)",
        fontWeight: "var(--syntax-operator-font-weight)",
      },
      {
        tag: tags.punctuation,
        color: "var(--syntax-punctuation)",
        fontStyle: "var(--syntax-punctuation-font-style)",
        fontWeight: "var(--syntax-punctuation-font-weight)",
      },
      {
        tag: [tags.keyword, tags.modifier, tags.operatorKeyword],
        color: "var(--syntax-keyword)",
        fontStyle: "var(--syntax-keyword-font-style)",
        fontWeight: "var(--syntax-keyword-font-weight)",
      },
      {
        tag: [tags.string, tags.special(tags.string), tags.regexp],
        color: "var(--syntax-string)",
        fontStyle: "var(--syntax-string-font-style)",
        fontWeight: "var(--syntax-string-font-weight)",
      },
      {
        tag: tags.number,
        color: "var(--syntax-number)",
        fontStyle: "var(--syntax-number-font-style)",
        fontWeight: "var(--syntax-number-font-weight)",
      },
      {
        tag: [tags.bool, tags.null, tags.atom],
        color: "var(--syntax-constant)",
        fontStyle: "var(--syntax-constant-font-style)",
        fontWeight: "var(--syntax-constant-font-weight)",
      },
      {
        tag: [tags.comment, tags.meta],
        color: "var(--syntax-comment)",
        fontStyle: "var(--syntax-comment-font-style)",
        fontWeight: "var(--syntax-comment-font-weight)",
      },
      {
        tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
        color: "var(--syntax-function)",
        fontStyle: "var(--syntax-function-font-style)",
        fontWeight: "var(--syntax-function-font-weight)",
      },
      {
        tag: [tags.typeName, tags.className, tags.namespace],
        color: "var(--syntax-type)",
        fontStyle: "var(--syntax-type-font-style)",
        fontWeight: "var(--syntax-type-font-weight)",
      },
      {
        tag: tags.propertyName,
        color: "var(--syntax-property)",
        fontStyle: "var(--syntax-property-font-style)",
        fontWeight: "var(--syntax-property-font-weight)",
      },
      {
        tag: tags.attributeName,
        color: "var(--syntax-attribute)",
        fontStyle: "var(--syntax-attribute-font-style)",
        fontWeight: "var(--syntax-attribute-font-weight)",
      },
      {
        tag: tags.tagName,
        color: "var(--syntax-tag)",
        fontStyle: "var(--syntax-tag-font-style)",
        fontWeight: "var(--syntax-tag-font-weight)",
      },
      {
        tag: tags.heading,
        color: "var(--syntax-heading)",
        fontStyle: "var(--syntax-heading-font-style)",
        fontWeight: "var(--syntax-heading-font-weight)",
      },
      {
        tag: tags.invalid,
        color: "var(--syntax-invalid)",
        fontStyle: "var(--syntax-invalid-font-style)",
        fontWeight: "var(--syntax-invalid-font-weight)",
      },
      { tag: tags.link, textDecoration: "underline" },
      { tag: tags.emphasis, fontStyle: "italic" },
      { tag: tags.strong, fontWeight: "bold" },
    ]),
  ),
];
