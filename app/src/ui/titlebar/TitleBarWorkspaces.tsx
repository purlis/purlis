// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The title bar's workspace family** (#2916, spec #2897 R92, R101–R106, R167 and N17, ADR
 * 0104): the project in front's pinned workspaces as chips after its name, the dashed chip that
 * pins the current workspace while it is not pinned, the strip that holds them, and the plane
 * root's house before the strip (`RootChip`, #3654). A screen draws them from these and never
 * its own (`purlis/titlebar-family` in `eslint.config.js`).
 *
 * - **Drawn by its parts.** A chip carries `data-scope="titlebar"`, `data-part="workspace"` and
 *   its `data-variant`: `pinned`, `unpinned`, `dormant` for a pin whose workspace is gone
 *   (#3651), or `root` for the plane root's house, which stands outside the strip. Its waiting count is the `count` part.
 *   `titlebar.css` draws the defaults (the Purlis look: 28px, corners of 8, quiet words) and a
 *   theme's parts reach the same attributes (`titlebar.workspace.corner-radius`, ADR 0103).
 * - **States a theme can key** are the ones this wrapper marks: `data-hovered` under the pointer,
 *   `data-selected` on the current workspace, `data-focus-visible` from the keyboard. A state
 *   changes a colour, never a size or a neighbour's place (R153).
 * - **A plain `<button>` underneath**, as the rail's is: a chip is also a context menu's trigger,
 *   which hands it its handlers and its ref (`asChild`). The hover and the keyboard's focus are
 *   React Aria's own hooks.
 * - **A chip's notice dot** (#3648, spec #3647): a hidden notice about the workspace that is not
 *   dismissed leaves a `dot` part on the chip's leading corner, opposite its count, which brings
 *   the notice's card back. Each chip stands in a `slot` the dot is placed in, so the dot coming
 *   and going moves nothing. Its shape and its part are the most urgent kind's (#3649): `dot`,
 *   a disc, for news; `dot-warning`, a triangle, for a warning.
 * - **The strip is a toolbar**, one Tab stop whose arrows walk its chips. It is at most 62% of the bar and scrolls sideways with its scrollbar hidden
 *   (R103, R167); the slash before it is a part of its own, in the faint tier (R92, R156).
 */
import type {
  ComponentPropsWithoutRef,
  CSSProperties,
  KeyboardEvent as ReactKeyboardEvent,
  ReactNode,
  Ref,
} from "react";
import { mergeProps, useFocusRing, useHover, useId } from "react-aria";
import { ChevronDown, Circle, House, Triangle, type LucideIcon } from "lucide-react";
import { Tooltip, type Tip } from "../tooltip/Tooltip";

/**
 * **The kinds of notice, least urgent first** (spec #3647): `info` ●, `warning` ▲. A chip's dot
 * shows the most urgent kind among its notices, so the order is the urgency: a kind's place in
 * this list is its rank. The needs-you kind (✋, #3652) is one more entry here and in
 * {@link KINDS}, and one more part in the theme's manifest.
 */
export const NOTICE_KINDS = ["info", "warning"] as const;
export type NoticeKind = (typeof NOTICE_KINDS)[number];

/** How urgent `kind` is: its place in {@link NOTICE_KINDS}. */
export const urgencyOf = (kind: NoticeKind): number => NOTICE_KINDS.indexOf(kind);

/** **The most urgent of `kinds`**: what a dot shows. None is `info`, the least urgent. */
export const mostUrgentOf = (kinds: Iterable<NoticeKind>): NoticeKind => {
  let top: NoticeKind = NOTICE_KINDS[0];
  for (const one of kinds) if (urgencyOf(one) > urgencyOf(top)) top = one;
  return top;
};

/**
 * **What each kind draws and says**: the dot's part, which a theme keys its colours by (one
 * part per kind, `titlebar.dot` and `titlebar.dot-warning`, so a theme colouring one never
 * colours another the same); its shape, a Lucide icon filled, never a colour alone (WCAG
 * 1.4.1); and the words a description gives it after the count, none for news.
 */
const KINDS: Record<NoticeKind, { part: string; shape: LucideIcon; said: string | undefined }> = {
  info: { part: "dot", shape: Circle, said: undefined },
  warning: { part: "dot-warning", shape: Triangle, said: "warning" },
};

/** How many notices a chip has and the most urgent kind among them, as its description says it:
 *  "1 notice", "3 notices, most urgent: warning". */
export const noticesSaid = (count: number, kind: NoticeKind = "info") => {
  const counted = count === 1 ? "1 notice" : `${count} notices`;
  const said = KINDS[kind].said;
  return said === undefined ? counted : `${counted}, most urgent: ${said}`;
};

/** The family's variants, in the order the manifest names them. */
export const TITLEBAR_VARIANTS = ["pinned", "unpinned", "root", "dormant"] as const;
export type TitleBarVariant = (typeof TITLEBAR_VARIANTS)[number];

type Native = Omit<
  ComponentPropsWithoutRef<"button">,
  "children" | "style" | "className" | "title"
>;

export interface WorkspaceChipProps extends Native {
  /** The workspace's name: what the chip says, and its name. */
  name: string;
  /** A pin, or the current workspace while it is not pinned, whose press pins it (R105), or a
   *  pin whose workspace is gone, kept dormant, whose press shows its notice (#3651). The `root`
   *  variant is `RootChip`'s alone (#3654). */
  variant?: Exclude<TitleBarVariant, "root">;
  /** It is the workspace in front (R101). */
  current?: boolean;
  /** How many things wait on the person in it (R101); none draws no count. */
  waiting?: number;
  /** Its tooltip: `‹name› · ⌘N` for the first nine pins (R102), `Pin ‹name›` on the dashed one. */
  tooltip?: Tip;
  /** The notices about it its dot shows (#3648), said in its description; none says nothing. */
  notices?: { count: number; kind: NoticeKind };
  ref?: Ref<HTMLButtonElement>;
  /** A place for it, never a look. */
  style?: CSSProperties;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/**
 * **A workspace's chip** (R101, R105): its name in the quiet tier, and its waiting count in
 * amber after it. The current one takes the card's fill and the primary words; the dashed one
 * says `‹name› +`. A chip has no × (R110): a pin is let go from the workspace popover's toggle.
 */
export function WorkspaceChip({
  name,
  variant = "pinned",
  current = false,
  waiting = 0,
  tooltip,
  notices,
  ref,
  style,
  ...handed
}: WorkspaceChipProps) {
  const { hoverProps, isHovered } = useHover({});
  const { focusProps, isFocusVisible } = useFocusRing();
  const marked = mergeProps(handed, hoverProps, focusProps, {
    "data-hovered": isHovered ? "" : undefined,
    "data-focus-visible": isFocusVisible ? "" : undefined,
    "data-selected": current ? "" : undefined,
  });
  const counted = waiting > 99 ? "99+" : String(waiting);
  const said = useId();
  // Named by its words, as drawn (WCAG 2.5.3); what waits in it is its description.
  const describedBy =
    [
      (handed as { "aria-describedby"?: string })["aria-describedby"],
      waiting > 0 || (notices?.count ?? 0) > 0 || variant === "dormant" ? said : undefined,
    ]
      .filter(Boolean)
      .join(" ") || undefined;
  const chip = (
    <button
      type="button"
      {...marked}
      ref={ref}
      style={style}
      aria-current={current ? "true" : undefined}
      aria-describedby={describedBy}
      data-scope="titlebar"
      data-part="workspace"
      data-variant={variant}
    >
      <span data-scope="titlebar" data-part="name">
        {name}
      </span>
      {variant === "unpinned" && (
        <span data-scope="titlebar" data-part="plus" aria-hidden="true">
          +
        </span>
      )}
      {waiting > 0 && (
        // The count's wash is laid on the bar's own ground, whatever the chip's fill
        // (D-3345-3): this holds that ground under it.
        <span data-scope="titlebar" data-ground="count" aria-hidden="true">
          <span data-scope="titlebar" data-part="count">
            {counted}
          </span>
        </span>
      )}
    </button>
  );
  // What waits in it and what its dot shows, beside it and not in it, so its words are only
  // what it draws.
  const describes = [
    // A dormant chip says what it is, as its dotted edge and slanted words draw it (#3651).
    variant === "dormant" ? "gone, kept dormant" : undefined,
    waiting > 0 ? `${waiting} waiting` : undefined,
    notices !== undefined && notices.count > 0
      ? noticesSaid(notices.count, notices.kind)
      : undefined,
  ].filter((one) => one !== undefined);
  const description =
    describes.length > 0 ? (
      <span id={said} hidden>
        {describes.join(", ")}
      </span>
    ) : null;
  return (
    <>
      {tooltip === undefined ? chip : <Tooltip tip={tooltip}>{chip}</Tooltip>}
      {description}
    </>
  );
}

/**
 * **Where a chip stands in the strip** (#3648): the chip, and the dot of the notices about it,
 * placed over the chip's leading corner. Its own box is the chip's, so a dot coming or going
 * moves nothing (R153).
 */
export function ChipSlot({ children }: { children: ReactNode }) {
  return (
    <span data-scope="titlebar" data-part="slot">
      {children}
    </span>
  );
}

/**
 * **A chip's notice dot** (#3648, spec #3647): a hidden notice about the workspace, not yet
 * dismissed, as a shape on the chip's leading corner, opposite its waiting count. A press
 * brings the notice's card back. It is one of the stops the strip's arrows walk, never a Tab
 * stop of its own, as every chip but one is (charter-app#189). Its glyph is the kind's shape
 * (WCAG 1.4.1); its name says what a press shows, and the chip's description says it too.
 */
export function NoticeDot({
  workspace,
  count,
  kind,
  stop = false,
  onPress,
}: {
  workspace: string;
  count: number;
  kind: NoticeKind;
  /** A Tab stop of its own, where no strip's arrows walk to it: the plane root's house's. */
  stop?: boolean;
  onPress: () => void;
}) {
  const marked = useMarked({ onClick: onPress }, false);
  const { part, shape: Shape } = KINDS[kind];
  const shows =
    count === 1 ? `Show the notice on ${workspace}` : `Show the ${count} notices on ${workspace}`;
  const said = KINDS[kind].said;
  return (
    <button
      type="button"
      {...marked}
      tabIndex={stop ? 0 : -1}
      // What a press shows, then the most urgent kind where it is not news: the shape says it
      // to the eye, this to the ear (WCAG 1.4.1).
      aria-label={said === undefined ? shows : `${shows}, most urgent: ${said}`}
      data-scope="titlebar"
      data-part={part}
      data-kind={kind}
    >
      <Shape aria-hidden="true" fill="currentColor" strokeWidth={0} />
    </button>
  );
}

/**
 * **The plane root's chip** (#3654, spec #3647): a house right after the project's name and
 * outside the strip, so it never scrolls away, the `root` variant of the workspace's chip. It is
 * always drawn and has no pin to let go. It takes the chosen look while the plane root is in
 * front, as a chip does while its workspace is; its name is the root's (`name`), as the
 * workspace popover's row and the palette say it.
 */
export function RootChip({
  name,
  current,
  notices,
  onPress,
  ref,
  ...handed
}: Omit<Native, "onClick" | "aria-label"> & {
  /** What the root is called: its name and its tooltip, the words the palette gives it. */
  name: string;
  /** The plane root is in front. */
  current: boolean;
  /** The notices about the root its dot shows (#3648), said in its description. */
  notices?: { count: number; kind: NoticeKind };
  onPress: () => void;
  ref?: Ref<HTMLButtonElement>;
  /** `data-*` attributes reach the element as they are. */
  [attribute: `data-${string}`]: string | number | boolean | undefined;
}) {
  // What a context menu around it hands it (`asChild`) reaches the button with the press.
  const marked = useMarked({ ...handed, onClick: onPress }, current);
  const said = useId();
  const shows = notices !== undefined && notices.count > 0;
  return (
    <Tooltip tip={name}>
      <button
        type="button"
        {...marked}
        ref={ref}
        // A Tab stop of its own, before the strip's one (it is not in the strip).
        tabIndex={0}
        aria-label={name}
        aria-describedby={
          [
            (handed as { "aria-describedby"?: string })["aria-describedby"],
            shows ? said : undefined,
          ]
            .filter(Boolean)
            .join(" ") || undefined
        }
        aria-current={current ? "true" : undefined}
        data-scope="titlebar"
        data-part="workspace"
        data-variant="root"
      >
        <House aria-hidden="true" />
        {shows && (
          <span id={said} hidden>
            {noticesSaid(notices.count, notices.kind)}
          </span>
        )}
      </button>
    </Tooltip>
  );
}

/**
 * **The strip of pinned workspaces** (R101, R103): a toolbar named `name`, at most 62% of the
 * bar, scrolling sideways with its scrollbar hidden. Its last control is the ⌄ that opens every
 * workspace (R106), a button of the button family handed in as a child.
 *
 * **One Tab stop, and the arrows walk it** (charter-app#189, as every strip of the window is):
 * the caller gives the stop `tabIndex` 0 and every other chip −1; ←, →, Home and End move the
 * keyboard along the chips and the ⌄, stopping at the ends.
 */
export function PinnedStrip({
  name,
  children,
  ref,
  ...rest
}: {
  name: string;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  const walk = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const items = [
      ...event.currentTarget.querySelectorAll<HTMLElement>(
        '[data-part="workspace"][data-scope="titlebar"], [data-part="dot"][data-scope="titlebar"], [data-titlebar-all]',
      ),
    ];
    const at = items.findIndex((one) => one === document.activeElement);
    if (at < 0) return;
    const to =
      event.key === "ArrowRight"
        ? Math.min(items.length - 1, at + 1)
        : event.key === "ArrowLeft"
          ? Math.max(0, at - 1)
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? items.length - 1
              : undefined;
    if (to === undefined) return;
    event.preventDefault();
    items[to].focus();
  };
  return (
    <div
      {...rest}
      ref={ref}
      role="toolbar"
      aria-label={name}
      aria-orientation="horizontal"
      onKeyDown={walk}
      data-scope="titlebar"
      data-part="strip"
    >
      {children}
    </div>
  );
}

/** **The slash** between the project's name and its pins (R92): a part of its own, faint. */
export function TitleBarSlash() {
  return (
    <span data-scope="titlebar" data-part="slash" aria-hidden="true">
      /
    </span>
  );
}

/** **The project's name**, as the bar says it while the rail is shown (R92): quiet, 500. */
export function TitleBarProject({ name }: { name: string }) {
  return (
    <span data-scope="titlebar" data-part="project" data-testid="title-project">
      {name}
    </span>
  );
}

/** The hover and the keyboard's focus as the family marks them, merged over what was handed. */
function useMarked(handed: object, selected: boolean) {
  const { hoverProps, isHovered } = useHover({});
  const { focusProps, isFocusVisible } = useFocusRing();
  return mergeProps(handed, hoverProps, focusProps, {
    "data-hovered": isHovered ? "" : undefined,
    "data-focus-visible": isFocusVisible ? "" : undefined,
    "data-selected": selected ? "" : undefined,
  });
}

/**
 * **The project's crumb while the rail is hidden** (R93): its mark, its name and ⌄, 28px with
 * corners of 8, the card's fill under the pointer and while its switcher is open. It opens the
 * two-column switcher.
 */
export function ProjectCrumb({
  name,
  mark,
  open,
  onPress,
  ref,
}: {
  name: string;
  /** The project's mark, 20px. */
  mark: ReactNode;
  open: boolean;
  onPress: () => void;
  ref?: Ref<HTMLButtonElement>;
}) {
  const marked = useMarked({ onClick: onPress }, open);
  return (
    <button
      type="button"
      {...marked}
      ref={ref}
      tabIndex={0}
      aria-haspopup="dialog"
      aria-expanded={open}
      data-scope="titlebar"
      data-part="crumb"
      data-testid="title-project"
    >
      {mark}
      <span data-scope="titlebar" data-part="crumb-name">
        {name}
      </span>
      <ChevronDown aria-hidden="true" data-scope="titlebar" data-part="chevron" />
    </button>
  );
}

/**
 * **The ⌄ that opens every workspace** (R106): 26px with corners of 7, quiet, the card's fill and
 * the primary words under the pointer and while its popover is open. Its name is "All
 * workspaces", and its tooltip says its key.
 */
export function AllWorkspaces({
  keys,
  open,
  stop,
  onPress,
  ref,
}: {
  /** The key that opens it, said in its tooltip: `⌥⌘P`. */
  keys: string;
  open: boolean;
  /** It is the strip's one Tab stop (no chip is). */
  stop: boolean;
  onPress: () => void;
  ref?: Ref<HTMLButtonElement>;
}) {
  const marked = useMarked({ onClick: onPress }, open);
  return (
    <Tooltip tip={keys}>
      <button
        type="button"
        {...marked}
        ref={ref}
        tabIndex={stop ? 0 : -1}
        aria-label="All workspaces"
        aria-haspopup="dialog"
        aria-expanded={open}
        data-scope="titlebar"
        data-part="all"
        data-titlebar-all=""
      >
        <ChevronDown aria-hidden="true" />
      </button>
    </Tooltip>
  );
}
