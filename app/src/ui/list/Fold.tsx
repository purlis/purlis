// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What folds in a list** (#3670, spec #2897, the sidebar Skills view on
 * `prototype/skills-sidebar` and the studio's Library on `prototype/skill-studio-frame`): a group
 * of rows whose name folds them away, and a run of rows folded into one quiet line.
 *
 * - **A section that folds** (`ListFoldSection`): its name is a row the keyboard reaches, with
 *   its count at its end and an optional tone (`waiting`, for what needs the person). Enter,
 *   Space, → and ← fold and unfold it, and `aria-expanded` says which. It can start folded; its
 *   owner may hold what is folded (`folded`, `onFoldedChange`), so a view keeps it across a
 *   relaunch.
 * - **A run folded to one quiet line** (`ListRun`): its rows drawn as one row in the quiet tier,
 *   a mark and "N …", which unfolds them in place, followed by an optional closing line. **A row
 *   of the run that must show** (it needs the person, it clashes, it matches the filter:
 *   `standsOut`) is drawn on its own, before the line, and never folded.
 * - **Folding moves only what is under it**: the rows above keep their place. The twist is the
 *   tree's own (`TwistMark`), never a chevron drawn here.
 *
 * Both live in a `List` of rows that act. **In a list that chooses** (`selectionMode`, the
 * studio's Library) a fold's row and a run's closing line are never chosen (#3769): they say so
 * themselves (`isDisabled`, under the list's `disabledBehavior="selection"`, its default there),
 * so they are still reached, a press or a key still folds, and Select all leaves them out. Their
 * fold rows run no `onAction` of the list's: a press on one folds it, and nothing else.
 *
 * **What is folded is held by the fold, or by its owner** (`folded`, #3769): folding a section
 * unmounts what is under it, so a run inside it starts afresh as the section unfolds, unless its
 * owner holds the run's `folded`. The family keeps no state of its own past its rows: a view
 * that cares (the Skills view, the Library) already holds every fold, to keep it across a
 * relaunch.
 */
import { useContext, useLayoutEffect, useState, type ReactNode } from "react";
import {
  GridListItem as AriaGridListItem,
  GridListSection as AriaGridListSection,
  type Key,
} from "react-aria-components";
import { TwistMark } from "../tree/Tree";
import { groupHeader, ListChooses, ListFolds, ListSize, useSaidOnRow } from "./List";

/** **Folded or not**, held by its owner (`folded`) or here, starting as `defaultFolded`. */
function useFolded(
  folded: boolean | undefined,
  defaultFolded: boolean,
  onFoldedChange: ((folded: boolean) => void) | undefined,
): [boolean, (folded: boolean) => void] {
  const [own, setOwn] = useState(defaultFolded);
  const now = folded ?? own;
  return [
    now,
    (next: boolean) => {
      if (folded === undefined) setOwn(next);
      onFoldedChange?.(next);
    },
  ];
}

/** What a fold takes, beside its words. */
interface Folding {
  /** Whether it is folded, where its owner holds it. */
  folded?: boolean;
  /** Whether it starts folded, where it holds it itself. */
  defaultFolded?: boolean;
  /** Hears it fold (`true`) and unfold (`false`). */
  onFoldedChange?: (folded: boolean) => void;
}

/**
 * **A row that folds what is under it**: a section's name, or a run's quiet line. Enter and Space
 * run it (React Aria's action); → and ← are heard by the list (`List`'s keys), through the
 * toggle it leaves there under its key.
 */
function FoldRow({
  id,
  textValue,
  open,
  onOpen,
  part,
  tone,
  named = false,
  children,
}: {
  id: string;
  textValue: string;
  open: boolean;
  onOpen: (open: boolean) => void;
  part: "header" | "run";
  tone?: "waiting";
  named?: boolean;
  children: ReactNode;
}) {
  const folds = useContext(ListFolds);
  const size = useContext(ListSize);
  const chooses = useContext(ListChooses);
  useLayoutEffect(() => {
    folds.set(id, onOpen);
    return () => {
      folds.delete(id);
    };
  }, [folds, id, onOpen]);
  const said = useSaidOnRow("aria-expanded", open ? "true" : "false");
  return (
    <AriaGridListItem
      id={id}
      textValue={textValue}
      ref={said}
      isDisabled={chooses}
      onAction={() => onOpen(!open)}
      {...(part === "header"
        ? groupHeader("list", named)
        : { "data-scope": "list", "data-part": "run" })}
      data-size={size}
      data-fold={id}
      data-expanded={open || undefined}
      data-tone={tone}
    >
      {children}
    </AriaGridListItem>
  );
}

export interface ListFoldSectionProps extends Folding {
  /** Its key among the list's rows and groups. */
  id: string;
  /** Its name, drawn over its rows and heard as the group's name and its row's. */
  title: string;
  /** How many rows it holds, at the end of its name: seen, and heard after its name. */
  count?: number;
  /** **`waiting`**: what needs the person (Needs you), in the waiting colour. */
  tone?: "waiting";
  /** The name is data (a file's, a day's), drawn as it is written; else purlis's own label. */
  named?: boolean;
  children: ReactNode;
}

/**
 * **A group of rows whose name folds them** (#3670): its name a row with its twist, its count at
 * its end; folded, only its name is drawn, and the rows after it move up.
 */
export function ListFoldSection({
  id,
  title,
  count,
  tone,
  named = false,
  folded,
  defaultFolded = false,
  onFoldedChange,
  children,
}: ListFoldSectionProps) {
  const [isFolded, setFolded] = useFolded(folded, defaultFolded, onFoldedChange);
  return (
    <AriaGridListSection
      id={id}
      aria-label={title}
      data-scope="list"
      data-part="section"
      data-folded={isFolded || undefined}
    >
      <FoldRow
        id={`${id}:fold`}
        textValue={count === undefined ? title : `${title}, ${count}`}
        open={!isFolded}
        onOpen={(open) => setFolded(!open)}
        part="header"
        tone={tone}
        named={named}
      >
        <TwistMark />
        <span data-part="title">{title}</span>
        {count !== undefined && (
          <span data-scope="list" data-part="count" data-count="" aria-hidden="true">
            {count}
          </span>
        )}
      </FoldRow>
      {!isFolded && children}
    </AriaGridListSection>
  );
}

export interface ListRunProps<T> extends Folding {
  /** Its key among the list's rows: the quiet line's is `<id>:fold`, its closing line's
   *  `<id>:end`. */
  id: string;
  /** Every row of the run, in its order. */
  items: readonly T[];
  /** A row drawn on its own, outside the fold: it needs the person, it clashes, it matches the
   *  filter. */
  standsOut?: (item: T) => boolean;
  /** A row of the run, as a `ListRow` (or `ListCardRow`) with its own `id`. */
  children: (item: T) => ReactNode;
  /** What the quiet line says of how many it folds: "6 built-in skills". */
  says: (count: number) => string;
  /** The mark it leads with (a small purlis mark): decoration, its words say what it is. */
  mark?: ReactNode;
  /** What is said after the rows once they are unfolded, and what may be done about it. */
  closing?: { says: string; children: ReactNode };
}

/**
 * **A run of rows folded to one quiet line** (#3670): folded by default. Pressing the line
 * unfolds the rows in place, under it, and its closing line after them; the rows of the run that
 * must show are drawn on their own before it. With nothing left to fold, no line is drawn.
 */
export function ListRun<T>({
  id,
  items,
  standsOut,
  children,
  says,
  mark,
  closing,
  folded,
  defaultFolded = true,
  onFoldedChange,
}: ListRunProps<T>) {
  const [isFolded, setFolded] = useFolded(folded, defaultFolded, onFoldedChange);
  const out: T[] = [];
  const kept: T[] = [];
  for (const item of items) (standsOut?.(item) === true ? out : kept).push(item);
  const line = says(kept.length);
  return (
    <>
      {out.map(children)}
      {kept.length > 0 && (
        <FoldRow
          id={`${id}:fold`}
          textValue={line}
          open={!isFolded}
          onOpen={(open) => setFolded(!open)}
          part="run"
        >
          {mark !== undefined && (
            <span data-scope="list" data-part="lead">
              {mark}
            </span>
          )}
          <span data-scope="list" data-part="label">
            {line}
          </span>
          <TwistMark />
        </FoldRow>
      )}
      {kept.length > 0 && !isFolded && kept.map(children)}
      {kept.length > 0 && !isFolded && closing !== undefined && (
        <RunEnd id={`${id}:end`} says={closing.says}>
          {closing.children}
        </RunEnd>
      )}
    </>
  );
}

/** **A run's closing line**: what is said after its rows, and what may be done about it. */
function RunEnd({ id, says, children }: { id: Key; says: string; children: ReactNode }) {
  const folds = useContext(ListFolds);
  const chooses = useContext(ListChooses);
  useLayoutEffect(() => {
    // It runs nothing of the list's: a press on it is a press on what it holds.
    const key = String(id);
    folds.set(key, undefined);
    return () => {
      folds.delete(key);
    };
  }, [folds, id]);
  return (
    <AriaGridListItem
      id={id}
      textValue={says}
      isDisabled={chooses}
      data-scope="list"
      data-part="run"
      data-end=""
    >
      <span data-scope="list" data-part="label">
        {says}
      </span>
      {children}
    </AriaGridListItem>
  );
}
