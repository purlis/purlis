// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A card row** (#3669, spec #2897, the sidebar Skills view's variant C on
 * `prototype/skills-sidebar`): a row of a list of rows that act, drawn as a small card whose
 * height follows its lines.
 *
 * - **Its first line** is its name and its mark (a tag), with its own actions (Use) and its
 *   **⋯** at the end. The ⋯ is the row's menu (`menu`), the one a right-click, Shift+F10 or the
 *   menu key opens too: it asks the row for it, so the two are never two menus.
 * - **Its two lines about it** (`RowSummary`): its description, cut cleanly after the second
 *   line, or a reason in its place, in the waiting colour (it needs the person) or the quiet tier
 *   (it is left out). **The whole of it is the row's tooltip**, so nothing cut is lost.
 * - **Its action line** (`RowActions`): its one fix, under the summary.
 * - **The keyboard is the list's**: ↑ and ↓ walk the rows, ← and → its own buttons (Use, the ⋯,
 *   the fix), Enter runs the row, Shift+F10 opens its menu.
 * - **A state never changes its height** (#2800): under the pointer, chosen or under the
 *   keyboard it changes a fill, an edge or a ring, never a padding or a line.
 */
import type { ReactNode } from "react";
import { Ellipsis } from "lucide-react";
import { IconButton } from "../button/Button";
import { askForAMenu } from "../menu/ContextMenu";
import { ListRow, type ListRowProps } from "./List";
import { RowActions, RowLabel, RowSummary, RowTrail } from "./Row";

/** **Why a card row is as it is**, said in place of its description (#3669). */
export interface RowReason {
  says: string;
  /** `warning`: it needs the person (a change to review, a pin no chat reads). `quiet`: it is
   *  left out, and nothing is wrong. */
  tone: "warning" | "quiet";
}

export interface ListCardRowProps<T extends object> extends Omit<
  ListRowProps<T>,
  "variant" | "children" | "tip" | "textValue"
> {
  /** What typing finds it by, and what a reader hears first. */
  textValue: string;
  /** Its words on its first line: its name. */
  name: ReactNode;
  /** What stands right after its name: a tag (its level). */
  mark?: ReactNode;
  /** Its own actions at the end of its first line, before its ⋯ (Use). */
  actions?: ReactNode;
  /** What it is, in two lines at most. */
  description?: string;
  /** Why it is as it is, in place of its description. */
  reason?: RowReason;
  /** Its one fix, on a line of its own under the summary. */
  fix?: ReactNode;
}

/**
 * **One card row** of a `List`. Its tooltip is the whole of its summary, its ⋯ is drawn while it
 * has a `menu`, and `onAction` (on the row or the list) runs it.
 */
export function ListCardRow<T extends object>({
  textValue,
  name,
  mark,
  actions,
  description,
  reason,
  fix,
  menu,
  ...rest
}: ListCardRowProps<T>) {
  const says = reason?.says ?? description;
  return (
    <ListRow {...rest} variant="card" textValue={textValue} menu={menu} tip={says}>
      <RowLabel>{name}</RowLabel>
      {mark}
      {(actions !== undefined || menu !== undefined) && (
        <RowTrail>
          {actions}
          {menu !== undefined && <RowMenuButton name={textValue} />}
        </RowTrail>
      )}
      {says !== undefined && (
        <RowSummary tone={reason?.tone ?? "description"}>{says}</RowSummary>
      )}
      {fix !== undefined && <RowActions>{fix}</RowActions>}
    </ListRow>
  );
}

/**
 * **A row's ⋯** (#3669): it opens the row's own menu (`menu` on the row), at the button, the
 * menu a right-click or Shift+F10 opens. Named for the row it belongs to.
 */
export function RowMenuButton({ name }: { name: string }) {
  return (
    <IconButton
      size="sm"
      variant="ghost"
      aria-label={`More for ${name}`}
      aria-haspopup="menu"
      icon={<Ellipsis aria-hidden="true" />}
      onPress={(press) => askForAMenu(press.target)}
      data-way="menu"
    />
  );
}
