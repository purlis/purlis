// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  useContext,
  useId,
  useLayoutEffect,
  useRef,
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from "react";
import * as Checkbox from "@radix-ui/react-checkbox";
import * as RadioGroup from "@radix-ui/react-radio-group";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import * as Switch from "@radix-ui/react-switch";
import { Check, Info, ListFilter } from "lucide-react";
import { Select } from "../primitives/Select";
import { useTabStop } from "../roving";
import { Button } from "../ui/button/Button";
import { TextArea, TextField } from "../ui/field/Field";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { useReadOnlyReasonId } from "../readOnlyReason";

/**
 * **The house settings set** (SE-16, #1166; DS-3 #626; V89f, V89j): the six pieces every
 * settings screen and every form is drawn from — {@link SettingsLayout}, {@link SettingGroup},
 * {@link SettingRow}, {@link Field}, {@link Choice} and {@link SettingActions}.
 *
 * `docs/ui-primitives.md` forbids a house component library and allows exactly this set, for the
 * reasons written there: charter had two hand-built form styles that had already drifted
 * (`settings-*` and the dialogs' `choices`), and one small set is how a new setting stays a few
 * lines. Each piece is a thin layer over a native element or a Radix primitive already in the
 * window — radio group, checkbox, roving focus — and **says which at its call site**: the props
 * are the primitive's words (`value`, `onValueChange`, `checked`), not new ones.
 *
 * Drawn in `App.css` under `ui-*` classes, every colour a design-system token. The old
 * hand-built `settings-*`, `asks`, `choices`, `choice`, `who` and `picking` classes were deleted
 * once the last screen had moved (DS-3e, #1177), and `oldFormClasses.test.ts` keeps them gone.
 */

/**
 * **Whether a row's hint is a tooltip** (D3, D-1840-2): on a settings page, drawn by
 * {@link SettingsLayout}, a group says one help line and each row's hint sits behind an info mark
 * beside its label. A form elsewhere — a dialog's rows — keeps its hints as lines under its
 * fields, where a person filling it in reads them as they go.
 */
const QuietHints = createContext(false);

/** One level of the switcher: You, Project, Workspace or Persona (`CONTEXT.md`, **Level**). */
export type LevelOffer = { id: string; label: string };

/** One group in the nav: its stable id, and what the nav calls it. */
/** A group the nav offers. `sub` draws it as a page of the group before it (ST-4). */
export type GroupOffer = { id: string; label: string; sub?: boolean };

/**
 * **The two columns** (V89b): the level switcher and what that level is above a nav of its
 * groups, and the chosen group beside them.
 *
 * The switcher is a Radix radio group, because a level is one choice of a few; it lists only
 * the levels it is handed, so a level with nothing to set is not offered. **An arrow goes to the
 * next level** (D-626-4), as every {@link Choice} radio picks and the WAI-ARIA radio pattern
 * asks, with `Choice`'s own repair ({@link useArrowPick}): a level is a view and writes nothing,
 * so nothing holds the pick for a button. The caller may draw the new level with a layout of
 * its own (`SettingsTab` does), so the switcher the arrow left is gone; the one drawn next takes
 * the focus back on the level arrowed to, and only then. The nav is buttons in
 * one Tab stop, moved through with the arrow keys (`roving.ts`, as the explorer's rows are), and
 * the chosen one says so with `aria-current`.
 *
 * **The filter** (SE-21, V89c) sits above the nav: a search box, labelled, whose words the
 * caller narrows its groups by — the layout draws what it is handed. While it holds words, a
 * polite live region under it says how many settings match (`found`), and when none do, the
 * right column says so where the group would be — above what the level's files refuse, which
 * stays on screen whatever the filter. Escape in the box clears it.
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
  found,
  foot,
  children,
  holder,
}: {
  levels: readonly LevelOffer[];
  level: string;
  onLevelChange: (level: string) => void;
  /** One sentence on whose settings these are and where they are kept. */
  about?: ReactNode;
  groups: readonly GroupOffer[];
  group: string;
  onGroupChange: (group: string) => void;
  /** The words in the filter box, as typed. */
  filter: string;
  onFilterChange: (to: string) => void;
  /** How many settings the filter leaves, while it holds words; unset while the level is still
   *  being read, so nothing is said yet. */
  found?: number;
  /** What sits under the nav, whatever the filter. */
  foot?: ReactNode;
  /** The chosen group: a {@link SettingGroup}. */
  children: ReactNode;
  /** The tab's own element, for what is looked for in this tab and not the whole window. */
  holder?: Ref<HTMLDivElement>;
}) {
  const stop = useTabStop(
    group,
    groups.map((one) => one.id),
  );
  const { arrowing, listen } = useArrowPick();
  const switcher = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (arrowedTo !== level) return;
    arrowedTo = undefined;
    const shown = switcher.current?.querySelector<HTMLElement>(
      '[role="radio"][aria-checked="true"]',
    );
    if (shown && document.activeElement !== shown) shown.focus();
  }, [level]);
  const words = filter.trim();
  const none =
    words !== "" && found === 0 ? `No setting at this level matches “${words}”.` : undefined;
  const said =
    words === "" || found === undefined
      ? ""
      : (none ?? `${found} ${found === 1 ? "setting matches" : "settings match"}`);
  return (
    <div className="ui-settings" ref={holder}>
      <div className="ui-settings-top">
        <RadioGroup.Root
          ref={switcher}
          className="ui-levels"
          aria-label="Level"
          orientation="horizontal"
          value={level}
          onValueChange={onLevelChange}
          {...listen}
        >
          {levels.map((one) => (
            <RadioGroup.Item
              key={one.id}
              className="ui-level"
              value={one.id}
              onFocus={() => {
                if (!arrowing.current || one.id === level) return;
                arrowedTo = one.id;
                // Forgotten once this press is done, so a level the caller did not show never
                // takes the focus later.
                setTimeout(() => {
                  if (arrowedTo === one.id) arrowedTo = undefined;
                });
                onLevelChange(one.id);
              }}
            >
              {one.label}
            </RadioGroup.Item>
          ))}
        </RadioGroup.Root>
        {about !== undefined && <p className="ui-settings-about">{about}</p>}
      </div>
      <div className="ui-settings-side">
        <TextField
          className="ui-settings-filter"
          type="search"
          icon={<ListFilter aria-hidden="true" />}
          keepsFocus="settings-filter"
          value={filter}
          aria-label="Filter settings"
          placeholder="Filter settings"
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
        <p className="sr-only" role="status" aria-live="polite" aria-label="Settings found">
          {said}
        </p>
        <RovingFocusGroup.Root asChild orientation="vertical" {...stop}>
          <nav className="ui-settings-nav" aria-label="Groups">
            {groups.map((one) => (
              <RovingFocusGroup.Item key={one.id} asChild tabStopId={one.id}>
                <button
                  type="button"
                  className={
                    one.sub ? "ui-settings-nav-item ui-settings-sub" : "ui-settings-nav-item"
                  }
                  aria-current={one.id === group ? "true" : undefined}
                  onClick={() => onGroupChange(one.id)}
                >
                  {one.label}
                </button>
              </RovingFocusGroup.Item>
            ))}
          </nav>
        </RovingFocusGroup.Root>
        {foot}
      </div>
      <div className="ui-settings-body">
        {none && <p className="ui-settings-none">{none}</p>}
        <QuietHints.Provider value={true}>{children}</QuietHints.Provider>
      </div>
    </div>
  );
}

/** **One group**, the right column: its heading, what it is for, and its rows. A region named
 *  by its heading, so a screen reader can say where it is.
 *
 *  **Where its values are kept is said here, once** (`kept`, DS-8 #1840): the file and who sees
 *  it, at the end of the group's one help line, never under each row. A row whose value is kept
 *  anywhere else says so with its badge ({@link SettingRow}). */
export function SettingGroup({
  label,
  help,
  kept,
  children,
}: {
  label: string;
  help?: string;
  /** Where the group's values are kept, and who sees them: "Kept in purlis.toml, which your
   *  team sees." */
  kept?: string;
  children: ReactNode;
}) {
  const heading = useId();
  const said = [help, kept].filter(Boolean).join(" ");
  return (
    <section className="ui-setting-group" aria-labelledby={heading}>
      <h3 id={heading}>{label}</h3>
      {said && <p className="ui-setting-help">{said}</p>}
      {children}
    </section>
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
};

/** A row's way back to the value beneath it. */
export type Reset = { label: string; disabled: boolean; onReset: () => void };

/** **Where a row's value comes from**, when that is not where its group keeps its values: the
 *  level or the file in a word or two (`label`: Team, This machine, Workspace), and the whole
 *  sentence (`says`), which the badge's tooltip holds. */
export type Badge = { label: string; says: string };

/**
 * **One setting**, on one line: its label, its control and its reset, at one height — the
 * control height every button, field and select is (ADR 0094). On a settings page its line of
 * help is not drawn under it (D3): an info mark beside the label shows it in a tooltip, and the
 * control is described by it. In a dialog's form the help is a line under the field. The control is drawn by `control`, handed the ids that tie it to the label and the help
 * — a {@link Field} or a {@link Choice}.
 *
 * A `grouped` row's control is a group of controls (a radio group), which a `<label for>` cannot
 * name: its name is drawn as plain text, and the group is named by `aria-labelledby` instead.
 *
 * **A write the core refused is said here** (`error`, SE-17, V89e): beside the setting, as an
 * alert, and in the control's description, while the control shows what is on disk. And the
 * last change's **Undo** sits in the row it was made in.
 *
 * **Where the value comes from** (SE-18, V89d; drawn quietly since DS-8, #1840): the group says
 * once where its values are kept ({@link SettingGroup}'s `kept`), so a row says nothing more
 * unless its value is somewhere else. Then `badge` names the level beside the label — Team, This
 * machine, Workspace — and its tooltip holds the whole sentence. `origin`, that same sentence for
 * every row, is never drawn: it is the control's description, so a screen reader still hears
 * where each value comes from. `modified` marks a value set at this level, the one Reset takes
 * out. `place` is the row's "Move to…", at the end of the control's line.
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
  place,
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
  /** The row's "Move to…" between the files a value may be kept in (SE-18). */
  place?: ReactNode;
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
  const quiet = useContext(QuietHints);
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
  return (
    <div
      className={grouped ? "ui-setting-row ui-setting-row-grouped" : "ui-setting-row"}
      data-setting={setting}
    >
      <div className="ui-setting-line">
        <div className="ui-setting-name">
          {modified && (
            <span className="ui-setting-modified" role="img" aria-label="Set at this level" />
          )}
          {grouped ? (
            <span className="ui-setting-label" id={labelledBy}>
              {label}
            </span>
          ) : (
            <label className="ui-setting-label" id={labelledBy} htmlFor={id}>
              {label}
            </label>
          )}
          {help && quiet && <HintMark label={label} help={help} />}
          {badge && <OriginBadge badge={badge} />}
        </div>
        <div className="ui-setting-control">
          {control({ id, labelledBy, describedBy })}
          {reset && (
            <Button
              isDisabled={reset.disabled}
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
          {place}
        </div>
      </div>
      {help && (
        // On a settings page never drawn (D3: one help line per group, not per field): the info
        // mark's tooltip shows it, and it stays the control's description for a screen reader.
        <p className={quiet ? "sr-only" : "ui-setting-help"} id={described}>
          {help}
        </p>
      )}
      {origin && (
        <p className="sr-only" id={from}>
          {origin}
        </p>
      )}
      {failed && (
        <div className="ui-setting-error" id={refused} role="alert">
          {error.map((why, at) => (
            <p key={at}>{why}</p>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * **A row's hint** (DS-8, #1840; D3, D-1840-2): what the value does, behind an info mark beside
 * the label, in a tooltip — so a group draws one help line, its own, and no row draws another.
 * The mark takes the keyboard so the tooltip can be read without a pointer, and is named "About
 * <label>"; a screen reader also hears the hint as the control's own description.
 */
function HintMark({ label, help }: { label: string; help: ReactNode }) {
  return (
    <Tooltip tip={help}>
      <span className="ui-setting-hint" role="img" aria-label={`About ${label}`}>
        <Info className="node-icon" aria-hidden />
      </span>
    </Tooltip>
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
      <span className="ui-setting-badge">{badge.label}</span>
    </Tooltip>
  );
}

/** What every field and choice is handed: the row's ids. */
type Tied = { ids: RowIds };

/** With no row, a box or a select is never wider than the cell or the line it is drawn in, so a
 *  long value does not push a table past its page. */
const narrow = (ids: RowIds) => (ids.label === undefined ? [] : ["min-w-0", "max-w-full"]);

/** A select's look and place: the one select's own `ui-select`, one control tall like the Reset
 *  beside it (DS-8, #1840). */
const rowless = (ids: RowIds) => ["ui-field", ...narrow(ids)].join(" ");

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
      <span className="ui-field ui-field-range">
        <input
          id={ids.id}
          type="range"
          min={props.min}
          max={props.max}
          step={props.step ?? 1}
          value={props.value}
          aria-valuetext={(props.spoken ?? props.shown)(props.value)}
          aria-describedby={ids.describedBy}
          onChange={(event) => props.onChange(event.currentTarget.valueAsNumber)}
        />
        <output htmlFor={ids.id} aria-live="polite">
          {props.shown(props.value)}
        </output>
      </span>
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
};

/**
 * **A value picked**: `radio` one of a few, each with a line on what it does (a Radix radio
 * group); `select` one of many (the one select, Radix's, `primitives/Select.tsx`); `switch` a
 * setting on or off, in force the moment it is turned (a Radix switch, #1860); `toggle` an option
 * of a form on or off, done only when its button is pressed (a Radix checkbox);
 * `checks` any of a few, each ticked on its own (a Radix checkbox per option, in a group the row
 * names — the repos a workspace takes, the files offered to memory; DS-3e); `chips` any of a
 * few too, drawn small and side by side over the list they narrow (the Chats list's state
 * chips, #1687): **one Tab stop**, with the arrow keys between them (Radix roving focus,
 * `roving.ts`) and Space ticking the one the keyboard is on (a native checkbox in each chip),
 * so a list's filter costs the keyboard one stop however many chips it has.
 */
export type ChoiceProps = Tied &
  (
    | {
        kind: "radio";
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

/** The keys a radio group moves its pick with (Radix's own list). */
const ARROWS = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"];

/** The level an arrow went to, until the layout drawn at it takes the focus back (D-626-4). */
let arrowedTo: string | undefined;

/**
 * **A radio group whose pick follows the arrow keys**: `arrowing` says whether an arrow key is
 * down in the group, so the item the focus moves to is picked; `listen` goes on the group's
 * `RadioGroup.Root`, and each item's `onFocus` picks while `arrowing` holds. Every radio group
 * the set draws has it — {@link Choice}'s radios and {@link SettingsLayout}'s levels.
 *
 * Radix means a radio's pick to follow the arrow keys, and learns that one is down from a
 * `keydown` listener on `document`, which hears the key only after React's handlers (on the
 * root and on each portal, below `document`) have set the focus moving. So one press moved
 * the focus and picked nothing, and only a held key picked (`docs/ui-primitives.md`, "A radio
 * group's pick does not follow the arrow keys on its own"). Heard here, in the capture phase,
 * it is known before the focus moves, and forgotten when the key comes up or the focus leaves
 * the group. A choice that writes something with no Undo, or starts something, still holds
 * the pick and acts on a button (DS-3b, DS-3d).
 */
export function useArrowPick() {
  const arrowing = useRef(false);
  const listen = {
    onKeyDownCapture: (event: KeyboardEvent) => {
      if (ARROWS.includes(event.key)) arrowing.current = true;
    },
    onBlurCapture: (event: FocusEvent<HTMLElement>) => {
      // Out of the group, an arrow held on the way out is no longer a move between options.
      if (!event.currentTarget.contains(event.relatedTarget as Node | null))
        arrowing.current = false;
    },
    onKeyUpCapture: () => {
      // After the move, not before it: the roving focus moves on a timer of its own, set
      // when the key went down, so a quick press is up before the focus arrives.
      setTimeout(() => {
        arrowing.current = false;
      });
    },
  };
  return { arrowing, listen };
}

/** A chip's look: its edge and ground say it is on as well as its tick, never colour alone. */
const CHIP =
  "inline-flex flex-none items-center gap-1 rounded-full border border-border-subtle px-1.5 " +
  "text-sm whitespace-nowrap text-text-secondary hover:bg-surface-hover " +
  "data-on:border-text-secondary data-on:bg-surface-hover data-on:text-text-primary";

/**
 * **The chips** of {@link Choice}: one Tab stop, the arrows between them. Each is a native
 * checkbox in its chip's label, as the list's filter has always drawn one: the browser ticks it
 * on Space and on a press of the chip's words, and a screen reader hears a checkbox named by
 * the chip. The roving focus gives each its `tabIndex`, 0 or -1, so the WebKit fix the set's
 * other boxes carry (#186) is not wanted here: it would make every chip a stop of its own.
 */
function Chips({ ids, options, checked, onCheckedChange }: Tied & Ticks) {
  const chipId = (at: number) => `${ids.id}-${at}`;
  const stop = useTabStop(
    undefined,
    options.map((_, at) => chipId(at)),
  );
  return (
    <RovingFocusGroup.Root asChild orientation="horizontal" {...stop}>
      <div
        id={ids.id}
        role="group"
        className="flex flex-wrap gap-1"
        aria-label={ids.label}
        aria-labelledby={ids.labelledBy}
        aria-describedby={ids.describedBy}
      >
        {options.map((one, at) => {
          const on = checked.has(one.value);
          return (
            // By place, not by value, as every option list of the set is keyed.
            <Tooltip key={at} tip={one.title}>
              <label className={CHIP} data-on={on || undefined}>
                <RovingFocusGroup.Item asChild tabStopId={chipId(at)}>
                  <input
                    type="checkbox"
                    className="m-0"
                    checked={on}
                    disabled={one.disabled}
                    onChange={(event) => onCheckedChange(one.value, event.currentTarget.checked)}
                  />
                </RovingFocusGroup.Item>
                {one.label}
              </label>
            </Tooltip>
          );
        })}
      </div>
    </RovingFocusGroup.Root>
  );
}

export function Choice(props: ChoiceProps) {
  const { ids } = props;
  const { arrowing, listen } = useArrowPick();
  if (props.kind === "chips") return <Chips {...props} />;
  if (props.kind === "switch")
    return (
      <Tooltip tip={ids.title}>
        <Switch.Root
          id={ids.id}
          className="ui-switch"
          // #186: Radix's switch is a `<button>`, and WebKit leaves one out of the Tab order
          // without `tabIndex`.
          tabIndex={0}
          checked={props.checked}
          disabled={props.disabled}
          aria-label={ids.label}
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
          onCheckedChange={props.onCheckedChange}
        >
          <Switch.Thumb className="ui-switch-thumb" />
        </Switch.Root>
      </Tooltip>
    );
  if (props.kind === "toggle")
    return (
      <Checkbox.Root
        id={ids.id}
        className="ui-checkbox"
        // #186: Radix's checkbox is a `<button>`, and WebKit leaves one out of the Tab order
        // without `tabIndex` — New project's box was reachable by nothing until it had one.
        tabIndex={0}
        checked={props.checked}
        disabled={props.disabled}
        aria-describedby={ids.describedBy}
        onCheckedChange={(to) => props.onCheckedChange(to === true)}
      >
        <Checkbox.Indicator>
          <Check />
        </Checkbox.Indicator>
      </Checkbox.Root>
    );
  if (props.kind === "checks")
    return (
      <div
        id={ids.id}
        role="group"
        className="ui-choice-checks"
        aria-label={ids.label}
        aria-labelledby={ids.labelledBy}
        aria-describedby={ids.describedBy}
      >
        {props.options.map((one, at) => (
          // By place, not by value: two options can share one (two owners' `api`, D-DS3e-10).
          <div className="ui-choice-option" key={at}>
            <Tooltip tip={one.title}>
              <Checkbox.Root
                id={`${ids.id}-${at}`}
                className="ui-checkbox"
                // #186: WebKit leaves a `<button>` out of the Tab order without `tabIndex`.
                tabIndex={0}
                checked={props.checked.has(one.value)}
                disabled={one.disabled}
                aria-describedby={one.says ? `${ids.id}-${at}-says` : undefined}
                onCheckedChange={(to) => props.onCheckedChange(one.value, to === true)}
              >
                <Checkbox.Indicator>
                  <Check />
                </Checkbox.Indicator>
              </Checkbox.Root>
            </Tooltip>
            <label htmlFor={`${ids.id}-${at}`}>{one.label}</label>
            {one.says && (
              <div className="ui-choice-says" id={`${ids.id}-${at}-says`}>
                {one.says}
              </div>
            )}
          </div>
        ))}
      </div>
    );
  if (props.kind === "select")
    return (
      <Tooltip tip={ids.title}>
        <Select
          id={ids.id}
          className={rowless(ids)}
          value={props.value ?? ""}
          // An option out of reach is the prompt a pick starts on and cannot go back to.
          options={[
            ...(props.unset !== undefined ? [{ value: "", label: props.unset }] : []),
            ...props.options,
          ]}
          disabled={props.disabled}
          aria-label={ids.label}
          aria-describedby={ids.describedBy}
          onValueChange={props.onValueChange}
        />
      </Tooltip>
    );
  return (
    <RadioGroup.Root
      id={ids.id}
      className="ui-choice-radio"
      value={props.value ?? ""}
      onValueChange={props.onValueChange}
      disabled={props.disabled}
      aria-label={ids.label}
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
      {...listen}
    >
      {props.options.map((one, at) => (
        // By place, not by value: two options can share one (two owners' `api`, D-DS3e-10).
        <div className="ui-choice-option" key={at}>
          <Tooltip tip={one.title}>
            <RadioGroup.Item
              className="ui-radio"
              value={one.value}
              id={`${ids.id}-${at}`}
              disabled={one.disabled}
              aria-describedby={one.says ? `${ids.id}-${at}-says` : undefined}
              onFocus={() => {
                if (
                  arrowing.current &&
                  !props.disabled &&
                  !one.disabled &&
                  one.value !== props.value
                )
                  props.onValueChange(one.value);
              }}
            >
              <RadioGroup.Indicator className="ui-radio-mark" />
            </RadioGroup.Item>
          </Tooltip>
          <label htmlFor={`${ids.id}-${at}`}>
            {one.mark}
            {one.label}
          </label>
          {one.says && (
            <span className="ui-choice-says" id={`${ids.id}-${at}-says`}>
              {one.says}
            </span>
          )}
        </div>
      ))}
    </RadioGroup.Root>
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
