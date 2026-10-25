// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useId, useRef, useState } from "react";
import type { TextEditorHandle } from "../editor/TextEditor";
import type { Shown } from "./fileControls";
import { Button } from "../ui/button/Button";

/** The editor, and the merge view a file changed on disk opens: CodeMirror, in a chunk of its
 *  own fetched when a file is first edited, never in the window's main bundle (#987). */
const TextEditor = lazy(() =>
  import("../editor/TextEditor").then((module) => ({ default: module.TextEditor })),
);
const ConflictMerge = lazy(() =>
  import("../editor/TextEditor").then((module) => ({ default: module.ConflictMerge })),
);

/**
 * **Edit as TOML** (SE-19, #1169; the spec on #558, V89d): a file of a level, as its whole text.
 * The Workspace level's `workspace.json` is the same, as **Edit as JSON** (NO-7, #1232): its
 * Save goes through the core's check of a manifest, against the text the edit began from.
 * One link per file sits at the foot of the nav; pressing it shows the file's text in the right
 * column, in place of a group, with an explicit Save and Discard. Nothing is written as it is
 * typed: half a table header is not a file to write. The text is typed into the light editor's
 * CodeMirror (DS-10, #1842: `editor/TextEditor.tsx`), with line numbers, the file's colours and
 * find on ⌘F or Ctrl+F.
 *
 * **Nothing here decides what a file may say.** Save sends the whole text against the text the
 * edit began from, and the core checks it with the rules it reads the file with
 * (`purlis_core::settings::save`): text that does not parse, or that the next read would refuse,
 * is refused in the core's words and nothing is written.
 *
 * **A file changed outside the tab** — by hand, by a chat — is read again by the level's driver.
 * While no edit is under way the editor shows the file as it now is. An edit is kept, and says
 * the file moved under it, for as long as it lasts — typed back to what it began from included:
 * every keystroke of it keeps the text it began from. While the file has moved under it, the
 * editor becomes the merge view (DS-10): the file as it now is beside the edit, which is still
 * typed into, and Save is off, so an outside edit is never overwritten unseen. Both texts stay
 * until the person chooses: **Keep my edit** makes the edit one of the file as it now is, to be
 * saved over it; **Discard** takes up the file as it now is. Were a save made against the text
 * the edit began from all the same, the core refuses it as changed on disk.
 *
 * **A save goes through the level's driver** (`Driven.writeRaw`), in the queue every setting's
 * write is in: what it wrote is what the next edit begins from, at once, and the last change's
 * Undo is gone.
 */

/** One file a level offers as its whole text. */
export type RawFile = {
  /** Which file, as the level names it: `shared`, `local` or `workspace`. */
  id: string;
  /** What the text is written in: a project's files are TOML, a workspace's manifest JSON. */
  as: "TOML" | "JSON";
  /** The file as it was last read. */
  file: Shown;
  /** The sentence on where the file is kept and who sees it. */
  kept: string;
  /** Writes `text` over the file whose text was `base` (`null`: not there yet); answers why
   *  nothing was written, or `undefined` once it was. */
  save: (base: string | null, text: string) => Promise<readonly string[] | undefined>;
};

/**
 * An edit of one file, kept by the tab so it survives a look at a group: the text, the file's
 * text it began from (`base`, `null` for a file not there yet), and why its last Save was
 * refused, while it was (#1198): the refusal is the edit's, so it comes back with it. It lasts
 * until it is saved or discarded.
 */
export type RawDraft = { base: string | null; text: string; refused?: readonly string[] };

/** The file's name, without its folder. */
export function named(file: Shown): string {
  return file.file.split("/").pop() ?? file.file;
}

/** The links at the foot of the nav: one per file, the one on screen marked current, under
 *  what the level's files are written in (a level's files share one). */
export function RawLinks({
  files,
  editing,
  onEdit,
}: {
  files: readonly RawFile[];
  editing: string | undefined;
  onEdit: (id: string) => void;
}) {
  const label = useId();
  return (
    <div className="ui-settings-raw" role="group" aria-labelledby={label}>
      <p className="ui-settings-raw-label" id={label}>
        {`Edit as ${files[0]?.as ?? "TOML"}`}
      </p>
      {files.map((one) => (
        <button
          key={one.id}
          type="button"
          className="ui-settings-raw-file"
          // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
          tabIndex={0}
          aria-current={one.id === editing ? "true" : undefined}
          onClick={() => onEdit(one.id)}
        >
          <code>{named(one.file)}</code>
        </button>
      ))}
    </div>
  );
}

/** One file's text, in the right column, with its Save and Discard. */
export function RawEditor({
  raw,
  draft,
  onDraft,
  off = false,
}: {
  raw: RawFile;
  draft: RawDraft | undefined;
  /** Sets the edit, or works it out from the edit as it now is. */
  onDraft: (
    to: RawDraft | undefined | ((now: RawDraft | undefined) => RawDraft | undefined),
  ) => void;
  /** Read only: the file is committed and the project is read-only (#826). Its buttons are off
   *  by the fieldset around them; the text is CodeMirror's, which a fieldset cannot turn off. */
  off?: boolean;
}) {
  const heading = useId();
  const box = useRef<TextEditorHandle>(null);
  const [saving, setSaving] = useState(false);
  const { file } = raw;
  const name = named(file);
  const label = `${name}, as ${raw.as}`;
  const base = file.exists ? file.text : null;
  const text = draft?.text ?? file.text;
  const dirty = draft !== undefined && draft.text !== (draft.base ?? "");
  /** The file is no longer what the edit began from. */
  const moved = draft !== undefined && draft.base !== base;
  const refused = draft?.refused;
  // The keyboard goes back to the text once it is drawn: Keep my edit and Discard swap the
  // merge view for the editor, so the text to focus is the one the next render draws.
  const [focusing, setFocusing] = useState(0);
  const focus = () => setFocusing((n) => n + 1);
  useEffect(() => {
    if (focusing > 0) box.current?.focus();
  }, [focusing]);
  // An edit keeps the text it began from for as long as it lasts.
  const typed = (to: string) =>
    onDraft((now) => ({ base: now ? now.base : base, text: to, refused: now?.refused }));

  const save = async () => {
    if (!draft || !dirty || moved) return;
    const sent = draft.text;
    setSaving(true);
    const why = await raw.save(draft.base, sent).catch((err: unknown) => [String(err)]);
    setSaving(false);
    // Once written, the file is what was written: the edit is over, unless more was typed
    // while the save was on its way, which is then an edit of what was written. A refusal is
    // kept with the edit, whatever was typed since.
    if (why === undefined)
      onDraft((now) => (now && now.text !== sent ? { base: sent, text: now.text } : undefined));
    else onDraft((now) => (now ? { ...now, refused: why } : now));
    focus();
  };

  return (
    <section className="ui-setting-group ui-raw" aria-labelledby={heading}>
      <h2 id={heading}>{name}</h2>
      <p className="ui-setting-help">
        {raw.as === "TOML" ? "The whole file, comments and all." : "The whole file."} {raw.kept}
        {!file.exists && " Not created yet: the first save creates it."} Nothing is written until
        you save, and purlis refuses text it would not read.
      </p>
      {/* Mounted whatever it says, so a screen reader hears its first sentence (#1198). */}
      <p className="ui-setting-help" role="status">
        {moved &&
          `${name} changed on disk since this edit began. The file as it now is is beside your edit, and both are kept: take what you want from it, then Keep my edit to save yours over it, or Discard to take up the file.`}
      </p>
      <Suspense
        fallback={
          <p className="ui-setting-help" aria-busy="true">
            Opening the editor…
          </p>
        }
      >
        {moved ? (
          <ConflictMerge
            ref={box}
            path={file.file}
            label={label}
            diskLabel={`${name}, on disk now`}
            disk={base ?? ""}
            mine={text}
            onChange={typed}
            off={off}
          />
        ) : (
          <TextEditor
            ref={box}
            path={file.file}
            label={label}
            text={text}
            onChange={typed}
            off={off}
          />
        )}
      </Suspense>
      <div className="ui-raw-actions">
        <Button isDisabled={!dirty || moved || saving} onPress={() => void save()}>
          {saving ? "Saving…" : `Save ${name}`}
        </Button>
        {moved && (
          <Button
            isDisabled={saving}
            onPress={() => {
              // The edit, as one of the file as it now is: Save writes it over the file.
              onDraft((now) => now && { base, text: now.text });
              focus();
            }}
          >
            Keep my edit
          </Button>
        )}
        <Button
          isDisabled={!(dirty || moved) || saving}
          onPress={() => {
            onDraft(undefined);
            focus();
          }}
        >
          Discard
        </Button>
      </div>
      {refused && (
        <div className="ui-setting-error" role="alert">
          <p>Nothing was saved:</p>
          {refused.map((why, at) => (
            <p key={at}>{why}</p>
          ))}
        </div>
      )}
    </section>
  );
}
