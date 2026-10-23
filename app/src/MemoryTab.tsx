// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useState, type FormEvent } from "react";
import Markdown from "react-markdown";
import { LoaderCircle } from "lucide-react";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Choice, Field, SettingActions, SettingRow } from "./settings/components";
import { COMPONENTS } from "./SessionRecordTab";
import {
  commands,
  type MemoryEdited,
  type MemoryScope,
  type MemoryView,
  type PlaneId,
} from "./bindings";
import {
  archiveWhere,
  DRAFT,
  memoryKey,
  scopeKey,
  scopeWord,
  setDraft,
  TITLE_MAX,
  useDraft,
  type Draft,
  type MemoryRef,
} from "./memories";
import { useLentMemoryStores } from "./MemoryEdits";
import { MOVES_WHOLE, movesPublishSaid, storeLabel } from "./memoryMoves";
import { OffWhenReadOnly, useReadOnlySaid } from "./ReadOnlyLine";
import { Button } from "./ui/button/Button";
import { Form } from "./ui/field/Field";

// The body's editor is CodeMirror (#1862), a chunk of its own loaded the first time a memory is
// edited, so the window's main bundle carries none (`tools/main-chunk-has-no-codemirror.mjs`).
const TextEditor = lazy(() =>
  import("./editor/TextEditor").then((module) => ({ default: module.TextEditor })),
);
const ConflictMerge = lazy(() =>
  import("./editor/TextEditor").then((module) => ({ default: module.ConflictMerge })),
);

/**
 * **One memory, in a tab of its own** (SI-9b, ADR 0065 Q2, Q3, Q10).
 *
 * Read, it is where the memory is — its store, its stamp and its file, under the tab's heading,
 * which is its title — and its body rendered as Markdown. The `# title` and `_stamp · kind_`
 * lines are the header's, so the body starts under them (`MemoryView.body`). **No HTML**, for
 * `SessionRecordTab`'s reason: a memory is a file a chat wrote, and raw HTML is dropped.
 *
 * Edited, the same tab is a title field capped at `TITLE_MAX` and the raw body in the text
 * editor (`editor/TextEditor.tsx`, with the Markdown grammar, #1862), with Save and Cancel. What
 * is typed is kept outside the tab (`memories.Draft`), so looking at another tab loses nothing. A
 * save is checked against the file's text as this tab read it, and one the file changed under is
 * refused: the tab says so, shows the body as it is on disk now beside the edit (DS-10's
 * `ConflictMerge`, whose arrows take its lines into the edit), and offers **Reload** (what is on
 * disk now, dropping the edit) or **Overwrite** (the edit, as it then stands, over it).
 *
 * Edit and Delete are the heading's buttons, the catalogue's `memory.edit:<key>` and
 * `memory.delete:<key>` rows (`Views.OWN_ROWS`), so the heading, a row's menu and the palette
 * are one verb. An edit asked for from outside arrives as a draft of `"wanted"`.
 *
 * A new memory's tab ({@link DRAFT} slug) reads nothing and is an editor from the start; its
 * Save is the store's own remember (`memory_create`), and its Cancel closes it.
 *
 * Read, it can also be **moved** to another store (KN-3): {@link MoveMemory}.
 *
 * **On a project purlis opens read-only** (#826, FR-24) a memory is committed, so Save,
 * Overwrite and the Move are off with the project's sentence as their reason; what is typed is
 * kept, and Cancel and Reload still work.
 */
export function MemoryTab({
  plane,
  at,
  changed,
  onSaved,
  onClose,
  onOpenArchive,
}: {
  plane: PlaneId;
  at: MemoryRef;
  /** Bumped when the plane changes on disk or a memory is written: the tab reads again. */
  changed: number;
  /** A save, a create or a move wrote the memory: the tab's name, key and every list follow
   *  it. */
  onSaved: (memory: MemoryView) => void;
  /** The tab asks to be closed: a new memory's Cancel. */
  onClose: () => void;
  /** Open the store's archive (`memory.archived:<store>`): a gone memory's way out. */
  onOpenArchive?: () => void;
}) {
  const key = memoryKey(at);
  const isNew = at.slug === DRAFT;
  const [said, setSaid] = useState<{ memory?: MemoryView | null; trouble?: string }>();
  const draft = useDraft(plane, key);
  const [saving, setSaving] = useState(false);
  const [refused, setRefused] = useState<string>();
  /** Why the project is read-only, when it is: Save and Overwrite are off with it (#826). */
  const readOnly = useReadOnlySaid();

  const { scope, slug } = at;
  useEffect(() => {
    if (isNew) return;
    let gone = false;
    void commands
      .memoryRead(plane, scope, slug)
      .then((answer) => {
        if (gone) return;
        setSaid(
          answer.status === "error" ? { trouble: answer.error } : { memory: answer.data ?? null },
        );
      })
      .catch((err: unknown) => {
        if (!gone) setSaid({ trouble: String(err) });
      });
    return () => {
      gone = true;
    };
    // `scope` by value: a fresh object with the same store must not read again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plane, key, isNew, changed]);

  const memory = said?.memory ?? undefined;
  // An edit asked for before the read arrived — or a new memory, which is only ever an editor
  // — becomes a draft of what there is to edit.
  useEffect(() => {
    if (isNew && draft === undefined) setDraft(plane, key, { title: "", body: "", base: "" });
    else if (draft === "wanted" && memory !== undefined)
      setDraft(plane, key, { title: memory.title, body: memory.body, base: memory.text });
  }, [draft, isNew, key, memory, plane]);

  const editing = typeof draft === "object" ? draft : undefined;

  if (editing !== undefined) {
    const change = (to: Partial<Draft>) => setDraft(plane, key, { ...editing, ...to });
    const answered = (outcome: MemoryEdited) => {
      if (outcome.kind === "stale") {
        change({ stale: { now: outcome.now } });
        return;
      }
      saved(outcome.memory);
    };
    const saved = (written: MemoryView) => {
      setSaid({ memory: written });
      setDraft(plane, key, undefined);
      onSaved(written);
    };
    const save = async (overwrite: boolean) => {
      setSaving(true);
      setRefused(undefined);
      try {
        if (isNew) {
          const answer = await commands.memoryCreate(plane, scope, editing.title, editing.body);
          if (answer.status === "error") setRefused(answer.error);
          else saved(answer.data);
        } else {
          const answer = await commands.memoryEdit(
            plane,
            scope,
            slug,
            editing.title,
            editing.body,
            editing.base,
            overwrite,
          );
          if (answer.status === "error") setRefused(answer.error);
          else answered(answer.data);
        }
      } catch (err: unknown) {
        setRefused(String(err));
      } finally {
        setSaving(false);
      }
    };
    const submit = (event: FormEvent) => {
      event.preventDefault();
      if (readOnly === undefined) void save(false);
    };
    const stale = editing.stale;
    return (
      <Form className="memory-editor" onSubmit={submit} aria-label={`Editing ${key}`}>
        <Meta at={at} memory={memory} />
        <SettingRow
          label="Title"
          help={`${isNew ? "The body's first line, when left empty · " : ""}${editing.title.length} / ${TITLE_MAX}`}
          control={(ids) => (
            <Field
              ids={ids}
              kind="text"
              value={editing.title}
              maxLength={TITLE_MAX}
              onChange={(title) => change({ title })}
            />
          )}
        />
        {/* The body is free Markdown, a page and not a value: the text editor, under its name at
            the row's width. While the memory changed on disk, the body as it now is stands
            beside the edit, and both are kept until Reload or Overwrite. */}
        <SettingRow
          label="Body"
          grouped
          control={() => (
            <Suspense
              fallback={
                <p className="ui-setting-help" aria-busy="true">
                  Opening the editor…
                </p>
              }
            >
              {stale?.now ? (
                <ConflictMerge
                  path={`${slug}.md`}
                  label="Body"
                  diskLabel="Body, on disk now"
                  disk={stale.now.body}
                  mine={editing.body}
                  onChange={(body) => change({ body })}
                />
              ) : (
                <TextEditor
                  path={`${slug}.md`}
                  label="Body"
                  text={editing.body}
                  onChange={(body) => change({ body })}
                />
              )}
            </Suspense>
          )}
        />
        {stale !== undefined && (
          <div className="trouble memory-stale" role="alert">
            {stale.now === null
              ? "This memory is not there any more — it was archived or removed since you opened it. Nothing was saved."
              : "This memory changed on disk since you opened it, so nothing was saved. It is beside your edit as it now is: take what you want from it, then Overwrite to save your edit over it, or Reload to take it up and drop your edit."}{" "}
            <SettingActions>
              <Button
                onPress={() => {
                  setDraft(plane, key, undefined);
                  if (stale.now !== undefined) setSaid({ memory: stale.now });
                }}
              >
                Reload
              </Button>
              {stale.now !== null && (
                <Button
                  variant="danger"
                  isDisabled={saving || readOnly !== undefined}
                  tooltip={readOnly}
                  onPress={() => void save(true)}
                >
                  Overwrite
                </Button>
              )}
            </SettingActions>
          </div>
        )}
        {refused !== undefined && (
          <p className="trouble" role="alert">
            {refused}
          </p>
        )}
        <SettingActions>
          <Button
            variant="primary"
            type="submit"
            isDisabled={saving || stale !== undefined || readOnly !== undefined}
            tooltip={readOnly}
          >
            Save
          </Button>
          <Button
            onPress={() => {
              setDraft(plane, key, undefined);
              setRefused(undefined);
              if (isNew) onClose();
            }}
          >
            Cancel
          </Button>
        </SettingActions>
      </Form>
    );
  }

  if (said === undefined) {
    return (
      <p className="pending" aria-busy="true">
        <LoaderCircle className="node-icon spinning" />
        Reading the memory…
      </p>
    );
  }
  if (said.trouble !== undefined) {
    return (
      <p className="trouble" role="alert">
        {said.trouble}
      </p>
    );
  }
  if (memory === undefined) {
    // **Not an error**: a tab the last launch left open can name a memory archived since.
    // So its way out is the store's archive, where a deleted memory went (#1191).
    return (
      <EmptyState
        headline="This memory is not here any more"
        body={`Nothing is called ${slug} in ${scopeWord(scope)}'s memory now.`}
        action={
          onOpenArchive && (
            <Button variant="primary" onPress={onOpenArchive}>
              {`Open ${archiveWhere(scope)}`}
            </Button>
          )
        }
        testid="view-gone"
      />
    );
  }
  return (
    <>
      <Meta at={at} memory={memory} />
      <article className="memory-body release-notes" data-testid="memory-body">
        <Markdown skipHtml components={COMPONENTS}>
          {memory.body}
        </Markdown>
      </article>
      <MoveMemory plane={plane} at={at} onMoved={onSaved} />
    </>
  );
}

/** Where a memory is: its store as a badge, its stamp and its file. */
function Meta({ at, memory }: { at: MemoryRef; memory?: MemoryView }) {
  return (
    <p className="memory-meta" data-testid="memory-meta">
      <span className="memory-scope">{memory?.place ?? scopeWord(at.scope)}</span>
      {memory === undefined ? (
        <span>not written yet</span>
      ) : (
        <>
          {memory.stamp !== "" && <span>{memory.stamp}</span>}
          <code>{memory.path}</code>
        </>
      )}
    </p>
  );
}

/**
 * **Move a memory to another store** (KN-3): a workspace's journal, a persona's memory or shared
 * memory. The file moves whole — its title and stamp with it, nothing copied — and the tab
 * follows it there (`onMoved`, which a save calls too). A journal name moved away and back
 * comes back to the minute. Persona and shared memory are published with the project, and so is
 * a LIVE workspace's journal, which the help line says, naming the LIVE ones among the stores
 * (`memoryMoves.movesPublishSaid`, as the Move rows say it): a move out of a LOCAL journal into
 * one of them publishes it with the next save.
 *
 * **The pick is held, and only the button moves** (`docs/ui-primitives.md`'s held pick). A move
 * has an Undo now (`useMemoryEdits`, #1190), but only for a few seconds, and a move out of a
 * LOCAL journal publishes the memory with the next save, so it is still never made by a pick
 * passed through on the way to another. A move a store refuses — one
 * already holding a memory of that name, one charter may not write — says why, and nothing moved.
 */
function MoveMemory({
  plane,
  at,
  onMoved,
}: {
  plane: PlaneId;
  at: MemoryRef;
  onMoved: (memory: MemoryView) => void;
}) {
  const [scopes, setScopes] = useState<MemoryScope[]>();
  const [picked, setPicked] = useState("");
  const [moving, setMoving] = useState(false);
  const [refused, setRefused] = useState<string>();
  const here = scopeKey(at.scope);
  // Which workspaces are LIVE, as the window lends them: none outside a project's window.
  const { live } = useLentMemoryStores();

  useEffect(() => {
    let gone = false;
    void commands
      .memoryScopes(plane)
      .then((answer) => {
        if (!gone && answer.status === "ok" && Array.isArray(answer.data)) setScopes(answer.data);
      })
      .catch(() => {});
    return () => {
      gone = true;
    };
  }, [plane]);

  const others = (scopes ?? []).filter((scope) => scopeKey(scope) !== here);
  if (others.length === 0) return null;
  const target = others.find((scope) => scopeKey(scope) === picked);

  const move = async () => {
    if (target === undefined) return;
    setMoving(true);
    setRefused(undefined);
    try {
      const answer = await commands.memoryMove(plane, at.scope, at.slug, target, null);
      if (answer.status === "error") setRefused(answer.error);
      else {
        setPicked("");
        onMoved(answer.data);
      }
    } catch (err: unknown) {
      setRefused(String(err));
    } finally {
      setMoving(false);
    }
  };

  return (
    <div className="memory-move">
      {/* A move writes two committed stores: off on a read-only project, with why (#826). */}
      <OffWhenReadOnly>
        <SettingRow
          label="Move to"
          help={`${MOVES_WHOLE} ${movesPublishSaid(others, live)}`}
          control={(ids) => (
            <Choice
              ids={ids}
              kind="select"
              unset="Pick a store…"
              options={others.map((scope) => ({
                value: scopeKey(scope),
                label: storeLabel(scope),
              }))}
              value={picked}
              onValueChange={setPicked}
            />
          )}
        />
        {refused !== undefined && (
          <p className="trouble" role="alert">
            {refused}
          </p>
        )}
        <SettingActions>
          <Button
            variant="primary"
            isDisabled={target === undefined || moving}
            onPress={() => void move()}
          >
            Move
          </Button>
        </SettingActions>
      </OffWhenReadOnly>
    </div>
  );
}
