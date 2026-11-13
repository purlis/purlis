// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The project rail family** (#2915, spec #2897 R76–R89 and N17, ADR 0104): the column of
 * projects at the window's far left, each a button with its monogram, the waiting badge and the
 * ring on the current one, then open a project and the rail's own hide. A screen draws the rail
 * from these and never its own (`purlis/rail-family` in `eslint.config.js`).
 *
 * - **Drawn by its parts.** A button carries `data-scope="rail"`, `data-part="button"` and its
 *   `data-variant`: `project` or `action`. Its monogram and its badge are the `monogram` and
 *   `badge` parts. `rail.css` draws the defaults (the Purlis look: 36px, corners of 9), and a
 *   theme's parts reach the same attributes (`rail.button.corner-radius`, R139, ADR 0103).
 * - **States a theme can key** are the ones this wrapper marks: `data-hovered` under the pointer,
 *   `data-selected` on the current project, `data-focus-visible` from the keyboard. A state
 *   changes a colour and the ring, never a size (R153).
 * - **A plain `<button>` underneath**, not React Aria's: a project's button is also a tab of the
 *   tabs family's strip (`AsStripTab`, #3134), a sortable item and a context menu's trigger, each
 *   of which hands it its handlers and its ref, as the project tabs before it were handed them.
 *   The hover and the keyboard's focus are React Aria's own hooks, so it marks them as the other
 *   families do.
 * - **Its name is its tooltip** (R89): the shared tooltip family (#2908), beside the button at its
 *   end, 10px from it. A project's button is named by the project; an action's by what it does,
 *   with its `tooltip` as a second line.
 */
import type { ComponentPropsWithoutRef, CSSProperties, ReactNode, Ref } from "react";
import { mergeProps, useFocusRing, useHover, useId } from "react-aria";
import { PanelLeft, RotateCw, X } from "lucide-react";
import { RoundCount } from "../badge/Badge";
import { IconButton } from "../button/Button";
import { Tooltip, type Tip } from "../tooltip/Tooltip";
import { useTabStripRow, type TabStrip } from "../tabs/TabStrip";

/** The family's variants, in the order the manifest names them. */
export const RAIL_VARIANTS = ["project", "action"] as const;
export type RailVariant = (typeof RAIL_VARIANTS)[number];

/** The rail's column, as wide as the prototype draws it (R76). */
export const RAIL_WIDTH = 52;

/**
 * **A project's monogram** (R77): the first letter of each part of its name, split at its
 * hyphens (and underscores, dots and spaces), two at most, in capitals. A name of one part is
 * one letter.
 */
export function monogramOf(name: string): string {
  return name
    .split(/[-_.\s]+/u)
    .filter((part) => part !== "")
    .map((part) => Array.from(part)[0])
    .slice(0, 2)
    .join("")
    .toLocaleUpperCase();
}

type Native = Omit<
  ComponentPropsWithoutRef<"button">,
  "children" | "style" | "className" | "title"
>;

/** What both buttons take: their element's own attributes, handed on (an `asChild` trigger's). */
interface Drawn extends Native {
  ref?: Ref<HTMLButtonElement>;
  /** A place for it (the sortable's transform), never a look. */
  style?: CSSProperties;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/** The hover and the keyboard's focus as the families mark them, merged over what was handed. */
function useMarked(handed: Native, selected?: boolean) {
  const { hoverProps, isHovered } = useHover({});
  const { focusProps, isFocusVisible } = useFocusRing();
  return mergeProps(handed, hoverProps, focusProps, {
    "data-hovered": isHovered ? "" : undefined,
    "data-focus-visible": isFocusVisible ? "" : undefined,
    "data-selected": selected ? "" : undefined,
  });
}

export interface RailProjectProps extends Drawn {
  /** The project's name: the button's name, its tooltip, and its monogram's letters. */
  name: string;
  /** It is the project in front: ringed (R78). */
  current: boolean;
  /** How many things wait on the person in it (R79); none draws no badge. */
  waiting?: number;
  /** Its unsaved work (charter-app#302): a dot at its bottom corner, and what the Saving tab
   *  would say, as its tooltip's second line. `blocked` is a save that cannot go on. */
  unsaved?: { blocked: boolean; says: string };
  /** Its colour, a name of the theme's palette (`projectColour.ts`), and the tokens it draws
   *  in it: its monogram's and its ground's. Absent draws the accent. */
  colour?: { colour: string | undefined; tint: Record<string, string> };
  /** How its load is going at a launch (#5323): `loading` draws the loading mark and marks it
   *  busy; `failed` draws the failed mark (#5324); `says` is said as its description and its
   *  tooltip's second line ("Loading…", "Still loading…", "2 of 3 chats back", or the error).
   *  Absent, it is loaded and says nothing of it. */
  load?: { loading: boolean; failed?: boolean; says?: string };
}

/**
 * **A project on the rail** (R77–R79): its monogram in its colour on that colour's ground, the
 * waiting badge at its top right, the dot of its unsaved work at its bottom right
 * (charter-app#302), the ring while it is current. A tab of the rail's tablist,
 * whose tab (`AsStripTab`), drag and menu hand it their props.
 */
export function RailProject({
  name,
  current,
  waiting = 0,
  unsaved,
  colour,
  load,
  ref,
  style,
  ...handed
}: RailProjectProps) {
  const marked = useMarked(handed, current);
  const said = [
    load?.says,
    waiting > 0 ? `${waiting} waiting` : undefined,
    unsaved === undefined ? undefined : unsaved.blocked ? "saving is blocked" : "unsaved work",
  ].filter((one) => one !== undefined);
  // Said as its description, beside what it is handed (a drag's instructions), which
  // `aria-description` would lose to.
  const saidId = useId();
  const describedBy =
    [
      (handed as { "aria-describedby"?: string })["aria-describedby"],
      said.length > 0 ? saidId : undefined,
    ]
      .filter(Boolean)
      .join(" ") || undefined;
  return (
    <>
      {/* Its description, beside the button and not in it: words inside it would be words of its
        own to the tooltip family, which takes a tip they repeat as said already (#5323). */}
      {said.length > 0 && (
        <span id={saidId} hidden aria-hidden="true">
          {said.join(", ")}
        </span>
      )}
      {/* Its name first, as an icon button's tooltip is, and how its load is going or what the
        Saving tab would say of its unsaved work as the second line (R89, S1, #5323). The two
        lines come from the tooltip family's icon-only rule (`Tooltip.tsx`: a button with no
        words of its own and an aria-label puts the label first and `tip` under it), so the
        button must keep showing no words of its own. */}
      <Tooltip tip={load?.says ?? unsaved?.says ?? name}>
        <button
          type="button"
          {...marked}
          ref={ref}
          aria-label={name}
          aria-describedby={describedBy}
          aria-busy={load?.loading ? true : undefined}
          data-scope="rail"
          data-part="button"
          data-variant="project"
          data-colour={colour?.colour}
          style={{ ...style, ...colour?.tint } as CSSProperties}
        >
          {/* Drawn by the stylesheet from their attributes, so the button holds no words but its
            name: its letters and its count are marks, and its name and description say them. */}
          <span
            data-scope="rail"
            data-part="monogram"
            data-letters={monogramOf(name)}
            aria-hidden="true"
          />
          {/* Its loading mark (#5323): a dashed ring inside the button, which a shape tells apart
            from the current one's solid ring and not its colour alone; its words are its
            description's. */}
          {load?.loading && <span data-scope="rail" data-part="loading" aria-hidden="true" />}
          {/* Its failed mark (#5324): a "!" at its bottom left, which a shape and not a colour
            alone says; its words are its description's and its tooltip's. */}
          {load?.failed && <span data-scope="rail" data-part="failed" aria-hidden="true" />}
          {/* The badge family's round count (#6342): one character, 9+ above 9; the real
            number is its description's. Never drawn again as it changes, so it slides. */}
          <RoundCount count={waiting} at="mark" />
          {unsaved !== undefined && (
            <span
              data-scope="rail"
              data-part="unsaved"
              data-blocked={unsaved.blocked ? "" : undefined}
              aria-hidden="true"
            />
          )}
        </button>
      </Tooltip>
    </>
  );
}

/**
 * **What a project's row offers while its load fell short** (#5324): Retry, and Dismiss on a
 * project whose open failed, which has no menu to close it from. Under its button and beside it
 * in the Tab order, never inside it: a button cannot hold a button, and the tablist holds tabs.
 */
export function RailProjectWay({
  doing,
  name,
  onPress,
}: {
  doing: "retry" | "dismiss";
  /** The project's name, which names the way: two rows can both offer Retry. */
  name: string;
  onPress: () => void;
}) {
  const retry = doing === "retry";
  return (
    <IconButton
      size="sm"
      variant="ghost"
      aria-label={`${retry ? "Retry" : "Dismiss"} ${name}`}
      tooltip={retry ? "Opens it again, and brings its chats back" : "Takes it off this list"}
      icon={retry ? <RotateCw aria-hidden="true" /> : <X aria-hidden="true" />}
      onPress={onPress}
    />
  );
}

export interface RailActionProps extends Drawn {
  /** What it does: its name, which its tooltip shows. */
  "aria-label": string;
  /** Its icon, which is all it shows. */
  icon: ReactNode;
  /** A second line under its name in its tooltip. */
  tooltip?: Tip;
}

/** **An action of the rail**: open a project (R80), hide the rail or keep it shown (R82, R88). */
export function RailAction({ icon, tooltip, ref, style, ...handed }: RailActionProps) {
  const marked = useMarked(handed);
  return (
    <Tooltip tip={tooltip ?? handed["aria-label"]}>
      <button
        type="button"
        {...marked}
        ref={ref}
        // In WebKit's tab sequence, as every button of the window says it is (#190).
        tabIndex={handed.tabIndex ?? 0}
        style={style}
        data-scope="rail"
        data-part="button"
        data-variant="action"
      >
        {icon}
      </button>
    </Tooltip>
  );
}

/**
 * **The rail's column** (R76): its tablist, which owns the projects' buttons by their ids (a
 * tablist holds tabs only, and the rail's actions are not; #1204's rule for every strip), then
 * whatever it is given: the projects, open a project, the room that pushes the last button to
 * the bottom ({@link RailSpacer}) and the hide (R82). The tablist and the keyboard along it are
 * the tabs family's (`ui/tabs/TabStrip`, #3134): Up and Down, Home and End, one Tab stop.
 */
export function Rail({
  name,
  strip,
  tabs,
  peek = false,
  children,
  ref,
  ...rest
}: {
  /** The tablist's name, which is the projects'. */
  name: string;
  /** The projects' tabs, as the tabs family holds them (`useTabStrip`). */
  strip: TabStrip;
  /** The ids of the project buttons, in their order. */
  tabs: readonly string[];
  /** It peeks over the content from the window's edge (R86), rather than standing in its
   *  column. */
  peek?: boolean;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
} & Omit<ComponentPropsWithoutRef<"div">, "children" | "className" | "style" | "title">) {
  const { row, tablist } = useTabStripRow({ strip, name, orientation: "vertical", ref });
  const { ref: own, ...along } = row;
  return (
    <div
      {...mergeProps(rest, along)}
      ref={own}
      data-scope="rail"
      data-part="column"
      data-peek={peek ? "" : undefined}
    >
      <span
        data-scope="rail"
        data-part="tablist"
        {...tablist}
        aria-owns={tabs.length > 0 ? tabs.join(" ") : undefined}
      />
      {children}
    </div>
  );
}

/**
 * **The rail's place in the window** (R84, R85): 52px wide while it is shown, none while it is
 * hidden. The width closes over `duration.slide` on the one ease, so the work beside it glides
 * with it, while the rail slides out and fades over `duration.leave`; showing is the reverse.
 * Reduced motion has none of it. Hidden, the rail is out of reach: inert, and heard by nothing.
 * `settling` draws a change with no motion at all: the rail kept shown from its peek is already
 * on screen, and settles in its column without sliding again (R88).
 */
export function RailSlot({
  hidden,
  settling = false,
  onSlid,
  label,
  children,
}: {
  hidden: boolean;
  /** What the rail navigates, its landmark's name (#2929): a screen reader's rotor lists it. */
  label: string;
  settling?: boolean;
  /** Its column has finished opening or closing (`railSlide.ts`). */
  onSlid?: () => void;
  children: ReactNode;
}) {
  return (
    // A navigation landmark (#2929): the rail is how the window goes from project to project.
    <nav
      aria-label={label}
      onTransitionEnd={(event) => {
        if (event.target === event.currentTarget && event.propertyName === "inline-size")
          onSlid?.();
      }}
      data-scope="rail"
      data-part="slot"
      data-hidden={hidden ? "" : undefined}
      data-settling={settling ? "" : undefined}
      aria-hidden={hidden || undefined}
      inert={hidden || undefined}
    >
      {children}
    </nav>
  );
}

/**
 * **The rail's projects and its +**, which scroll when the window is not tall enough for them,
 * with the room above the first for its badge and its ring, which stand outside it (R78, R79).
 */
export function RailList({ children, ref }: { children: ReactNode; ref?: Ref<HTMLDivElement> }) {
  return (
    <div data-scope="rail" data-part="list" ref={ref}>
      {children}
    </div>
  );
}

/**
 * **Something waits below the rail's fold** (ADR 0054: hiding never hides a chat that needs
 * you): a count at the list's foot while a project that waits is scrolled out of sight. A press
 * brings it into view.
 */
export function RailBelow({ waiting, onPress }: { waiting: number; onPress: () => void }) {
  const label = `${waiting} waiting below`;
  return (
    <Tooltip tip={label}>
      <button
        type="button"
        aria-label={label}
        onClick={onPress}
        data-scope="rail"
        data-part="below"
      >
        <RoundCount count={waiting} at="mark" />
      </button>
    </Tooltip>
  );
}

/** The room between the rail's projects and its last button, which it pushes to the bottom. */
export function RailSpacer() {
  return <span data-scope="rail" data-part="spacer" aria-hidden="true" />;
}

/** The sidebar mark the rail's hide, its keep and the title bar's show draw (R82, R88, R91). */
export function SidebarIcon() {
  return <PanelLeft aria-hidden="true" />;
}
