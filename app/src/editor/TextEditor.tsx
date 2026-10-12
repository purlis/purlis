// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The text editor** (DS-10, #1842; D7 of the spec on #1832): a whole file's text, typed into
 * — Settings' *Edit as TOML* and *Edit as JSON*. It is the light editor's CodeMirror
 * (`LightEditor.tsx`) with the keys that edit a text (`@codemirror/commands`, D8): its line
 * numbers, its grammars (`languages.ts`), its look from the theme's tokens (`look.ts`) and its
 * find bar on ⌘F or Ctrl+F (`find.ts`). Tab is not taken: it still moves the keyboard to the
 * next control, as it does in every other box of a form.
 *
 * **The text is its owner's** (a controlled editor): every edit is handed to `onChange`, and a
 * `text` that differs from what the editor holds — a Discard, a file read again — replaces it.
 * Nothing here saves.
 *
 * **When the file changed on disk under an edit**, {@link ConflictMerge} shows both: the file as
 * it now is beside the edit, which is still typed into, with an arrow on each difference that
 * takes the file's lines into the edit. Neither text is dropped until the person chooses.
 *
 * Like the light editor, this module and everything it imports is a chunk of its own, loaded the
 * first time a file is edited (`settings/RawToml.tsx`), so the window's main bundle carries no
 * CodeMirror (`tools/main-chunk-has-no-codemirror.mjs`, #987).
 */
import { useEffect, useImperativeHandle, useRef, type MouseEvent, type Ref } from "react";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { bracketMatching } from "@codemirror/language";
import { MergeView } from "@codemirror/merge";
import {
  Annotation,
  Compartment,
  EditorState,
  Transaction,
  type Extension,
} from "@codemirror/state";
import {
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { colour, useFindIn } from "./LightEditor";
import { findable } from "./find";
import { look } from "./look";

/** What an editor's owner can ask of it: the keyboard, back in its text. */
export type TextEditorHandle = { focus: () => void };

/** Marks a change that came from the owner's `text`, never typed: it is not handed back. */
const fromOwner = Annotation.define<boolean>();

/** The editor's frame: the field's border and the focus ring every control draws. CodeMirror's
 *  own stylesheet outranks the window's (`look.ts`), so they are said here, in tokens. */
const framed = EditorView.theme({
  "&": {
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-md)",
  },
  "&.cm-focused": {
    outline: "var(--focus-ring-width) solid var(--focus-ring)",
    outlineOffset: "var(--focus-ring-offset)",
  },
});

/** Typed into, or not: a read-only project's committed file is read, not written (#826). */
const lockedWhen = (off: boolean): Extension => [
  EditorState.readOnly.of(off),
  EditorView.editable.of(!off),
];

/** What every editable view has: the light editor's reading pieces and the keys that edit. */
function typing(name: string, language: Compartment, lock: Compartment, off: boolean) {
  return [
    EditorView.contentAttributes.of({ "aria-label": name }),
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    highlightSpecialChars(),
    bracketMatching(),
    history(),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    language.of([]),
    lock.of(lockedWhen(off)),
    look,
    framed,
    findable,
  ];
}

/** What the arrow on a difference is called: it takes the file's lines into the edit. */
export const TAKE_FROM_DISK = "Take these lines from the file on disk";

/** The file as it now is, beside an edit: read, never typed into. The merge view names its
 *  arrows from this side's phrases. */
function reading(name: string, language: Compartment) {
  return [
    EditorView.contentAttributes.of({ "aria-label": name }),
    EditorState.phrases.of({ "Revert this chunk": TAKE_FROM_DISK }),
    lineNumbers(),
    highlightSpecialChars(),
    EditorState.readOnly.of(true),
    EditorView.editable.of(false),
    language.of([]),
    look,
    framed,
    findable,
  ];
}

/** Hands each typed change to `onChange`, and none the owner made. */
function heard(onChange: { current: (text: string) => void }): Extension {
  return EditorView.updateListener.of((update) => {
    if (!update.docChanged) return;
    if (update.transactions.every((tr) => tr.annotation(fromOwner) === true)) return;
    onChange.current(update.state.doc.toString());
  });
}

/** Puts `text` in `view` when it differs from what the view holds: not typed, so not undone. */
function follow(view: EditorView, text: string) {
  const now = view.state.doc.toString();
  if (now === text) return;
  view.dispatch({
    changes: { from: 0, to: now.length, insert: text },
    annotations: [fromOwner.of(true), Transaction.addToHistory.of(false)],
  });
}

/** Turns typing in `view` on or off. */
function lockTo(view: EditorView, lock: Compartment, off: boolean) {
  view.dispatch({ effects: lock.reconfigure(lockedWhen(off)) });
}

/**
 * One file's text, typed into. `path` picks its grammar (`charter.toml`, `workspace.json`) and
 * `label` is its name, which a screen reader says for the text (DS-6).
 */
export function TextEditor({
  path,
  label,
  text,
  onChange,
  off = false,
  ref,
}: {
  path: string;
  label: string;
  text: string;
  onChange: (text: string) => void;
  /** Read only: a read-only project's committed file. */
  off?: boolean;
  ref?: Ref<TextEditorHandle>;
}) {
  const host = useRef<HTMLDivElement>(null);
  const views = useRef<readonly EditorView[]>([]);
  const lock = useRef(new Compartment());
  const told = useRef(onChange);
  const find = useFindIn(views, `Find in ${label}`);
  useEffect(() => {
    told.current = onChange;
  }, [onChange]);
  useImperativeHandle(ref, () => ({ focus: () => views.current[0]?.focus() }), []);
  // Made once per file; what it says after that follows `text` and `off` below.
  useEffect(() => {
    if (!host.current) return;
    const language = new Compartment();
    const view = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: text,
        extensions: [typing(label, language, lock.current, off), heard(told)],
      }),
    });
    views.current = [view];
    const stop = colour(path, language, [view]);
    return () => {
      stop();
      views.current = [];
      view.destroy();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `text` and `off` follow below
  }, [path, label]);
  useEffect(() => {
    const [view] = views.current;
    if (view) follow(view, text);
  }, [text]);
  useEffect(() => {
    const [view] = views.current;
    if (view) lockTo(view, lock.current, off);
  }, [off]);
  return (
    <>
      <div
        ref={host}
        className="light-editor text-editor"
        data-testid="text-editor"
        onKeyDown={find.onKeyDown}
      />
      {find.bar}
    </>
  );
}

/**
 * The merge view's arrows act on a mouse press only. Enter or Space on a focused arrow is a click
 * with no pointer behind it (`detail` 0): it is handed on as the press the arrow listens for, so
 * the keyboard takes a difference's lines too.
 */
function pressedFromTheKeyboard(event: MouseEvent<HTMLDivElement>) {
  if (event.detail !== 0) return;
  const arrow = (event.target as Element).closest(".cm-merge-revert > *");
  if (!arrow) return;
  arrow.dispatchEvent(new window.MouseEvent("mousedown", { bubbles: true, cancelable: true }));
}

/**
 * **The file changed on disk under an edit** (DS-10): the file as it now is (`disk`, read only)
 * beside the edit (`mine`, still typed into), each difference marked, with an arrow that takes
 * the file's side of it into the edit. Both texts stay until the person chooses; the choice is
 * the owner's buttons, not this view's.
 */
export function ConflictMerge({
  path,
  label,
  diskLabel,
  disk,
  mine,
  onChange,
  off = false,
  ref,
}: {
  path: string;
  /** The edit's name, as {@link TextEditor}'s. */
  label: string;
  /** The file's side's name. */
  diskLabel: string;
  disk: string;
  mine: string;
  onChange: (text: string) => void;
  off?: boolean;
  ref?: Ref<TextEditorHandle>;
}) {
  const host = useRef<HTMLDivElement>(null);
  const merge = useRef<MergeView>(undefined);
  const views = useRef<readonly EditorView[]>([]);
  const lock = useRef(new Compartment());
  const told = useRef(onChange);
  const find = useFindIn(views, `Find in ${label} and ${diskLabel}`);
  useEffect(() => {
    told.current = onChange;
  }, [onChange]);
  useImperativeHandle(ref, () => ({ focus: () => merge.current?.b.focus() }), []);
  useEffect(() => {
    if (!host.current) return;
    const language = new Compartment();
    const made = new MergeView({
      parent: host.current,
      a: { doc: disk, extensions: reading(diskLabel, language) },
      b: { doc: mine, extensions: [typing(label, language, lock.current, off), heard(told)] },
      gutter: true,
      highlightChanges: true,
      revertControls: off ? undefined : "a-to-b",
      collapseUnchanged: { margin: 3, minSize: 6 },
    });
    merge.current = made;
    views.current = [made.a, made.b];
    const stop = colour(path, language, [made.a, made.b]);
    return () => {
      stop();
      views.current = [];
      merge.current = undefined;
      made.destroy();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the texts follow below
  }, [path, label, diskLabel, off]);
  useEffect(() => {
    if (merge.current) follow(merge.current.a, disk);
  }, [disk]);
  useEffect(() => {
    if (merge.current) follow(merge.current.b, mine);
  }, [mine]);
  return (
    <>
      <div
        ref={host}
        role="group"
        aria-label={`${diskLabel}, beside ${label}`}
        className="light-editor merge text-editor"
        data-testid="conflict-merge"
        onKeyDown={find.onKeyDown}
        onClick={pressedFromTheKeyboard}
      />
      {find.bar}
    </>
  );
}
