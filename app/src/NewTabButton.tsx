// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import { Plus } from "lucide-react";
import type { Offer } from "./actions";
import {
  CLOSES_AFTER_MS,
  FILTER_FROM,
  inANewTab,
  OPENS_AFTER_MS,
  shown,
  type NewTabRow,
} from "./newTabList";
import { IconButton } from "./ui/button/Button";
import {
  AnchoredMenu,
  Menu,
  MenuItem,
  MenuSeparator,
  MenuWords,
  opensOnTheArrows,
} from "./ui/menu/Menu";

/** How the list was opened: by the pointer's rest, which takes no keyboard, or by the keys. */
type OpenedBy = "rest" | "keys";

/**
 * **The strip's `+`, and the list it shows** (#5292; spec #5288 stories 73 to 81; the prototype's
 * "The + button" scenarios, accepted 2026-11-05).
 *
 * - **A press opens an empty tab** (`tab.new`), so no chat starts by accident.
 * - **A rest shows the list** {@link OPENS_AFTER_MS} after the pointer arrives. It takes no
 *   keyboard, stays open while the pointer moves onto it, and closes {@link CLOSES_AFTER_MS}
 *   after the pointer has left both. A press on `+` is the empty tab and closes it, and the
 *   pointer has to leave before a rest opens it again.
 * - **Down opens it on its first row**, Up on its last; the arrows move, Enter chooses, and Esc
 *   gives the keyboard back to `+`. A long list's filter takes the keyboard instead.
 * - **Its rows are the catalogue's** (`newTabList.ts`): what starts, a rule, then the views,
 *   each with the key that does the same. Past {@link FILTER_FROM} rows it scrolls and has a
 *   filter. Every row means a new tab (`inANewTab`).
 *
 * No tooltip on `+`: the list is what a rest on it shows, and a tip would come up over it.
 */
export function NewTabButton({
  offer,
  rows,
  onPress,
  dragging = () => false,
}: {
  /** The catalogue's `tab.new`: what a press on `+` runs, and its name. */
  offer: Offer | undefined;
  rows: readonly NewTabRow[];
  onPress: (offer: Offer) => void;
  /** Whether a tab or a pane is being carried: a pointer carrying one is not resting. */
  dragging?: () => boolean;
}) {
  const [open, setOpen] = useState<OpenedBy>();
  const [to, setTo] = useState<"first" | "last">("first");
  const [typed, setTyped] = useState("");
  const trigger = useRef<HTMLButtonElement>(null);
  const opening = useRef<number | undefined>(undefined);
  const closing = useRef<number | undefined>(undefined);
  /** A press on `+` while the pointer is over it: no rest opens the list until it has left. */
  const pressedHere = useRef(false);
  /** A row was chosen: the keyboard goes where that row sends it, not back to `+`. */
  const picked = useRef(false);

  const notOpening = useCallback(() => {
    window.clearTimeout(opening.current);
    opening.current = undefined;
  }, []);
  const notClosing = useCallback(() => {
    window.clearTimeout(closing.current);
    closing.current = undefined;
  }, []);
  useEffect(
    () => () => {
      notOpening();
      notClosing();
    },
    [notClosing, notOpening],
  );
  const show = useCallback(
    (by: OpenedBy, at: "first" | "last" = "first") => {
      notOpening();
      notClosing();
      picked.current = false;
      setTyped("");
      setTo(at);
      setOpen(by);
    },
    [notClosing, notOpening],
  );
  const hide = useCallback(() => {
    notOpening();
    notClosing();
    setOpen(undefined);
  }, [notClosing, notOpening]);
  /** The pointer left `+` or the list: a list the rest opened closes soon. */
  const leaveSoon = () => {
    notOpening();
    if (open !== "rest") return;
    notClosing();
    closing.current = window.setTimeout(hide, CLOSES_AFTER_MS);
  };

  if (offer === undefined) return null;
  const filtered = rows.length > FILTER_FROM;
  const drawn = shown(rows, typed);
  const starts = drawn.filter((row) => row.group === "start");
  const views = drawn.filter((row) => row.group === "view");
  const byKeys = open === "keys";
  const row = (one: NewTabRow) => (
    <MenuItem
      key={one.offer.id}
      id={one.offer.id}
      textValue={one.said}
      shortcut={one.offer.id}
      // A row that cannot run stays in the arrows' way and says why.
      aria-disabled={one.offer.available ? undefined : true}
      shouldCloseOnSelect={one.offer.available}
      tooltip={one.offer.available ? undefined : one.offer.reason}
      onAction={() => {
        if (!one.offer.available) return;
        picked.current = true;
        onPress(inANewTab(one.offer));
      }}
    >
      <MenuWords said={one.said} />
    </MenuItem>
  );

  return (
    <>
      <IconButton
        ref={trigger}
        variant="quiet"
        size="sm"
        className="bare"
        icon={<Plus />}
        aria-label={offer.title}
        aria-haspopup="menu"
        aria-expanded={open !== undefined}
        isDisabled={!offer.available}
        onPress={(event) => {
          // Only a pointer's press is over `+`: a key's leaves the next rest free.
          pressedHere.current = event.pointerType === "mouse" || event.pointerType === "pen";
          hide();
          onPress(offer);
        }}
        onPointerEnter={(event) => {
          notClosing();
          // A button held (a tab's drag, a selection) or a tab being carried is no rest.
          if (event.buttons !== 0 || dragging() || pressedHere.current || open !== undefined)
            return;
          notOpening();
          opening.current = window.setTimeout(() => {
            opening.current = undefined;
            if (!dragging()) show("rest");
          }, OPENS_AFTER_MS);
        }}
        onPointerLeave={() => {
          pressedHere.current = false;
          leaveSoon();
        }}
        // Down on a list the rest opened gives it the keyboard.
        onKeyDown={(event) => opensOnTheArrows(byKeys, (at) => show("keys", at))(event)}
      />
      <AnchoredMenu
        triggerRef={trigger}
        isOpen={open !== undefined}
        onOpenChange={(next) => {
          if (!next) hide();
        }}
        // A rest takes no keyboard; the keys put it on the first row, or the last, or in the
        // filter where the list has one.
        autoFocus={byKeys ? (filtered ? false : to) : false}
        onClosed={() => {
          // A row chosen sends the keyboard where it opens. Otherwise it comes back to `+`,
          // where it was, when the list had it.
          if (picked.current || !byKeys) return;
          trigger.current?.focus({ preventScroll: true });
        }}
      >
        <Menu
          aria-label="Open in a new tab"
          className="new-tab-list"
          takesNoKeyboard={!byKeys}
          onPointerEnter={notClosing}
          onPointerLeave={leaveSoon}
          filter={
            filtered
              ? {
                  typed,
                  onTyped: setTyped,
                  takesKeyboard: byKeys,
                  none: "Nothing matches what you typed.",
                }
              : undefined
          }
        >
          {starts.map(row)}
          {starts.length > 0 && views.length > 0 && <MenuSeparator />}
          {views.map((one) => (
            <Fragment key={one.offer.id}>{row(one)}</Fragment>
          ))}
        </Menu>
      </AnchoredMenu>
    </>
  );
}
