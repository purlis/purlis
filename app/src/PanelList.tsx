// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useRef, useState, type ReactNode } from "react";
import { HeldPopover } from "./ui/popover/Popover";
import clsx from "clsx";
import {
  CircleDashed,
  Circle,
  FileText,
  FolderGit2,
  GitBranch,
  KeyRound,
  Search,
  Star,
  TriangleAlert,
  UserRound,
} from "lucide-react";
import { PersonaMark } from "./PersonaMark";
import type { Offer } from "./actions";
import type { PanelEmpty, PanelRow, RowAction } from "./bindings";
import { byDay, isDated } from "./dayGroups";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Button } from "./ui/button/Button";
import { Checkbox } from "./ui/choice/Checkbox";
import { TextField } from "./ui/field/Field";
import { List, ListRow, ListRowSection, type RowMenu } from "./ui/list/List";
import { InPanelTop, useInPanelView } from "./ui/panel/Panel";
import { RowBelow, RowDescription, RowLabel, RowLead, RowTrail } from "./ui/list/Row";

/**
 * The list a panel is drawn from, and the one the operator asked for by name.
 *
 * His four sentences about the todos panel, each of which is a property below:
 *
 * > *"rows are too long, they are wrapping in 3 lines — should be shortened automatically"*
 * > *"on clicking we should show full body — like persona description"*
 * > *"max 10 or 20 todos should be loaded, with scroll inside panel and load more"*
 * > *"search input should be visible when count is bigger than N"*
 *
 * …and the sentence that made it a primitive rather than four fixes to one panel: *"same
 * load-more and search for personas will be better to have"*.
 *
 * # It knows what a row is, and nothing else
 *
 * A [`PanelRow`] is `purlis_core::panel::Row` on the wire. This component takes rows and never
 * asks where they came from, which is what makes it the vocabulary a panel is drawn *with*
 * rather than advice a panel is written *to*. Its consumers on the day it was written:
 *
 * - **charter's todos panel** and **charter's personas panel**, which are contributions now
 *   (`Panels.tsx`);
 * - **a persona's memories**, in the persona's own tab — a list in a view rather than in a
 *   panel, searched and paged by this same code, answered by `open_view` like any view;
 * - **a contributed panel**, whose rows an extension declared in its manifest and which gets
 *   every one of the four properties for free.
 *
 * That last one is the test of whether the contract was worth defining, and the third is the
 * test of whether this is a primitive or a panel with a general-sounding name.
 *
 * # Shortening is done twice, and the two do different work
 *
 * **CSS does the pixel-exact fit** — one line, `text-overflow: ellipsis` (`App.css`). It is the
 * only thing that can, because the width is a region the operator drags and a font the theme
 * picks, and neither is a number this component has.
 *
 * **[`SHORTEST`] does the DOM.** A 400-character title inside an ellipsis is drawn correctly and
 * is still 400 characters in the accessible name, in the tree and in every `getByText` — so a
 * row is cut to a generous line's worth before it is rendered, and the whole of it lives in the
 * card the row opens. That cut is what a jsdom test can see: `docs/ui-primitives.md` records
 * that jsdom lays nothing out and every element measures zero, so a claim about wrapping can
 * only be a claim about characters here, and a claim about pixels is the scenario run's.
 */
export const SHORTEST = 64;

/** What a persona's row runs, up to the persona's name (`actions.ts`). */
const SHOWS_PERSONA = "persona.show:";

/** How many rows are drawn before the list stops and offers the rest. */
export const PAGE = 12;

/**
 * Every mark in `purlis_core::panel::Mark`, as the glyph charter already ships for it.
 *
 * **A word out of a closed set, resolved here** — ADR 0041's third crossing is *a reference to
 * a file*, and an icon is the shape that invites one. A word this map does not know draws the
 * plain circle rather than nothing, which is `PlaneView`'s `MARKS` rule: a missing icon is
 * cosmetic and a missing row is not.
 */
const MARKS: Record<string, React.ComponentType<{ className?: string }>> = {
  todo: CircleDashed,
  persona: UserRound,
  repo: FolderGit2,
  piece: GitBranch,
  note: FileText,
  trouble: TriangleAlert,
  vault: KeyRound,
  dot: Circle,
};

/**
 * A context menu on a row, supplied by the panel drawing it.
 *
 * **A panel's and not this component's, because a menu names what the row is ABOUT.**
 * `Menus.tsx` takes `{ on: "persona", persona }`, and there is no way to derive that from a row:
 * a row is words, a key and at most a catalogue id. charter's personas panel knows its rows are
 * personas and says so; a contributed panel cannot, and gets no context menu — which is the same
 * asymmetry `purlis_core::panel`'s header names, one notch smaller, and is written down here
 * rather than in a comment on the day somebody notices.
 */
export type MenuOfRow = (row: PanelRow) => RowMenu | undefined;

/**
 * **A row's box, ticked or not** (R131): drawn in place of its mark, named by what pressing it
 * does, and ticked rows are drawn struck through. A panel's, as {@link MenuOfRow} is: the Todos
 * view knows a row is a todo, done or not, and what ticking it runs.
 */
export type CheckOfRow = (row: PanelRow) =>
  | {
      checked: boolean;
      /** What pressing the box does, which names it: "Mark done: …", "Reopen: …". */
      label: string;
      /** Why it cannot be pressed, when it cannot: drawn off, the sentence its tooltip. */
      off?: string;
      onChange: () => void;
    }
  | undefined;

export function PanelList({
  rows,
  empty,
  label,
  open,
  onOpen,
  detailOf,
  onRun,
  onAct,
  actsOff,
  menuOf,
  besideRow,
  checkOf,
  groupOf,
  page = PAGE,
  testid,
  offerFor,
  titleOnly = false,
  search = "beyond-a-page",
  dateAtEnd = false,
  markless = false,
}: {
  rows: readonly PanelRow[];
  /** What to say when there are none — the panel's own sentence, not a shared default. */
  empty: PanelEmpty;
  /** What this list is, for the search box and for a screen reader. */
  label: string;
  /** The key of the row whose card is open, held by whoever owns that state. */
  open: string | undefined;
  onOpen: (key: string | undefined) => void;
  /** What goes in an open row's card. The default is the row's own `detail`; a panel whose
   *  rows open something richer — the persona card — supplies it. */
  detailOf?: (row: PanelRow) => ReactNode;
  /** Run the catalogue row this row names, if the catalogue still offers it. `kept` is a
   *  double-click: what a single click previews, kept (SI-9b, `actions.toKeep`). */
  onRun?: (id: string, kept?: boolean) => void;
  /** The catalogue's row for an id, or none: what the empty state's `offer` is drawn from
   *  (#1156). A list without it, or without `onRun`, draws no way out. */
  offerFor?: (id: string) => Offer | undefined;
  /** Run one of the extension's own actions this row offers (charter-app#341). A list with no
   *  handler draws no action buttons: a button that does nothing is not drawn. */
  onAct?: (row: PanelRow, action: RowAction) => void;
  /**
   * Why the row actions are off, when they are: a read-only project's sentence (#826), since
   * the core refuses an extension's action there whatever it does. Each is still drawn, with
   * the sentence as its tooltip, `aria-disabled` and not `disabled`: the actions are items of
   * the list's roving group, so the keyboard walks onto one and reads why, and a press does
   * nothing.
   */
  actsOff?: string;
  /** A context menu on each row, where the panel can name what the row is about. */
  menuOf?: MenuOfRow;
  /** A sentence under a row, inside it, where the core refused something about it (#474). */
  besideRow?: (row: PanelRow) => string | undefined;
  /** A box at each row's start in place of its mark, where the panel can say what it ticks. */
  checkOf?: CheckOfRow;
  /**
   * The named group a row is drawn under, after the days the other rows are under — the Todos
   * view's Done (ADR 0106). `undefined` leaves the row under its day, as without this.
   */
  groupOf?: (row: PanelRow) => string | undefined;
  page?: number;
  testid?: string;
  /**
   * **A row on the right side is its title alone** (#1674, B-9): the note — a date, a count —
   * is said on hover, and a list whose every note is a date is drawn under the days it was
   * written on (`dayGroups.ts`). The right side's panels set it; a list in a view tab has the
   * pane's width and keeps its notes on the line.
   */
  titleOnly?: boolean;
  /**
   * When the search box is drawn: once there is more than a page (the operator's rule for a
   * list), or from the first row, as a sidebar view draws it in its top (#3340). In a view the
   * box is in the view's top either way ({@link InPanelTop}), held there while the rows scroll.
   */
  search?: "beyond-a-page" | "always";
  /** Each row's date, short and quiet at its end, as the prototype's Q1 A draws a memory's or a
   *  session's (`.rm`, #3340): the day for an earlier one, the time for today's. */
  dateAtEnd?: boolean;
  /** No mark before a row's words, where every row's would be the same one (#3340). */
  markless?: boolean;
}) {
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(page);
  /** In a view each day's name says how many rows it holds (the prototype's Q1 A, #3340). */
  const inView = useInPanelView();

  /**
   * **The search is over what the row says and what its body says** (ADR 0065 Q11): a memory
   * is found by the fact it holds, not only by the sentence it is filed under. A match the row
   * cannot show would read as a bug, so while a search is typed a row that matched in its body
   * shows the part that matched under it ({@link snippetOf}).
   */
  const wanted = query.trim().toLowerCase();
  const matching = useMemo(() => {
    if (wanted === "") return rows;
    return rows.filter((row) =>
      `${row.text} ${row.note ?? ""} ${row.detail?.text ?? ""}`.toLowerCase().includes(wanted),
    );
  }, [wanted, rows]);

  // **Grouped by day when the whole list is dated**, decided on every row and never on the
  // ones a search left, so typing never makes headings appear over a list that has none. The
  // days are put in order before the page is cut, so the first page is the newest day's.
  // One clock for the whole render, so its days cannot disagree with each other.
  const now = new Date();
  const dated = titleOnly && isDated(rows, noteOf);
  // A row a named group holds is drawn after every day, in that group (`groupOf`).
  const own = (row: PanelRow) => groupOf?.(row) === undefined;
  const named = groupOf === undefined ? [] : matching.filter((row) => !own(row));
  const ordered = dated
    ? [...byDay(matching.filter(own), noteOf, now).flatMap((day) => day.items), ...named]
    : [...matching.filter(own), ...named];
  const drawn = ordered.slice(0, limit);
  const days =
    dated || named.length > 0
      ? [
          ...(dated
            ? byDay(drawn.filter(own), noteOf, now)
            : [{ day: undefined, items: drawn.filter(own) }].filter((g) => g.items.length > 0)),
          ...groupsOf(
            drawn.filter((row) => !own(row)),
            (row) => groupOf?.(row) ?? "",
          ),
        ]
      : undefined;
  const more = matching.length - drawn.length;
  // **Once there is more than a page**, which is the operator's own rule. Against the whole
  // list and never the filtered one: a box that vanished when a search narrowed the list to
  // eight rows would take away the control being used.
  const searchable = search === "always" || rows.length > page;
  // The rows are ONE Tab stop, and Up and Down move along them (charter-app#189): the list
  // family's (`ui/list`, #2911), which comes back in on the row last left. An extension's row
  // actions are in their row, and ← and → reach them (charter-app#341).
  function drawRow(row: PanelRow) {
    return (
      <Row
        key={row.key}
        row={row}
        open={open === row.key}
        onOpen={(opening) => onOpen(opening ? row.key : undefined)}
        detail={detailOf?.(row) ?? defaultDetail(row)}
        snippet={snippetOf(row, wanted)}
        onRun={onRun}
        onAct={onAct}
        actsOff={actsOff}
        menu={menuOf?.(row)}
        refused={besideRow?.(row)}
        check={checkOf?.(row)}
        titleOnly={titleOnly}
        date={dateAtEnd && row.note !== null ? shortDate(row.note, now) : undefined}
        markless={markless}
      />
    );
  }

  if (rows.length === 0) {
    // **The empty state's way out is the catalogue's row** (#1156), drawn only while the
    // catalogue offers it, and run the way a row's is: an id the catalogue has stopped offering
    // draws nothing, which is `Doer`'s rule.
    const offer = empty.offer === null || onRun === undefined ? undefined : offerFor?.(empty.offer);
    return (
      <EmptyState
        variant="panel"
        headline={empty.headline}
        body={empty.body === null ? undefined : codeSpans(empty.body)}
        action={
          offer === undefined ? undefined : (
            <Button
              variant="primary"
              isDisabled={!offer.available}
              tooltip={offer.available ? offer.note : offer.reason}
              onPress={() => onRun?.(offer.id)}
            >
              {offer.title}
            </Button>
          )
        }
        testid={testid === undefined ? undefined : `${testid}-empty`}
      />
    );
  }

  return (
    <div className="panel-list" data-testid={testid}>
      {searchable && (
        <InPanelTop>
          <TextField
            className="panel-search"
            type="search"
            icon={<Search aria-hidden="true" />}
            keepsFocus={`panel-search:${label}`}
            value={query}
            aria-label={`Search ${label}`}
            // In a view the box says what it searches, as the prototype's does (#3340).
            placeholder={inView ? `Search ${label.toLowerCase()}` : `Search ${rows.length}`}
            onChange={(to) => {
              setQuery(to);
              // Back to one page: a search that kept a grown limit would show forty matches
              // for one word and then twelve for the next, which reads as rows going missing.
              setLimit(page);
            }}
          />
        </InPanelTop>
      )}

      {matching.length === 0 ? (
        // Not the empty state: the list is not empty, the search found nothing in it, and
        // those are different facts about a panel whose rows the reader can still get back.
        <p className="none ui-text-secondary">Nothing here matches “{query.trim()}”.</p>
      ) : (
        <List aria-label={label} className="panel-rows" size="sm">
          {days === undefined
            ? drawn.map(drawRow)
            : /* One scroll for the whole list and one Tab stop across its days: Down walks from
                 the last row of Today to the first of Yesterday. */
              days.map(({ day, items }) => (
                <ListRowSection
                  key={day ?? ""}
                  id={day ?? ""}
                  title={day}
                  named
                  count={inView ? items.length : undefined}
                >
                  {items.map(drawRow)}
                </ListRowSection>
              ))}
        </List>
      )}

      {more > 0 && (
        // Load more, at the list's foot: the way to see the rest is where the list ends.
        <Button
          variant="ghost"
          size="sm"
          className="panel-more"
          onPress={() => setLimit((was) => was + page)}
        >
          {`Show ${Math.min(more, page)} more`}
          <span className="note ui-text-meta">{` · ${more} left`}</span>
        </Button>
      )}
    </div>
  );
}

const noteOf = (row: PanelRow) => row.note;

/** `rows` under the group `groupOf` names for each, the groups in the order they are first met. */
function groupsOf(
  rows: readonly PanelRow[],
  groupOf: (row: PanelRow) => string,
): { day: string | undefined; items: PanelRow[] }[] {
  const groups: { day: string; items: PanelRow[] }[] = [];
  for (const row of rows) {
    const name = groupOf(row);
    const group = groups.find((one) => one.day === name);
    if (group) group.items.push(row);
    else groups.push({ day: name, items: [row] });
  }
  return groups;
}

/**
 * **A row's date, short** (#3340, the prototype's `.rm`): `HH:MM` for a stamp of today, `MM-DD`
 * for an earlier day, and the note as it is when it is no date.
 */
export function shortDate(note: string, now: Date): string {
  const stamp = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2}))?/.exec(note);
  if (stamp === null) return note;
  const [, year, month, day, hour, minute] = stamp;
  const today =
    Number(year) === now.getFullYear() &&
    Number(month) === now.getMonth() + 1 &&
    Number(day) === now.getDate();
  return today && hour !== undefined ? `${hour}:${minute}` : `${month}-${day}`;
}

/**
 * A panel's sentence with each `` `…` `` span drawn in code font (#1156): the core writes a
 * command the way the docs do, and the window draws it the way a Notice draws one, as `<code>`.
 * A backtick with no partner is left as it is.
 */
export function codeSpans(text: string): ReactNode {
  const parts = text.split(/`([^`]+)`/);
  if (parts.length === 1) return text;
  return parts.map((part, at) => (at % 2 === 1 ? <code key={at}>{part}</code> : part));
}

/** The card a row opens when the panel does not supply a richer one: the row's own words. */
function defaultDetail(row: PanelRow): ReactNode {
  if (row.detail === null) return null;
  return <p className="row-detail">{row.detail.text}</p>;
}

/**
 * One row: a mark, one line of words, a quiet note — and a card, when there is one to open.
 *
 * **A row of the list family** (`ui/list`, #2911): it runs its catalogue verb, or opens its
 * card, by a press or Enter; a row that does neither runs nothing. Its own actions stand at its
 * end, and what a search matched in its body, or what the core refused about it, is a line under
 * its words, inside it.
 */
function Row({
  row,
  open,
  onOpen,
  detail,
  snippet,
  onRun,
  onAct,
  actsOff,
  menu,
  refused,
  check,
  titleOnly,
  date,
  markless = false,
}: {
  row: PanelRow;
  titleOnly: boolean;
  /** Its date, short, at its end: {@link PanelList}'s `dateAtEnd`. */
  date?: string;
  /** No mark before its words: {@link PanelList}'s `markless`. */
  markless?: boolean;
  open: boolean;
  onOpen: (opening: boolean) => void;
  detail: ReactNode;
  /** The part of its body a search matched, while one is typed and the row's words do not
   *  show the match themselves. */
  snippet?: Snippet;
  onRun?: (id: string, kept?: boolean) => void;
  onAct?: (row: PanelRow, action: RowAction) => void;
  /** Why the row's actions are off: {@link PanelList}'s `actsOff`. */
  actsOff?: string;
  /** Its context menu: {@link PanelList}'s `menuOf`. */
  menu?: RowMenu;
  /** What the core refused about it: {@link PanelList}'s `besideRow`. */
  refused?: string;
  /** Its box: {@link PanelList}'s `checkOf`. */
  check?: ReturnType<CheckOfRow>;
}) {
  const Mark = MARKS[row.mark] ?? Circle;
  // **A persona's row wears that persona's mark** (#1449). Told by the row it runs, which only
  // purlis's own rows carry (`PanelRow.runs` is never set from a manifest): an extension's row
  // marked `persona` names nobody this window could look up, and keeps the plain glyph.
  const persona = row.runs?.startsWith(SHOWS_PERSONA) ? row.runs.slice(SHOWS_PERSONA.length) : null;
  const shortened = shorten(row.text);
  // **A row that runs opens something that holds the whole of it** — a persona's tab, a
  // memory's (SI-9b, ADR 0065 Q4) — so it carries no tooltip repeating its words over the row
  // below. A row whose card is a popover keeps one, for the reader who does not click.
  const whole = row.runs === null && row.text !== shortened ? row.text : undefined;
  // **On the right side the note is the hover** (#1674): the date a memory was written, how
  // much a persona remembers. On the whole row.
  const hover =
    titleOnly && row.note !== null
      ? row.text !== shortened
        ? `${row.text}\n${row.note}`
        : row.note
      : undefined;

  // **The catalogue row runs on the way open and never on the way shut.** A persona row's
  // `persona.show:<name>` is the same verb the palette and a context menu run (charter-app#174),
  // so the three are one state rather than three that look alike — and running it again on
  // dismissal would re-open what was just closed.
  /** The row, which its card stands beside. */
  const own = useRef<HTMLDivElement>(null);
  const opened = (opening: boolean) => {
    onOpen(opening);
    if (opening && row.runs !== null) onRun?.(row.runs);
  };
  const hasCard = detail !== null && row.runs === null;

  // **A row that does something does it, and opens no card** when it runs a verb. A persona's row
  // opens that persona's tab (`persona.show:<name>`, the operator's ruling of 2026-09-23), and a
  // memory's opens the memory's (SI-9b, ADR 0065 Q4): a card beside the row as well would be two
  // surfaces for one thing. A double-click keeps what the first click previewed.
  const action =
    row.runs !== null
      ? () => {
          if (row.runs !== null) onRun?.(row.runs);
        }
      : hasCard
        ? () => opened(!open)
        : undefined;

  return (
    <ListRow
      ref={own}
      id={row.key}
      textValue={row.text}
      className={clsx(
        "panel-row",
        row.tone !== "plain" && `is-${row.tone}`,
        check?.checked && "is-checked",
      )}
      data-testid={`panel-row-${row.key}`}
      data-open={hasCard && open ? "" : undefined}
      onAction={action}
      onDoubleClick={
        row.runs === null
          ? undefined
          : () => {
              if (row.runs !== null) onRun?.(row.runs, true);
            }
      }
      menu={menu}
      tip={hover ?? whole}
    >
      {(check !== undefined || !markless) && (
        <RowLead>
          {check !== undefined ? (
            <Checkbox
              /* Out of the row's own selection slot: this box ticks the todo, and selects
                 nothing (React Aria gives a list item's checkboxes the `selection` slot). */
              slot={null}
              className="row-check"
              isSelected={check.checked}
              isDisabled={check.off !== undefined}
              tooltip={check.off}
              aria-label={check.label}
              onChange={() => check.onChange()}
              data-testid={`row-check-${row.key}`}
            />
          ) : persona ? (
            <PersonaMark persona={persona} className="node-icon" />
          ) : (
            <Mark className="node-icon" />
          )}
        </RowLead>
      )}
      <RowLabel
        className={clsx("row-text ui-text-body", row.tone === "default" && "ui-text-strong")}
      >
        {shortened}
      </RowLabel>
      {row.note !== null && !titleOnly && (
        <RowDescription className="row-note ui-text-meta">{` · ${row.note}`}</RowDescription>
      )}
      {(date !== undefined ||
        row.tone === "default" ||
        (onAct !== undefined && row.actions.length > 0)) && (
        <RowTrail className="row-actions">
          {date !== undefined && <span className="row-date ui-text-meta">{date}</span>}
          {row.tone === "default" && <Star className="node-icon" />}
          {/* **The extension's own actions, at the row's end** (charter-app#341): pressing the
              row opens its card, and pressing an action asks the extension's program — two
              different things a click could mean, so they are two different controls, which ←
              and → reach. The titles and the asking are the manifest's. */}
          {onAct !== undefined &&
            row.actions.map((action) => (
              <Button
                key={action.id}
                size="sm"
                aria-disabled={actsOff !== undefined || undefined}
                tooltip={actsOff}
                onPress={actsOff === undefined ? () => onAct(row, action) : undefined}
                className="row-action"
              >
                {action.title}
              </Button>
            ))}
        </RowTrail>
      )}
      {snippet !== undefined && (
        <RowBelow className="row-snippet ui-text-meta" data-testid={`row-snippet-${row.key}`}>
          {snippet.before}
          <mark>{snippet.match}</mark>
          {snippet.after}
        </RowBelow>
      )}
      {refused !== undefined && (
        <RowBelow className="row-refusal trouble">
          <span role="alert">{refused}</span>
        </RowBelow>
      )}
      {hasCard && (
        /* `side="left"` is where it opens from in the default arrangement and no more than
           that: a region MOVES (ADR 0038), and the popover flips to the other side when
           there is no room, which is what makes naming a side safe. */
        <HeldPopover
          triggerRef={own}
          isOpen={open}
          onOpenChange={opened}
          className="row-card"
          data-testid={`row-detail-${row.key}`}
          side="left"
          align="start"
          offset={6}
          arrow
          aria-label={row.text}
        >
          {detail}
        </HeldPopover>
      )}
    </ListRow>
  );
}

/** The part of a row's body a search matched: the match, and a little either side of it. */
export type Snippet = { before: string; match: string; after: string };

/** How many characters of body a snippet shows either side of the match. */
export const SNIPPET_AROUND = 40;

/**
 * The part of `row`'s body that `wanted` (lower-cased) matched, for the line under the row —
 * or nothing, when there is no search, when the body does not hold it, or when the row's own
 * words already show it (a snippet then says what is already on screen).
 *
 * Cut to {@link SNIPPET_AROUND} characters either side, on one line, with an ellipsis where
 * the body goes on: a list is still a list while it is searched.
 */
export function snippetOf(row: PanelRow, wanted: string): Snippet | undefined {
  const body = row.detail?.text;
  if (wanted === "" || body === undefined || body === row.text) return undefined;
  if (`${row.text} ${row.note ?? ""}`.toLowerCase().includes(wanted)) return undefined;
  const flat = body.replace(/\s+/g, " ");
  const at = flat.toLowerCase().indexOf(wanted);
  if (at < 0) return undefined;
  const from = Math.max(0, at - SNIPPET_AROUND);
  const to = Math.min(flat.length, at + wanted.length + SNIPPET_AROUND);
  return {
    before: `${from > 0 ? "…" : ""}${flat.slice(from, at)}`,
    match: flat.slice(at, at + wanted.length),
    after: `${flat.slice(at + wanted.length, to)}${to < flat.length ? "…" : ""}`,
  };
}

/**
 * One line's worth of a row's words, with the rest left to the card.
 *
 * Cut on a word boundary where there is one near the end, because a cut mid-word reads as a
 * truncated *word* rather than a truncated line — and the ellipsis is then doing two jobs.
 */
export function shorten(text: string, most = SHORTEST): string {
  if (text.length <= most) return text;
  const cut = text.slice(0, most);
  const space = cut.lastIndexOf(" ");
  return `${(space > most * 0.6 ? cut.slice(0, space) : cut).trimEnd()}…`;
}
