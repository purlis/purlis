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
 * takes the file's lines into the edit, and the same as a list of named buttons under the view
 * for the keyboard (#1862). Neither text is dropped until the person chooses.
 *
 * Like the light editor, this module and everything it imports is a chunk of its own, loaded the
 * first time a file is edited (`settings/RawToml.tsx`), so the window's main bundle carries no
 * CodeMirror (`tools/main-chunk-has-no-codemirror.mjs`, #987).
 */
import { EDITOR_HOLDER } from "../keys/contexts";
import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { bracketMatching } from "@codemirror/language";
import { getChunks, MergeView, type Chunk } from "@codemirror/merge";
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
import { Button, ButtonRow } from "../ui/button/Button";

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
        // The key registry's mark for "an editor has the keyboard here" (#2921).
        {...{ [EDITOR_HOLDER]: "" }}
        className="light-editor text-editor"
        data-testid="text-editor"
        onKeyDown={find.onKeyDown}
      />
      {find.bar}
    </>
  );
}

/** What one difference's button says it takes: the file's lines, or the edit's lines the file
 *  does not have. */
function takeName(merge: MergeView, chunk: Chunk, diskLabel: string): string {
  const lines = (doc: MergeView["a"]["state"]["doc"], from: number, to: number) => {
    const first = doc.lineAt(from).number;
    const last = doc.lineAt(Math.max(from, to - 1)).number;
    return first === last ? `line ${first}` : `lines ${first} to ${last}`;
  };
  if (chunk.fromA < chunk.toA)
    return `Take ${lines(merge.a.state.doc, chunk.fromA, chunk.toA)} from ${diskLabel}`;
  return `Take out ${lines(merge.b.state.doc, chunk.fromB, chunk.toB)}, which ${diskLabel} does not have`;
}

/**
 * Takes the file's side of `chunk` into the edit: what the merge view's own arrow does
 * (`MergeView.revertClicked` in `@codemirror/merge`, which is not exported), for a press that is
 * not the pointer's.
 */
function take(merge: MergeView, chunk: Chunk) {
  const { a, b } = merge;
  let insert = a.state.sliceDoc(chunk.fromA, Math.max(chunk.fromA, chunk.toA - 1));
  if (chunk.fromA !== chunk.toA && chunk.toB <= b.state.doc.length) insert += a.state.lineBreak;
  b.dispatch({
    changes: { from: chunk.fromB, to: Math.min(b.state.doc.length, chunk.toB), insert },
    userEvent: "revert",
  });
}

/** The arrow the gutter draws on each difference: the pointer's way to take it. Out of the Tab
 *  order and hidden from a screen reader, since the list under the view is the keyboard's way
 *  to every difference, not only those scrolled into sight (#1862). */
function arrow(): HTMLElement {
  const button = document.createElement("button");
  button.type = "button";
  button.tabIndex = -1;
  button.title = TAKE_FROM_DISK;
  button.setAttribute("aria-hidden", "true");
  button.textContent = "⇝";
  return button;
}

/** One difference between the file and the edit, and what its button says. */
type Difference = { chunk: Chunk; name: string };

/** Hands the merge view's differences to `heard` whenever they change. */
function differencesTo(heard: { current: (chunks: readonly Chunk[]) => void }): Extension {
  return EditorView.updateListener.of((update) => {
    const now = getChunks(update.state)?.chunks;
    if (now !== undefined && now !== getChunks(update.startState)?.chunks) heard.current(now);
  });
}

/**
 * **The file changed on disk under an edit** (DS-10): the file as it now is (`disk`, read only)
 * beside the edit (`mine`, still typed into), each difference marked, with an arrow that takes
 * the file's side of it into the edit. Both texts stay until the person chooses; the choice is
 * the owner's buttons, not this view's.
 *
 * **From the keyboard** (#1862): under the view, after the edit in the Tab order, a button per
 * difference, named by the lines it takes. Taken, the keyboard goes on to the next difference's
 * button, and back into the edit once nothing differs. None while the edit is read only.
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
  const [differences, setDifferences] = useState<readonly Difference[]>([]);
  /** Names the merge view's differences as they change (set below, once `diskLabel` is known). */
  const changed = useRef<(chunks: readonly Chunk[]) => void>(() => {});
  const takes = useRef<HTMLDivElement>(null);
  /** The difference whose button the keyboard was on when it was taken. */
  const taken = useRef<number>(undefined);
  useEffect(() => {
    told.current = onChange;
  }, [onChange]);
  // Before the view is made below, so its first differences are named too.
  useEffect(() => {
    changed.current = (chunks) => {
      const made = merge.current;
      if (made)
        setDifferences(chunks.map((chunk) => ({ chunk, name: takeName(made, chunk, diskLabel) })));
    };
  }, [diskLabel]);
  useImperativeHandle(ref, () => ({ focus: () => merge.current?.b.focus() }), []);
  // Once a difference is taken, the keyboard goes on to the one now in its place, or the one
  // before it, and back into the edit when none is left.
  useEffect(() => {
    const at = taken.current;
    if (at === undefined) return;
    taken.current = undefined;
    const buttons = takes.current?.querySelectorAll<HTMLElement>("button") ?? [];
    if (buttons.length === 0) merge.current?.b.focus();
    else buttons[Math.min(at, buttons.length - 1)].focus();
  }, [differences]);
  useEffect(() => {
    if (!host.current) return;
    const language = new Compartment();
    const made = new MergeView({
      parent: host.current,
      a: { doc: disk, extensions: reading(diskLabel, language) },
      b: {
        doc: mine,
        extensions: [
          typing(label, language, lock.current, off),
          heard(told),
          differencesTo(changed),
        ],
      },
      gutter: true,
      highlightChanges: true,
      revertControls: off ? undefined : "a-to-b",
      renderRevertControl: arrow,
      collapseUnchanged: { margin: 3, minSize: 6 },
    });
    merge.current = made;
    views.current = [made.a, made.b];
    changed.current(made.chunks);
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
        // The key registry's mark for "an editor has the keyboard here" (#2921).
        {...{ [EDITOR_HOLDER]: "" }}
        role="group"
        aria-label={`${diskLabel}, beside ${label}`}
        className="light-editor merge text-editor"
        data-testid="conflict-merge"
        onKeyDown={find.onKeyDown}
      />
      {!off && differences.length > 0 && (
        <ButtonRow
          size="sm"
          className="merge-takes"
          ref={takes}
          role="group"
          aria-label={`Differences from ${diskLabel}`}
        >
          {differences.map(({ chunk, name }, at) => (
            <Button
              size="sm"
              key={at}
              onPress={() => {
                if (!merge.current) return;
                taken.current = at;
                take(merge.current, chunk);
              }}
            >
              {name}
            </Button>
          ))}
        </ButtonRow>
      )}
      {find.bar}
    </>
  );
}
