// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  useContext,
  useId,
  useLayoutEffect,
  useRef,
  type ReactNode,
  type Ref,
} from "react";
import { Ellipsis, Search } from "lucide-react";
import { Select } from "../ui/select/Select";
import { ViewNav, type ViewPage } from "../ui/panel/ViewNav";
import { Button, IconButton, ToggleButton } from "../ui/button/Button";
import { Menu, MenuItem, MenuSeparator, MenuTrigger } from "../ui/menu/Menu";
import { Checkbox, CheckboxGroup, RovingCheckbox } from "../ui/choice/Checkbox";
import {
  OptionCard,
  OptionCards,
  Radio,
  RadioGroup,
  Segment,
  SegmentedControl,
} from "../ui/choice/Radio";
import { Slider } from "../ui/choice/Slider";
import { Switch } from "../ui/choice/Switch";
import { TextArea, TextField, useFormSummary } from "../ui/field/Field";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { useReadOnlyReasonId } from "../readOnlyReason";
import { Badge } from "../ui/badge/Badge";
import { Disclosure } from "../ui/disclosure/Disclosure";

/**
 * **The house settings set** (SE-16, #1166; DS-3 #626; V89f, V89j): the six pieces every
 * settings screen and every form is drawn from — {@link SettingsLayout}, {@link SettingGroup},
 * {@link SettingRow}, {@link Field}, {@link Choice} and {@link SettingActions}.
 *
 * `docs/ui-primitives.md` forbids a house component library and allows exactly this set, for the
 * reasons written there: charter had two hand-built form styles that had already drifted
 * (`settings-*` and the dialogs' `choices`), and one small set is how a new setting stays a few
 * lines. Each piece is a thin layer over a native element or a family already in the window — the
 * choice families (`ui/choice`, #2907), the field family, the select family — and **says which at
 * its call site**: the props are the primitive's words (`value`, `onValueChange`, `checked`), not
 * new ones.
 *
 * Drawn in `App.css` under `ui-*` classes, every colour a design-system token. The old
 * hand-built `settings-*`, `asks`, `choices`, `choice`, `who` and `picking` classes were deleted
 * once the last screen had moved (DS-3e, #1177), and `oldFormClasses.test.ts` keeps them gone.
 */

/**
 * **Whether a row is drawn on a settings page** (#2919, prototype A, R50–R52): drawn by
 * {@link SettingsLayout}, a row is the aligned form's, its label and the help line under it in
 * one column, its control in the next and its actions in the last, so every control starts at
 * the same x. A form elsewhere — a dialog's rows — keeps its label and control on one line and
 * its help under them, where a person filling it in reads it as they go.
 */
const OnPage = createContext(false);

/** The elements a `<label for>` focuses on its own. */
const LABELABLE = new Set(["INPUT", "SELECT", "TEXTAREA", "BUTTON", "METER", "OUTPUT", "PROGRESS"]);

/** One level of the switcher: You, Project, Workspace or Persona (`CONTEXT.md`, **Level**). */
export type LevelOffer = { id: string; label: string };

/** A group the nav offers. `sub` draws it as a page of the group before it (ST-4); `count` is
 *  how many of its things wait for the person (Your hosts' waiting hosts, R46). */
export type GroupOffer = { id: string; label: string; sub?: boolean; count?: number };

/**
 * **The nav's pages, folded by what is kept** (#2919, R44, R46, R47): a page followed by pages
 * of its own heads a group, open or folded; a folded group's pages are not drawn, and the page
 * that heads it carries the sum of their counts, so nothing waiting is hidden. While the filter
 * holds words every group is drawn open, without keeping it.
 */
function navPages(
  groups: readonly GroupOffer[],
  open: ((id: string) => boolean) | undefined,
  filtering: boolean,
): ViewPage[] {
  const pages: ViewPage[] = [];
  groups.forEach((one, at) => {
    if (one.sub) return;
    const kids: GroupOffer[] = [];
    for (const next of groups.slice(at + 1)) {
      if (!next.sub) break;
      kids.push(next);
    }
    // With no kept folds every group is open, and draws no toggle.
    const shown = open === undefined || filtering || open(one.id);
    const waiting = kids.reduce((all, kid) => all + (kid.count ?? 0), 0);
    pages.push({
      id: one.id,
      label: one.label,
      ...(kids.length > 0 && open !== undefined ? { expanded: shown } : {}),
      count: (one.count ?? 0) + (kids.length > 0 && !shown ? waiting : 0) || undefined,
      waiting: true,
    });
    if (shown)
      for (const kid of kids)
        pages.push({ id: kid.id, label: kid.label, sub: true, count: kid.count, waiting: true });
  });
  return pages;
}

/** **The nav's page of every match at every level** (#2920): no group's address can be it. */
export const FOUND_PAGE = "\u0000found";

/** **What a search that finds nothing says** (#2920), by what it searched for: its words, the
 *  values set at this level, or both. */
export function noneFound(words: string, setHere: boolean): string {
  const said = words.trim();
  if (said === "") return "No setting is set at this level.";
  return setHere
    ? `No setting set at this level matches “${said}”.`
    : `No setting matches “${said}”.`;
}

/** The page that heads the group `id` is a page of, if it is one. */
function headOf(groups: readonly GroupOffer[], id: string): string | undefined {
  const at = groups.findIndex((one) => one.id === id);
  if (at < 0 || !groups[at]?.sub) return undefined;
  for (let back = at - 1; back >= 0; back--) if (!groups[back]?.sub) return groups[back]?.id;
  return undefined;
}

/**
 * **The two columns** (V89b): the level switcher and what that level is above a nav of its
 * groups, and the chosen group beside them.
 *
 * The switcher is the segmented control (`ui/choice`, #2907), because a level is one choice of a
 * few; it lists only the levels it is handed, so a level with nothing to set is not offered. **An
 * arrow goes to the next level** (D-626-4), as every {@link Choice} radio picks and the WAI-ARIA
 * radio pattern asks: a level is a view and writes nothing, so nothing holds the pick for a
 * button. The caller may draw the new level with a layout of its own (`SettingsTab` does), so
 * the switcher the keyboard left is gone; the one drawn next takes the focus back on the level
 * picked, and only then. The nav is the panel family's page navigation (`ViewNav`, #2930): one
 * Tab stop, moved through with the arrow keys, Home and End, chosen with Enter, Space or a press,
 * and the chosen group says so with `aria-current="page"` and the family's mark.
 *
 * **The search** (SE-21, V89c; one search across every level, #2920, N27) sits above the nav: a
 * search box, labelled, and the "Set here" toggle under it, whose words and toggle the caller
 * searches by — the layout draws what it is handed. While either holds (`searching`), the nav's
 * first page is every match at every level ({@link FOUND_PAGE}, "All results"), apart from the
 * caller's narrowed groups under it, and a polite live region under the box says how many
 * settings match (`found`), or that none does. Escape in the box clears its words.
 *
 * **The foot of the nav** (`foot`, SE-19, V89d) is the caller's: the Project level's "Edit as
 * TOML" link per file. It is under the groups and not one of them, so the filter leaves it be.
 */
export function SettingsLayout({
  levels,
  level,
  onLevelChange,
  about,
  groups,
  group,
  onGroupChange,
  filter,
  onFilterChange,
  setHere = false,
  onSetHereChange,
  found,
  foot,
  children,
  holder,
  folds,
}: {
  /** **Which of the nav's groups are open** (R42, R47), and the way to open or fold one: the
   *  window's kept choice (`navGroups.ts`). Without it every group is open, with no toggle. */
  folds?: { isOpen: (group: string) => boolean; setOpen: (group: string, open: boolean) => void };
  levels: readonly LevelOffer[];
  level: string;
  onLevelChange: (level: string) => void;
  /** One sentence on whose settings these are and where they are kept. */
  about?: ReactNode;
  groups: readonly GroupOffer[];
  group: string;
  onGroupChange: (group: string) => void;
  /** The words in the search box, as typed. */
  filter: string;
  onFilterChange: (to: string) => void;
  /** Whether "Set here" narrows the search to the values set at this level (#2920). */
  setHere?: boolean;
  onSetHereChange?: (to: boolean) => void;
  /** How many settings the search finds, while it searches; unset while the level is still
   *  being read, so nothing is said yet. */
  found?: number;
  /** What sits under the nav, whatever the filter. */
  foot?: ReactNode;
  /** The chosen group: a {@link SettingGroup}. */
  children: ReactNode;
  /** The tab's own element, for what is looked for in this tab and not the whole window. */
  holder?: Ref<HTMLDivElement>;
}) {
  const switcher = useRef<HTMLDivElement>(null);
  // A group is open while one of its pages is current (R44): drawn so, not kept, so a link that
  // lands in a folded group leaves the person's fold as they made it.
  const head = headOf(groups, group);
  const setOpen = folds?.setOpen;
  const words = filter.trim();
  const searching = words !== "" || setHere;
  const narrowedPages = navPages(
    groups,
    folds === undefined ? undefined : (id) => folds.isOpen(id) || id === head,
    searching,
  );
  // While it searches, every match at every level is the nav's first page, apart from this
  // level's pages under it.
  const pages: ViewPage[] = searching
    ? [
        { id: FOUND_PAGE, label: "All results", count: found },
        ...narrowedPages.map((one, at) => (at === 0 ? { ...one, startsGroup: true } : one)),
      ]
    : narrowedPages;
  useLayoutEffect(() => {
    if (arrowedTo !== level) return;
    arrowedTo = undefined;
    const shown = switcher.current?.querySelector<HTMLElement>('input[type="radio"]:checked');
    if (shown && document.activeElement !== shown) shown.focus();
  }, [level]);
  const said =
    !searching || found === undefined
      ? ""
      : found === 0
        ? noneFound(words, setHere)
        : `${found} ${found === 1 ? "setting matches" : "settings match"}`;
  return (
    <div className="ui-settings" ref={holder}>
      {/* **The side column** (R37, R38): the level switcher, whose settings these are, the
          filter and the pages, on the window's own ground beside the page. No title of its own:
          the tab names the view (#3147, the operator's ruling of 2026-10-24, after the
          prototype drew "Settings" here). */}
      <div className="ui-settings-side">
        <SegmentedControl
          ref={switcher}
          // Small, so its three levels fit the 236 px column in every theme (Classic's words are
          // wider than Purlis's).
          size="sm"
          aria-label="Level"
          value={level}
          onChange={(to) => {
            // Picked from the keyboard (the focus is on the switcher), the level drawn next takes
            // the focus back. Forgotten once this pick is done, so a level the caller did not
            // show never takes the focus later.
            if (switcher.current?.contains(document.activeElement)) {
              arrowedTo = to;
              setTimeout(() => {
                if (arrowedTo === to) arrowedTo = undefined;
              });
            }
            onLevelChange(to);
          }}
        >
          {levels.map((one) => (
            <Segment key={one.id} value={one.id}>
              {one.label}
            </Segment>
          ))}
        </SegmentedControl>
        {about !== undefined && <p className="ui-settings-about">{about}</p>}
        <div className="ui-settings-search">
          <TextField
            className="ui-settings-filter"
            type="search"
            icon={<Search aria-hidden="true" />}
            keepsFocus="settings-filter"
            value={filter}
            aria-label="Search settings"
            placeholder="Search all levels"
            onChange={onFilterChange}
            onKeyDown={(event) => {
              if (event.key !== "Escape" || filter === "") return;
              // Taken here: an Escape that cleared the box has done its job, and nothing
              // behind the tab should act on it too.
              event.preventDefault();
              event.stopPropagation();
              onFilterChange("");
            }}
          />
          <ToggleButton
            size="sm"
            variant="ghost"
            isSelected={setHere}
            onChange={(to) => onSetHereChange?.(to)}
          >
            Set here
          </ToggleButton>
        </div>
        <p className="sr-only" role="status" aria-live="polite" aria-label="Settings found">
          {said}
        </p>
        <div className="ui-settings-nav">
          <ViewNav
            label="Groups"
            pages={pages}
            page={group}
            onPage={(to) => {
              // Choosing the page that heads a group opens it too (R45).
              if (pages.some((one) => one.id === to && one.expanded === false)) setOpen?.(to, true);
              onGroupChange(to);
            }}
            onExpand={setOpen}
            rowWhenNarrow
            edge={false}
          />
        </div>
        {foot}
      </div>
      {/* **The page** (R37): it scrolls on its own beside the side column, at most 860 px wide. */}
      <div className="ui-settings-body">
        <div className="ui-settings-page">
          <OnPage.Provider value={true}>{children}</OnPage.Provider>
        </div>
      </div>
    </div>
  );
}

/** **One group**, the right column: its heading, what it is for, and its rows. A region named
 *  by its heading, so a screen reader can say where it is.
 *
 *  **Where its values are kept is said here, once** (`kept`, DS-8 #1840): the file and who sees
 *  it, never under each row. A row whose value is kept anywhere else says so with its badge
 *  ({@link SettingRow}).
 *
 *  **On a settings page it is the page's header** (#2919, R48, R49): the title, the blurb (its
 *  `help`), and the line that says where the page is kept, the file named as code (`file`), then
 *  Edit as TOML (`onEdit`), which opens that file's text. In a dialog its help and where it is
 *  kept are one line under its heading. */
export function SettingGroup({
  label,
  help,
  kept,
  file,
  onEdit,
  editAs = "TOML",
  rarely,
  open = false,
  level = 2,
  children,
}: {
  label: string;
  /** Its heading's level: 2 in a view, the first heading under the tab that names it (#3147);
   *  3 in a dialog, under the dialog's own title. */
  level?: 2 | 3;
  help?: string;
  /** Where the group's values are kept, and who sees them: "Kept in purlis.toml, which your
   *  team sees." */
  kept?: string;
  /** The file `kept` names, which the page's header sets as code (R48). */
  file?: string;
  /** Opens the file's whole text (SE-19): the header's Edit as TOML, after where it is kept.
   *  None for a file that has no text to edit here. */
  onEdit?: () => void;
  /** What the file's text is written in: a project's files are TOML, a workspace's JSON. */
  editAs?: "TOML" | "JSON";
  /** **A rarely used group of a page** (R61), as many settings as it holds: folded until it is
   *  opened, its title then the count ("Branches · 1"), its chevron after both. */
  rarely?: number;
  /** A rarely used group starts open: while a filter holds words, so what it found is shown. */
  open?: boolean;
  children: ReactNode;
}) {
  const heading = useId();
  const onPage = useContext(OnPage);
  if (onPage && level === 2)
    return (
      <section className="ui-setting-group ui-setting-page" aria-labelledby={heading}>
        <div className="ui-setting-head">
          <h2 id={heading}>{label}</h2>
          {help && <p className="ui-setting-blurb">{help}</p>}
          {kept && (
            <p className="ui-setting-where">
              <span>{withFile(kept, file)}</span>
              {onEdit && (
                <>
                  <span aria-hidden="true">·</span>
                  <Button variant="ghost" size="sm" onPress={onEdit}>
                    {`Edit as ${editAs}`}
                  </Button>
                </>
              )}
            </p>
          )}
        </div>
        {/* The page's rows scroll under its header, which keeps the pane's actions' room at its
            end (#3147), so nothing that scrolls passes under them. */}
        <div className="ui-setting-rows">{children}</div>
      </section>
    );
  const Heading = level === 2 ? "h2" : "h3";
  const said = [help, kept].filter(Boolean).join(" ");
  // **A page's own groups** (R60, R61): a heading of 13/600, its one line, its rows; folded, a
  // disclosure whose trigger is the heading.
  if (onPage && level === 3 && rarely !== undefined)
    return (
      <Disclosure
        className="ui-setting-section ui-setting-fold"
        level={3}
        twistAt="end"
        label={`${label} · ${rarely}`}
        defaultExpanded={open}
        panelRole="region"
      >
        {said && <p className="ui-setting-help">{said}</p>}
        {children}
      </Disclosure>
    );
  return (
    <section
      className={onPage ? "ui-setting-group ui-setting-section" : "ui-setting-group"}
      aria-labelledby={heading}
    >
      <Heading id={heading}>{label}</Heading>
      {said && <p className="ui-setting-help">{said}</p>}
      {children}
    </section>
  );
}

/** `kept`, with the first place it names `file` set as code — by its path, else by its name
 *  alone; as it is when it names neither. */
function withFile(kept: string, file: string | undefined): ReactNode {
  if (file === undefined || file === "") return kept;
  const shown = [file, file.split("/").pop() ?? file].find((one) => kept.includes(one));
  if (shown === undefined) return kept;
  const at = kept.indexOf(shown);
  return (
    <>
      {kept.slice(0, at)}
      <code>{shown}</code>
      {kept.slice(at + shown.length)}
    </>
  );
}

/**
 * The ids a row hands its control, so the control is named and described by the row.
 *
 * **A control with no row** (#626) is handed these by its caller: a box in a table's cell, a
 * pick inside a sentence, the chips over a list. No row's `<label>` names it and no help line
 * sits under it, so it carries its own name (`label`, its `aria-label`) and, where it has one,
 * what the help line would have said (`title`). It is still the set's control, with the set's
 * keyboard and look.
 */
export type RowIds = {
  /** The control's own id; the row's `<label for>` points at it, unless the row is `grouped`. */
  id: string;
  /** The row's name's id, for a control a `<label for>` cannot name — a radio group. A control
   *  with no row has none, and is named by `label`. */
  labelledBy?: string;
  /** The help line's id, when the row has help. */
  describedBy?: string;
  /** A control with no row: its own name, said as its `aria-label`. */
  label?: string;
  /** A control with no row: what its help line would have said, as its tooltip. */
  title?: string;
  /** The row's name, in words, for a control a `<label for>` cannot name: a slider, whose
   *  native range is its thumb's. */
  name?: string;
};

/** A row's way back to the value beneath it. */
export type Reset = {
  label: string;
  disabled: boolean;
  onReset: () => void;
  /** Its name where a page has many alike: it starts with `label`, which stays what is heard. */
  name?: string;
};

/** **Where a row's value comes from**, when that is not where its group keeps its values: the
 *  level or the file in a word or two (`label`: Team, This machine, Workspace), and the whole
 *  sentence (`says`), which the badge's tooltip holds. */
export type Badge = { label: string; says: string };

/**
 * **One setting**: its label, its control and its reset, at one height — the control height
 * every button, field and select is (ADR 0094). On a settings page it is a row of the aligned
 * form (#2919, R50–R52): the label with its help line under it in the first column, the control
 * in the second, the row's actions in the third. In a dialog's form the label and the control
 * share a line and the help is a line under the field. Either way the control is described by
 * the help. The control is drawn by `control`, handed the ids that tie it to the label and the
 * help — a {@link Field} or a {@link Choice}.
 *
 * A `grouped` row's control is a group of controls (a radio group), which a `<label for>` cannot
 * name: its name is drawn as plain text, and the group is named by `aria-labelledby` instead.
 *
 * **A write the core refused is said here** (`error`, SE-17, V89e): beside the setting, as an
 * alert, and in the control's description, while the control shows what is on disk. In a form
 * (a collection's New… or Rename), the send is what was refused, and the form's one summary is
 * its alert (#3291): the row's line is a polite live region, quiet while the summary says it.
 * And the last change's **Undo** sits in the row it was made in.
 *
 * **Where the value comes from** (SE-18, V89d; drawn quietly since DS-8, #1840): the group says
 * once where its values are kept ({@link SettingGroup}'s `kept`), so a row says nothing more
 * unless its value is somewhere else. Then `badge` names the level beside the label — Team, This
 * machine, Workspace — and its tooltip holds the whole sentence. `origin`, that same sentence for
 * every row, is never drawn: it is the control's description, so a screen reader still hears
 * where each value comes from. `modified` marks a value set at this level, the one Reset takes
 * out. On a settings page the row's Reset, its "Move to…" (`moves`) and Copy key are one ⋯ menu
 * at the row's end, in the same place on every row (R56–R59).
 */
export function SettingRow({
  label,
  help,
  reset,
  grouped = false,
  error,
  undo,
  origin,
  badge,
  modified = false,
  deprecated = false,
  moves,
  copyKey,
  control,
  setting,
}: {
  label: string;
  /** One line on what the value does. A node and not only a string, because a dialog's help
   *  names files and commands, and those are `<code>` (DS-3c). */
  help?: ReactNode;
  reset?: Reset;
  grouped?: boolean;
  /** Why the last write of this setting was refused, in the core's words. */
  error?: readonly string[];
  /** Puts back what this setting was before the last change, when that change was here. */
  undo?: () => void;
  /** Which level and which file the value shown comes from, or that none here holds it: the
   *  control's description, never a line of its own. */
  origin?: string;
  /** The value is kept elsewhere than its group says: where, as a badge with a tooltip. */
  badge?: Badge;
  /** The value is set at this level: what Reset would take out. */
  modified?: boolean;
  /** The key is kept only until every machine has moved off it: an amber badge after the label
   *  says so (R62). */
  deprecated?: boolean;
  /** The row's "Move to…" between the files a value may be kept in (SE-18): rows of the menu
   *  family, drawn in the row's ⋯ menu between its Reset and Copy key (R57). */
  moves?: ReactNode;
  /** The value's key in its file, which the ⋯ menu's Copy key copies (R57): `plane.mode`. */
  copyKey?: string;
  control: (ids: RowIds) => ReactNode;
  /** The setting's id, which a link into Settings names to focus its control (NO-7). */
  setting?: string;
}) {
  const id = useId();
  const labelledBy = useId();
  const described = useId();
  const from = useId();
  const refused = useId();
  const failed = error !== undefined && error.length > 0;
  const onPage = useContext(OnPage);
  const summary = useFormSummary();
  // Off on a read-only project: why, with the rest of what describes it (#2908).
  const readOnly = useReadOnlyReasonId();
  const describedBy =
    [
      help ? described : undefined,
      origin ? from : undefined,
      failed ? refused : undefined,
      readOnly,
    ]
      .filter(Boolean)
      .join(" ") || undefined;
  const name = grouped ? (
    <span className="ui-setting-label" id={labelledBy}>
      {label}
    </span>
  ) : (
    <label
      className="ui-setting-label"
      id={labelledBy}
      htmlFor={id}
      onClick={(event) => {
        // A control a `<label for>` cannot reach (a slider's root, which hands the
        // focus to its thumb) is given the focus the press would have given it.
        const control = document.getElementById(id);
        if (control !== null && !LABELABLE.has(control.tagName)) {
          event.preventDefault();
          control.focus();
        }
      }}
    >
      {label}
    </label>
  );
  // **Set here** (R53): a 6 px dot in the accent before the label, which says so under the
  // pointer. Its hint is its own name, so the keyboard passes it by: the control is described as
  // set here by its origin.
  const mark = modified && (
    <Tooltip tip="Set at this level">
      <span className="ui-setting-modified" role="img" aria-label="Set at this level" />
    </Tooltip>
  );
  const controls = (
    <div className="ui-setting-control">
      {control({ id, labelledBy, describedBy, name: label })}
      {reset && !onPage && (
        <Button
          isDisabled={reset.disabled}
          aria-label={reset.name}
          onPress={() => {
            reset.onReset();
            // The button goes once the value is gone; the focus goes back to the control.
            document.getElementById(id)?.focus();
          }}
          className="ui-setting-reset"
        >
          {reset.label}
        </Button>
      )}
      {undo && (
        <Button
          onPress={() => {
            undo();
            // The button goes once it is pressed; the focus goes back to what it undid.
            document.getElementById(id)?.focus();
          }}
          className="ui-setting-reset"
        >
          Undo
        </Button>
      )}
    </div>
  );
  const helpLine = help && (
    <p className="ui-setting-help" id={described}>
      {help}
    </p>
  );
  return (
    <div
      className={grouped ? "ui-setting-row ui-setting-row-grouped" : "ui-setting-row"}
      data-setting={setting}
    >
      {onPage ? (
        // **The aligned form** (R50, R52): the label with its help under it, then the control,
        // then the row's actions, each in its column.
        <div className="ui-setting-line">
          <div className="ui-setting-name">
            <span className="ui-setting-title">
              {mark}
              {name}
              {deprecated && <Badge variant="waiting">deprecated</Badge>}
              {badge && <OriginBadge badge={badge} />}
            </span>
            {helpLine}
          </div>
          {controls}
          <div className="ui-setting-act">
            <RowMenu
              label={label}
              reset={reset}
              moves={moves}
              copyKey={copyKey}
              onReset={() => document.getElementById(id)?.focus()}
            />
          </div>
        </div>
      ) : (
        <>
          <div className="ui-setting-line">
            <div className="ui-setting-name">
              {mark}
              {name}
              {deprecated && <Badge variant="waiting">deprecated</Badge>}
              {badge && <OriginBadge badge={badge} />}
            </div>
            {controls}
          </div>
          {helpLine}
        </>
      )}
      {origin && (
        <p className="sr-only" id={from}>
          {origin}
        </p>
      )}
      {summary.inForm ? (
        // There from the start, so its words are heard as they land (#3033), and only once.
        <div
          className="ui-setting-error"
          id={refused}
          aria-live={summary.quiet ? "off" : "polite"}
          aria-atomic="true"
        >
          {failed && error.map((why, at) => <p key={at}>{why}</p>)}
        </div>
      ) : (
        failed && (
          <div className="ui-setting-error" id={refused} role="alert">
            {error.map((why, at) => (
              <p key={at}>{why}</p>
            ))}
          </div>
        )
      )}
    </div>
  );
}

/**
 * **A row's ⋯ menu** (#2919, R56–R59): one quiet icon button at the row's end, the same place on
 * every row, whose menu holds the row's Reset (held out of reach while the value is not set at
 * this level, R58), then its moves between files, then Copy key, with a rule between each part.
 * The menu opens under its button, lined up with its end, and closes on a row chosen, Escape or
 * a press outside it. A Reset puts the keyboard back on the control it reset; anything else
 * leaves it on the button. A row with none of the three draws no button, and its column stays.
 */
function RowMenu({
  label,
  reset,
  moves,
  copyKey,
  onReset,
}: {
  label: string;
  reset?: Reset;
  moves?: ReactNode;
  copyKey?: string;
  /** After a Reset, once the menu has closed. */
  onReset: () => void;
}) {
  const resetting = useRef(false);
  if (reset === undefined && !moves && copyKey === undefined) return null;
  const parts = [
    reset && (
      <MenuItem
        key="reset"
        id="reset"
        isDisabled={reset.disabled}
        onAction={() => {
          resetting.current = true;
          reset.onReset();
        }}
      >
        {reset.label}
      </MenuItem>
    ),
    moves,
    copyKey !== undefined && (
      <MenuItem
        key="copy"
        id="copy"
        textValue={`Copy key ${copyKey}`}
        onAction={() => {
          // Nothing to say when the clipboard refuses: the key is still on the menu's row.
          navigator.clipboard?.writeText(copyKey).catch(() => undefined);
        }}
      >
        Copy key <code className="ui-setting-key">{copyKey}</code>
      </MenuItem>
    ),
  ].filter(Boolean);
  return (
    <MenuTrigger
      onClosed={
        // Back on the control a Reset took the value of; the trigger keeps it otherwise.
        () => {
          if (!resetting.current) return;
          resetting.current = false;
          onReset();
        }
      }
    >
      <IconButton
        variant="quiet"
        aria-label={`More for ${label}`}
        tooltip="Reset, move to another file"
        icon={<Ellipsis aria-hidden="true" />}
      />
      <Menu aria-label={`More for ${label}`} align="end" className="ui-setting-menu">
        {parts.flatMap((part, at) =>
          at === 0 ? [part] : [<MenuSeparator key={`rule-${at}`} />, part],
        )}
      </Menu>
    </MenuTrigger>
  );
}

/**
 * **A row's badge** (DS-8, #1840): the level its value comes from, in a word or two, and the
 * whole sentence in a tooltip (the shared one, #2908). The badge is a word in the Tab order, so the
 * keyboard can reach it and read the tooltip; it presses nothing. Escape closes the tooltip.
 */
function OriginBadge({ badge }: { badge: Badge }) {
  return (
    // A word, not a control: it presses nothing, so it is no button. It takes the keyboard only
    // so the tooltip can be read without a pointer.
    <Tooltip tip={badge.says}>
      <Badge variant="accent">{badge.label}</Badge>
    </Tooltip>
  );
}

/** What every field and choice is handed: the row's ids. */
type Tied = { ids: RowIds };

/** With no row, a box or a select is never wider than the cell or the line it is drawn in, so a
 *  long value does not push a table past its page. */
const narrow = (ids: RowIds) => (ids.label === undefined ? [] : ["min-w-0", "max-w-full"]);

/** A box's place in its row (`ui-setting-box`): the field family draws it (`ui/field`, #2905). */
const boxPlace = (ids: RowIds) => ["ui-setting-box", ...narrow(ids)].join(" ");

/**
 * **A value typed or slid**: `text` is one line and `list` one entry per line, each the field
 * family's box (`TextField`, `TextArea`, #2905), and `range` a whole number between two bounds,
 * with what it is at said beside it, a native slider until the slider family moves.
 *
 * A typed value is **committed** when the field is left, and a `text` one on Enter too
 * (`onCommit`): a value written as it is changed (V89e) is written once it is typed, not at
 * every key — half a branch name is not a value to write, or to refuse. A typed value may be
 * **checked as it is left** (`validate`, N26): what is wrong is said under the box, and a form
 * of the family checks it again before it is sent.
 */
export type FieldProps = Tied &
  (
    | {
        kind: "text";
        value: string;
        onChange: (to: string) => void;
        /** What the empty box shows: an example of the answer, never what to do — that is the
         *  row's help. */
        placeholder?: string;
        /** The input's own cap on what can be typed. */
        maxLength?: number;
        /** The input's own `inputMode`: `numeric` for a count, so a touch keyboard offers
         *  digits. The value is still the text typed. */
        inputMode?: "numeric";
        /** The input's own `size`, in characters: a box in a table's cell is as wide as what it
         *  holds, not the row's width. */
        size?: number;
        /** The input's own `disabled`: out of reach while this answer is moot or its work is
         *  running. */
        disabled?: boolean;
        /** What is wrong with a value, said when the box is left (N26). */
        validate?: (value: string) => string | undefined;
        onCommit?: () => void;
      }
    /** One entry per line. The text is what is typed, kept whole — an empty line while the next
     *  entry is being typed is still there — and the caller reads the entries out of it. Drawn
     *  at its row's width, under its name, and as tall as its lines (DS-10, {@link grownTo}). */
    | {
        kind: "list";
        value: string;
        onChange: (to: string) => void;
        /** The fewest lines the box shows, before it grows with what is typed. Two unless said. */
        minRows?: number;
        /** What is wrong with the entries, said when the box is left (N26). */
        validate?: (value: string) => string | undefined;
        onCommit?: () => void;
      }
    | {
        kind: "range";
        value: number;
        min: number;
        max: number;
        step?: number;
        /** What the value reads as beside the slider: `18px`. */
        shown: (value: number) => string;
        /** What a screen reader says it is, when that is not `shown`: `18 pixels`. */
        spoken?: (value: number) => string;
        onChange: (to: number) => void;
      }
  );

export function Field(props: FieldProps) {
  const { ids } = props;
  if (props.kind === "range")
    return (
      <Slider
        id={ids.id}
        className="ui-setting-slider"
        minValue={props.min}
        maxValue={props.max}
        step={props.step}
        value={props.value}
        shown={props.shown}
        spoken={props.spoken}
        label={ids.label ?? ids.name ?? ""}
        aria-describedby={ids.describedBy}
        onChange={props.onChange}
      />
    );
  if (props.kind === "list") return <ListField {...props} />;
  // A value its check refuses is never written: the box says why, and keeps the words.
  const onCommit =
    props.onCommit &&
    (() => {
      if (props.validate?.(props.value) === undefined) props.onCommit?.();
    });
  return (
    <TextField
      id={ids.id}
      className={boxPlace(ids)}
      value={props.value}
      placeholder={props.placeholder}
      maxLength={props.maxLength}
      inputMode={props.inputMode}
      columns={props.size}
      isDisabled={props.disabled}
      validate={props.validate}
      aria-label={ids.label}
      aria-labelledby={ids.label === undefined ? ids.labelledBy : undefined}
      tooltip={ids.title}
      aria-describedby={ids.describedBy}
      onChange={props.onChange}
      onBlur={onCommit}
      onKeyDown={(event) => {
        if (event.key === "Enter" && onCommit) {
          event.preventDefault();
          onCommit();
        }
      }}
    />
  );
}

/**
 * **How tall a list field is drawn** (DS-10, #1842): its rows show a line per entry, and a line
 * that wraps needs more; then the box is as tall as its text and its borders, so it never
 * scrolls inside. Nothing (`""`) while the rows show every line.
 */
export function grownTo(box: HTMLTextAreaElement): string {
  box.style.blockSize = "";
  if (box.scrollHeight <= box.clientHeight) return "";
  return `${box.scrollHeight + box.offsetHeight - box.clientHeight}px`;
}

/** A list field: the shared field at its row's width, under its name, as tall as its lines. */
function ListField(props: Extract<FieldProps, { kind: "list" }>) {
  const { ids, value } = props;
  const box = useRef<HTMLTextAreaElement>(null);
  // Grown again as it is typed in, and as its width changes the lines that wrap.
  useLayoutEffect(() => {
    const element = box.current;
    if (!element) return;
    element.style.blockSize = grownTo(element);
  }, [value]);
  useLayoutEffect(() => {
    const element = box.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    let width = element.clientWidth;
    const watch = new ResizeObserver(() => {
      if (element.clientWidth === width) return;
      width = element.clientWidth;
      element.style.blockSize = grownTo(element);
    });
    watch.observe(element);
    return () => watch.disconnect();
  }, []);
  return (
    <TextArea
      inputRef={box}
      id={ids.id}
      className="ui-setting-box ui-setting-list"
      grows
      value={value}
      rows={Math.max(props.minRows ?? 2, value.split("\n").length)}
      validate={props.validate}
      aria-label={ids.label}
      aria-labelledby={ids.label === undefined ? ids.labelledBy : undefined}
      aria-describedby={ids.describedBy}
      onChange={props.onChange}
      onBlur={() => {
        if (props.validate?.(value) === undefined) props.onCommit?.();
      }}
    />
  );
}

/** One option of a choice: its value, what it is called, and a line on what it does. The line
 *  is words, or a few marked-up facts (a harness's kind, command and source) that describe the
 *  option as one line. A radio's or a box's option can be out of reach on its own (`disabled`),
 *  with why on its `title`, drawn as its tooltip (#2908) — the primitive's own words (DS-3e); a select's, as the prompt a pick
 *  starts on and cannot be picked back to. */
export type Option = {
  value: string;
  label: string;
  /** What is drawn before a radio's label and adds nothing to its name: a persona's mark. */
  mark?: ReactNode;
  says?: ReactNode;
  disabled?: boolean;
  title?: string;
  /** How many things a chip stands for, after its words and never in its name (#3484). */
  count?: number;
};

/**
 * **A value picked**, each from its family in `ui/choice` (#2907): `radio` one of a few, each
 * with a line on what it does (the radio group); `cards` one of a few compared before picking
 * (the option cards: Start a chat's harnesses); `select` one of many (the select family,
 * `ui/select/Select.tsx`, #2906); `switch` a setting on or off, in force the moment it is turned
 * (the switch, #1860); `toggle` an option of a form on or off, done only when its button is
 * pressed (a checkbox); `checks` any of a few, each ticked on its own (a checkbox per option, in
 * a group the row names — the repos a workspace takes, the files offered to memory; DS-3e);
 * `chips` any of a few too, drawn small and side by side over the list they narrow (the Chats
 * list's state chips, #1687): **one Tab stop**, with the arrow keys between them and Space
 * ticking the one the keyboard is on, so a list's filter costs the keyboard one stop however many
 * chips it has.
 */
export type ChoiceProps = Tied &
  (
    | {
        /** `cards`: the same choice drawn as option cards, each of the same height, its mark and
         *  name on the first line and its line under them, and no radio circle (R24). */
        kind: "radio" | "cards";
        options: readonly Option[];
        value: string | undefined;
        onValueChange: (to: string) => void;
        /** The group's own `disabled`: held while what it feeds is being done. */
        disabled?: boolean;
      }
    | {
        kind: "select";
        options: readonly Option[];
        value: string | undefined;
        /** What the empty option says; none is offered without it. */
        unset?: string;
        onValueChange: (to: string) => void;
        /** The select's own `disabled`: out of reach while there is nothing to pick. */
        disabled?: boolean;
      }
    | ({ kind: "switch" } & OnOff)
    | ({ kind: "toggle" } & OnOff)
    | ({ kind: "checks" } & Ticks)
    | ({ kind: "chips" } & Ticks)
  );

/** What a choice of on or off is handed: whether it is on, and the answer. */
type OnOff = {
  checked: boolean;
  onCheckedChange: (to: boolean) => void;
  /** The control's own `disabled`: held while what it feeds is being done. */
  disabled?: boolean;
};

/** What a choice of any of a few is handed: its options, which are ticked, and the answer. */
type Ticks = {
  options: readonly Option[];
  /** The values ticked. */
  checked: ReadonlySet<string>;
  onCheckedChange: (value: string, on: boolean) => void;
};

/** The level picked from the keyboard, until the layout drawn at it takes the focus back
 *  (D-626-4). */
let arrowedTo: string | undefined;

/**
 * **The chips** of {@link Choice} (#1687): the checkbox family's chips (`ui/choice`, #2907), one
 * Tab stop with the arrows between them, Space ticking the one the keyboard is on and a press of
 * a chip's words ticking it as a press of its tick does.
 */
function Chips({ ids, options, checked, onCheckedChange }: Tied & Ticks) {
  return (
    <CheckboxGroup
      id={ids.id}
      roving
      aria-label={ids.label}
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      {options.map((one, at) => (
        // By place, not by value, as every option list of the set is keyed.
        <RovingCheckbox
          key={at}
          at={at}
          variant="chip"
          tooltip={one.title}
          isSelected={checked.has(one.value)}
          isDisabled={one.disabled}
          count={one.count}
          onChange={(on) => onCheckedChange(one.value, on)}
        >
          {one.label}
        </RovingCheckbox>
      ))}
    </CheckboxGroup>
  );
}

export function Choice(props: ChoiceProps) {
  const { ids } = props;
  if (props.kind === "chips") return <Chips {...props} />;
  if (props.kind === "switch")
    return (
      <Switch
        id={ids.id}
        tooltip={ids.title}
        isSelected={props.checked}
        isDisabled={props.disabled}
        aria-label={ids.label}
        aria-labelledby={ids.labelledBy}
        aria-describedby={ids.describedBy}
        onChange={props.onCheckedChange}
      />
    );
  if (props.kind === "toggle")
    return (
      <Checkbox
        id={ids.id}
        tooltip={ids.title}
        isSelected={props.checked}
        isDisabled={props.disabled}
        aria-label={ids.label}
        aria-describedby={ids.describedBy}
        onChange={props.onCheckedChange}
      />
    );
  if (props.kind === "checks")
    return (
      <CheckboxGroup
        id={ids.id}
        aria-label={ids.label}
        aria-labelledby={ids.labelledBy}
        aria-describedby={ids.describedBy}
      >
        {props.options.map((one, at) => (
          // By place, not by value: two options can share one (two owners' `api`, D-DS3e-10).
          <Checkbox
            key={at}
            id={`${ids.id}-${at}`}
            tooltip={one.title}
            isSelected={props.checked.has(one.value)}
            isDisabled={one.disabled}
            description={one.says}
            onChange={(on) => props.onCheckedChange(one.value, on)}
          >
            {one.label}
          </Checkbox>
        ))}
      </CheckboxGroup>
    );
  if (props.kind === "select")
    return (
      <Select
        id={ids.id}
        className={narrow(ids).join(" ") || undefined}
        tooltip={ids.title}
        value={props.value ?? ""}
        // An option out of reach is the prompt a pick starts on and cannot go back to.
        options={[
          ...(props.unset !== undefined ? [{ value: "", label: props.unset }] : []),
          ...props.options.map((one) => ({
            value: one.value,
            label: one.label,
            isDisabled: one.disabled,
            tooltip: one.title,
          })),
        ]}
        isDisabled={props.disabled}
        // Named by its row's label, which React Aria says after what it holds.
        aria-label={ids.label}
        aria-labelledby={ids.label === undefined ? ids.labelledBy : undefined}
        aria-describedby={ids.describedBy}
        onValueChange={props.onValueChange}
      />
    );
  if (props.kind === "cards")
    return (
      <OptionCards
        id={ids.id}
        value={props.value ?? null}
        onChange={props.onValueChange}
        isDisabled={props.disabled}
        aria-label={ids.label}
        aria-labelledby={ids.labelledBy}
        aria-describedby={ids.describedBy}
      >
        {props.options.map((one, at) => (
          <OptionCard
            key={at}
            id={`${ids.id}-${at}`}
            value={one.value}
            tooltip={one.title}
            isDisabled={one.disabled}
            icon={one.mark}
            title={one.label}
            detail={one.says}
          />
        ))}
      </OptionCards>
    );
  return (
    <RadioGroup
      id={ids.id}
      value={props.value ?? null}
      onChange={props.onValueChange}
      isDisabled={props.disabled}
      aria-label={ids.label}
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      {props.options.map((one, at) => (
        // By place, not by value: two options can share one (two owners' `api`, D-DS3e-10).
        <Radio
          key={at}
          id={`${ids.id}-${at}`}
          value={one.value}
          tooltip={one.title}
          isDisabled={one.disabled}
          lead={one.mark}
          description={one.says}
        >
          {one.label}
        </Radio>
      ))}
    </RadioGroup>
  );
}

/**
 * **A form's buttons** (V89j, DS-3e #1177): the row a form ends in — Save and Cancel, Use this,
 * Move, Restore, Retry — under its rows, or a dialog's answers under its form.
 *
 * A row and nothing more: the buttons are native `<button>`s the caller writes, with their own
 * `type`, `disabled` and `onClick`, so the call site still says what each one is. The row draws
 * them all alike, from the same tokens as a row's reset; a button on a page whose act cannot
 * be undone from purlis says so with `className="ends-it"` (D-1719-3,
 * `docs/ui-primitives.md`). A dialog never puts one here: a confirm that
 * ends something is a question, and ends in the dialog's footer (D-1210-8, `dialogAnswers.guard.test.ts`).
 *
 * Every button in it still needs `tabIndex={0}`: WebKit leaves a `<button>` out of the Tab order
 * without one (#190), and the row cannot add it to buttons it does not draw.
 */
export function SettingActions({ children }: { children: ReactNode }) {
  return <div className="ui-setting-actions">{children}</div>;
}
