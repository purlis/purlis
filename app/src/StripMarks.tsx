// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState, type ReactNode } from "react";
import clsx from "clsx";
import { AnchoredMenu, Menu, MenuItem, opensOnTheArrows } from "./ui/menu/Menu";
import {
  ArrowLeftRight,
  ChevronDown,
  FolderOpen,
  FolderPlus,
  Minus,
  PanelRight,
  Pin as PinMark,
  Plus,
  Settings as SettingsMark,
  SquareSplitHorizontal,
  SquareSplitVertical,
  SquareArrowOutUpRight,
  X,
} from "lucide-react";
import { type Offer } from "./actions";
import { useArrived } from "./lib/arrived";
import { Button } from "./ui/button/Button";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { tabPart } from "./ui/tabs/Tabs";

/**
 * **The strips' own controls** (#1035, split out of `PlaneView.tsx`): a catalogue row drawn as a
 * button (`Doer`), its icon (`MARKS`), the show-more menu of what a full strip is not drawing
 * (`ShowMore`), a tab's pin, its `×` (`Closer`) and the focused workspace's gear (`Gear`). The
 * project, workspace and chat strips all draw them, so they are one look in one module.
 */

/**
 * Whether a row is a close that says it ends nothing — a pane or a tab showing only a view.
 *
 * Asked the other way round from `PlaneView`'s `endsAChat` on purpose: the danger look stays
 * on every close that has not said it is harmless, including a row the catalogue cannot run
 * right now, because a look that goes quiet when a row is merely unavailable would teach the
 * operator the wrong thing about the day it is available.
 */
export function closesOnly(does: Offer["does"]): boolean {
  // A task sent back out of its tab or pane ends nothing either (#1489).
  if (does.verb === "sendBack") return true;
  return (does.verb === "closeTab" || does.verb === "closePane") && !does.ends;
}

/** A button that IS a row of the catalogue: its words, its availability and its reason.
 *
 *  Nothing is drawn for an id the catalogue no longer has. That is the point: the bar cannot
 *  keep offering something the one list has stopped offering, because there is no second
 *  place for the words to live. */
export function Doer({
  offer,
  onPress,
  iconOnly,
  words,
  primary,
}: {
  offer?: Offer;
  onPress: (offer: Offer) => void;
  /**
   * What this button SAYS, where the surface it is on asks a different question from the bar.
   *
   * **It overrides the words and nothing else.** What the row does, whether it can run and why
   * not are still the catalogue's, and a row the catalogue has stopped offering still draws no
   * button — which is the whole of what `actions.ts` being the one list buys. What it does not
   * buy, and never claimed to, is that one verb has one phrasing on every surface.
   *
   * It exists for the empty state, and the argument is an accessibility one rather than a
   * stylistic one: the strip's `+` is already named `New tab`, and a second button with the
   * identical accessible name on the same screen is two controls a screen reader cannot tell
   * apart. A toolbar button says *what this control is*; a call to action in the middle of an
   * empty page says *what to do now*. Those are different sentences about one verb.
   */
  words?: string;
  /**
   * Drawn as its mark alone, with the row's words carried by `aria-label`.
   *
   * **For the controls at the end of a strip, and nothing else**: the `+`, the project strip's
   * open and create, and its switcher (FR-27). `docs/design-system.md` says an
   * icon goes *beside* words and never instead of them, with one exception — a control whose
   * accessible name is already `aria-label` — and this is that exception said out loud rather
   * than a second rule. It is the operator's own instruction for the project strip's opener
   * ("just icon"), and a `+` at the end of a row of tabs is the one glyph in this window that
   * every operator already reads, from every browser and from Zed.
   *
   * A row with no mark in `MARKS` keeps its words even here: an icon-only button with no icon
   * is an empty box, and the right way to fail is to look wrong rather than to disappear.
   */
  iconOnly?: boolean;
  /** Drawn as the shared primary button: an empty state's way out (DS-12), its words always. */
  primary?: boolean;
}) {
  if (!offer) return null;
  const Mark = MARKS[offer.id];
  const bare = iconOnly && Mark !== undefined;
  if (primary)
    return (
      <Button
        variant="primary"
        isDisabled={!offer.available}
        tooltip={offer.reason || offer.note || undefined}
        onPress={() => onPress(offer)}
      >
        {Mark && <Mark />}
        {words ?? offer.title}
      </Button>
    );
  return (
    <Tooltip tip={offer.reason || offer.note || (bare ? offer.title : undefined)}>
      <button
        className={clsx(
          offer.id === "pane.close" && !closesOnly(offer.does) && "ends-a-chat",
          bare && "bare",
        )}
        // WebKit leaves a `<button>` out of the tab sequence unless its `tabindex` is written down
        // (`docs/ui-primitives.md`, charter-app#189).
        tabIndex={0}
        disabled={!offer.available}
        aria-label={bare ? (words ?? offer.title) : undefined}
        onClick={() => onPress(offer)}
      >
        {Mark && <Mark />}
        {!bare && (words ?? offer.title)}
      </button>
    </Tooltip>
  );
}

/**
 * The icon beside each of the bar's own buttons, by catalogue row.
 *
 * **Beside the words, never instead of them.** An icon-only bar is a bar an operator has to
 * learn, and the words are what `pressOnly("New tab")` and a screen reader find — Lucide hides
 * a nameless icon from assistive technology by itself, so each button's name is its title
 * exactly as before. A row with no entry here draws its words alone, which is the right way to
 * fail: a missing icon is cosmetic, a missing button is not.
 *
 * `pane.close` ends a chat, so its mark is the same `X` a tab's close carries and it gets the
 * same danger hover (`App.css`, `.ends-a-chat`) — an icon may not make ending a chat look
 * lighter than it is.
 *
 * **The project strip draws three of these side by side, so no two may be the same glyph**
 * (charter-app#178). The third is the switcher's `ArrowLeftRight` (FR-27), first of the three:
 * two ways along the strip, which is what it does. `FolderPlus` is the folder-with-a-plus every file manager puts on *New
 * folder*, and `project.create` is the row that writes a directory that was not there; opening
 * one that already exists is `FolderOpen`, which is that same universal pair's other half.
 * `project.open` wore `FolderPlus` only because it was the strip's one control when #171 drew
 * it — two icon-only buttons an inch apart carrying one glyph is a strip an operator has to
 * aim at by memory.
 *
 * **`workspace.create` and `chat.new` share `Plus`, and that is the rule rather than an
 * oversight of the one above** (charter-app#193). Each is the ONE control at the end of its
 * own strip, a whole row apart from the other, and what both mean is the same thing: make one
 * more of what this strip lists. #178's rule is about two controls side by side; this is the
 * `+` an operator learns once and then reads on every strip in the window.
 */
export const MARKS: Record<string, typeof Plus> = {
  "chat.new": Plus,
  "workspace.create": Plus,
  "pane.split.right": SquareSplitHorizontal,
  "pane.split.down": SquareSplitVertical,
  "pane.close": X,
  "project.create": FolderPlus,
  "project.open": FolderOpen,
  // The title bar's switcher (FR-27): two ways along the strip, which is what it does.
  "project.switch": ArrowLeftRight,
};

/**
 * The show-more menu: the tabs the strip is not showing, most recently moved first.
 *
 * **It is not a find surface, and if it is built as one it should not have been built**
 * (ADR 0039). The palette lists every chat with a search and a ranking over it and is better
 * at finding than any menu will be. This lists what the strip is hiding and nothing else, so
 * that the strip has an affordance saying there is more — which a scrollbar never was.
 *
 * **Sorted by last activity — here, and nowhere else.** The strip's own order never moves
 * (`tabs.ts`), because a tab that moves under the cursor breaks aiming. A menu is a list you
 * read rather than a surface you aim at, and the boundary between the two rules is exactly
 * whether the thing moves under your hand.
 *
 * **And it carries the needs-you count of everything it hides** (ADR 0054). The operator's
 * constraint was that hiding a workspace must never hide a chat that needs you, so the
 * button draws the sum of its rows' counts in the red their own tabs draw it in, says it in
 * its name, and lists the rows that need you first. Nothing hidden is moved onto the strip
 * because it needs you: that would move tabs under the operator's hand, which is the one
 * thing ADR 0039 refuses, and the count and the title bar's ✋ menu already say it.
 *
 * **After those, the caller's order**, which on all three strips is most recently moved first:
 * a tab by its chats' newest move, a workspace by its chats', and a project by the newest move
 * its `PlaneView` reports (`PlaneReport.moved`, charter#401).
 *
 * **Only rows that bring a tab forward.** Every row is the catalogue's `tab.select:<id>`,
 * which is the same row the tab itself is and the same row the palette lists. Nothing
 * destructive is in here: a tab's `×` sits under the pointer on a surface the operator chose
 * to open, and a menu that pops up under the cursor with `End chat` in it is charter-app#130's
 * defect with a mouse attached.
 *
 * The menu family's (`ui/menu/Menu.tsx`, `docs/ui-primitives.md`), so the keyboard is React
 * Aria's and not a fifth hand-written `ArrowDown`.
 */
export function ShowMore({
  noun,
  hidden,
  onPress,
}: {
  /**
   * What one of these is, for the button's own words: `tab`, `workspace`, `project`.
   *
   * **Three strips, three nouns, one component.** A window drawing three of these owes an
   * operator — and a scenario spec — an answer to which strip is not showing everything, and
   * three buttons all saying "Show 3 more" is three answers to one query. `tab` is the chat
   * strip's, unchanged, because that is the name the operator reads on that strip.
   */
  noun: string;
  /** What to list, in the order it is listed in among the rows that need you and among the
   *  rows that do not. The first come first, whatever the order they were handed in. */
  hidden: readonly Hidden[];
  onPress: (offer: Offer) => void;
}) {
  /**
   * Whether the menu is up.
   *
   * **Held here, and opened by the click, and the reason is measured.** A menu opened on
   * `pointerdown` is right for a mouse and is not what every way of pressing a button
   * produces: the WebView the scenario tests drive answers a click with no pointer event at
   * all, so the menu never opened and the run reported "0 menus opened" on both platforms. A
   * control an automated press cannot open is one some input method cannot open. So the click
   * is what toggles it (`AnchoredMenu`), and a press by Enter or Space lands on its first row.
   */
  const [open, setOpen] = useState<boolean | "first" | "last">(false);
  const trigger = useRef<HTMLButtonElement>(null);
  // The strip has just started hiding tabs, as opposed to having been hiding them when this
  // strip was drawn: only the first is a change worth drawing (`useArrived`, and the motion
  // section of `App.css`).
  const arrived = useArrived(hidden.length > 0);
  // Nothing is hidden, so there is nothing to say there is more OF.
  if (hidden.length === 0) return null;
  const many = hidden.length === 1 ? `1 ${noun}` : `${hidden.length} ${noun}s`;
  // What is waiting behind it, from the same queue each row's own count is read from.
  const needs = hidden.reduce((sum, one) => sum + one.needs, 0);
  const needsSaid =
    needs === 0 ? "" : `, where ${needs === 1 ? "1 chat needs" : `${needs} chats need`} you`;
  // What needs you first, and the caller's order inside each half: `sort` is stable.
  const listed = [...hidden].sort((one, other) => Number(other.needs > 0) - Number(one.needs > 0));
  return (
    // **Not modal**, as every menu of the family is: a modal surface marks the rest of the
    // window `aria-hidden` (which `docs/ui-primitives.md` records the dialogs doing), and this
    // is a menu on a strip, not a question that has to be answered before the window can be
    // used again. A click outside closes it, which is what every menu on every platform does.
    <>
      <button
        ref={trigger}
        className={arrived ? "show-more arrived" : "show-more"}
        aria-label={`Show ${many} the strip is not showing${needsSaid}`}
        aria-haspopup="menu"
        aria-expanded={open !== false}
        tabIndex={0}
        onPointerDown={(event) => event.preventDefault()}
        // A press by Enter or Space (no pointer) lands on the menu's first row.
        onClick={(event) => setOpen((up) => (up ? false : event.detail === 0 ? "first" : true))}
        onKeyDown={opensOnTheArrows(open !== false, setOpen)}
      >
        {hidden.length} more
        {needs > 0 && <span className="show-more-needs">{needs}</span>}
        <ChevronDown />
      </button>
      <AnchoredMenu
        triggerRef={trigger}
        isOpen={open !== false}
        autoFocus={open === false ? true : open}
        onOpenChange={setOpen}
      >
        <Menu className="more-menu" align="end" aria-label={`${many} the strip is not showing`}>
          {listed.map(({ key, offer, children }) => {
            if (!offer) return null;
            return (
              <MenuItem
                key={key}
                id={key}
                textValue={offer.title}
                isDisabled={!offer.available}
                tooltip={offer.reason || undefined}
                onAction={() => onPress(offer)}
              >
                {children}
              </MenuItem>
            );
          })}
        </Menu>
      </AnchoredMenu>
    </>
  );
}

/** One row of a show-more menu: what it is drawn as, and the catalogue row it carries out.
 *
 *  **The strip and the menu draw the same thing**, so the caller hands the same markup to
 *  both rather than the menu having a second idea of what a workspace looks like. */
export type Hidden = {
  key: string;
  /** The row that brings it forward. Nothing is listed for an id the catalogue has dropped. */
  offer?: Offer;
  /** How many chats in it need you, read from the same queue as the tab counts: a
   *  workspace's or a project's is its tab's own count, and a chat tab's is how many of its
   *  panes' chats are in the queue. */
  needs: number;
  children: ReactNode;
};

/**
 * The mark on something the operator pinned (ADR 0039).
 *
 * **On the workspace strip a pin is also what puts a tab there at all** (ADR 0054): that strip
 * draws the pinned workspaces and the one you are in. On the other two a pin draws its tab
 * first.
 *
 * **A mark and not a button, and that is the whole of pinning's surface on a strip.** A `📌`
 * control on every tab is fifty more controls on the one strip that already broke at fifty
 * (charter-app#130), and a pin is a deliberate, occasional act — which is what the palette is
 * for. So the rows live in `actions.ts` like every other action, the palette is where they
 * are run, and this says which things carry one.
 *
 * **Lucide's pin, and not the `📌` this comment used to refuse.** The objection was to an
 * emoji — drawn by the operating system at its own size and in its own colours, louder than
 * the state dot beside it. A Lucide icon is none of those: a stroke in `currentColor`, at the
 * size and in the colour the stylesheet gives it.
 *
 * **Small, neutral, and in its tab's top corner** (qw337, `App.css`, `.pinned`): a pin is how
 * the strip is arranged and says nothing about the work, so it is the muted text colour and
 * sits out of the row the name, the live dot and the count pill share. Callers draw it last
 * in the tab.
 *
 * It is inside the tab's own button, so it can never be a second thing to click by accident
 * and there is no interactive element inside an interactive element for a screen reader to
 * have to explain. The glyph is decorative; `aria-label` is what carries the meaning, the
 * same split `ChatState` makes.
 */
export function Pin({ held, what }: { held: boolean; what: string }) {
  if (!held) return null;
  return (
    <Tooltip tip={`Pinned. Unpin it from the palette — yours, on this machine only.`}>
      <span className="pinned" role="img" aria-label={`pinned ${what}`}>
        <PinMark />
      </span>
    </Tooltip>
  );
}

/** A tab's close button. The same row the palette lists, drawn as the `×` a pointer wants —
 *  so the accessible name is the catalogue's words and the glyph is only the glyph.
 *
 *  **The words are the whole guard** (charter-app#130). This `×` ends a chat: it calls
 *  `close_session`, which ends the program and takes the chat off the board. The glyph reads
 *  as "hide this tab" and there is no undo, so the name a screen reader and a keyboard get is
 *  `End chat 3 steward`, and the tooltip a pointer gets says what that costs. */
export function Closer({ offer, onPress }: { offer?: Offer; onPress: (offer: Offer) => void }) {
  if (!offer) return null;
  // **A task's own tab has a minimise where a session's has its close** (#1489, V100-38): `−`,
  // named for what it does, "Send talk back into steward 4's tab". It ends nothing.
  const minimise = offer.does.verb === "sendBack";
  return (
    <Tooltip tip={offer.note ? `${offer.title} — ${offer.note}` : offer.title}>
      <button
        // A tab showing only a view closes and ends nothing, so its `×` does not wear the danger
        // hover a chat's does — the look may not say more than the act does, either way.
        className={clsx("closer", closesOnly(offer.does) && "keeps", minimise && "minimise")}
        data-minimise={minimise ? "" : undefined}
        // The tabs family's `×` (`ui/tabs`), which ends a chat unless it only closes a view.
        {...tabPart("closer")}
        data-keeps={closesOnly(offer.does) ? "" : undefined}
        // **Not a Tab stop, and deliberately** (charter-app#189). The strip it sits on is ONE
        // stop, the WAI-ARIA "Tabs" pattern; a `×` per tab in the sequence would be fifty stops
        // again. A keyboard ends a chat from the palette's row for it, or from the tab's own menu
        // (the context-menu key), both of which read the same catalogue row this does — and
        // Delete on the focused tab (charter-app#239, `closeOnDelete`), which a Mac needs.
        tabIndex={-1}
        aria-label={offer.title}
        onClick={() => onPress(offer)}
      >
        {minimise ? <Minus /> : <X />}
      </button>
    </Tooltip>
  );
}

/**
 * **A tab's settings gear** (SE-23, #1173; V89i): the same catalogue row its right-click menu
 * and the palette run — `project.settings:<plane>` on the project in front's tab,
 * `workspace.settings:<name>` on the focused workspace's — drawn as a gear, so its accessible
 * name and its tooltip are the row's words and the glyph is only the glyph, as `Closer`'s `×` is.
 *
 * **Quiet**: drawn on one tab per strip, the one you are on, and invisible there until that tab
 * is under the pointer or the keyboard (`App.css`, `.gear`). No gear is always on, and the title
 * bar has no settings button of its own — the operator's "it must not be noisy".
 *
 * **A Tab stop, unlike `Closer`.** A `×` per tab in the sequence would be fifty stops; this is
 * one, right after the strip's one stop, because only the tab you are on carries one.
 */
export function Gear({ offer, onPress }: { offer?: Offer; onPress: (offer: Offer) => void }) {
  if (!offer) return null;
  return (
    <Tooltip tip={offer.reason || offer.title}>
      <button
        className="gear"
        // WebKit leaves a `<button>` out of the tab sequence unless its `tabindex` is written down
        // (`docs/ui-primitives.md`, charter-app#189).
        tabIndex={0}
        disabled={!offer.available}
        aria-label={offer.title}
        onClick={() => onPress(offer)}
      >
        <SettingsMark />
      </button>
    </Tooltip>
  );
}

/**
 * The glyph a row is drawn as where it is drawn as one: by what it does for the rows about
 * where a task is (#1489), which are one per task, and by its id otherwise ({@link MARKS}).
 * **A minimise is `−`, whatever id it has**: a task's own tab and its own pane keep the close's
 * id, so its key and its menu reach it, and must never draw the close's `×`.
 */
export function markOf(offer: Offer): typeof Plus | undefined {
  switch (offer.does.verb) {
    case "sendBack":
      return Minus;
    case "ownTab":
      return SquareArrowOutUpRight;
    case "beside":
      return PanelRight;
    default:
      break;
  }
  if (offer.id.startsWith("chat.own:")) return SquareArrowOutUpRight;
  if (offer.id.startsWith("chat.beside:")) return PanelRight;
  return MARKS[offer.id];
}
