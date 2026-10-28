// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view tab's top bar** (#3667, spec #2897, the Skill studio frame's top bar): its own back
 * and forward, the breadcrumb of where the person is among its pages, and the view's own
 * controls after them (the studio's scope picker, New skill…). A member of the panel family,
 * drawn over a {@link ViewFrame}'s panes.
 *
 * - **Back and forward** are two icon buttons at the bar's start, pressed by ⌘[ and ⌘] (Alt+←
 *   and Alt+→ off a Mac) through the key registry, and by the mouse's own back and forward
 *   buttons anywhere in the view ({@link ViewFrame}'s `history`). Each is unavailable at its end
 *   of the history and says why, and keeps the keyboard so the reason is read.
 * - **The breadcrumb** ({@link ViewCrumbs}): each name before the last goes to that page, on the
 *   button family's quiet ghost drawn inline, as the pane's crumbs are (#3214); the last is where
 *   the person is, a name with `aria-current="page"` and no button. A screen reader hears a list
 *   named "Path". Long names give way first, cut with an ellipsis, and the whole path is its
 *   tooltip.
 * - **Drawn by its parts**: `data-scope="view-frame"` with `data-part` `bar`, `history`, `path`,
 *   `crumb` (where the person is) and `crumb-separator`.
 */
import type { ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { ariaChord, keySaid, shortcut } from "../../keys/registry";
import { onAMac } from "../../tabKeys";
import { Button, IconButton } from "../button/Button";
import { Tooltip } from "../tooltip/Tooltip";
import type { HistorySteps } from "./viewHistory";

/** **One step of the path**: the page's name, and its id, which a press hands back. */
export interface Crumb {
  id: string;
  label: string;
}

/** What back and forward say when there is nowhere to go, as their tooltips. */
export const NO_BACK = "Nothing to go back to: this is the first page you opened in this tab";
export const NO_FORWARD = "Nothing to go forward to: go back first";

/** A key's chords on this keyboard, as `aria-keyshortcuts` names them. */
function ariaKeys(id: string, mac: boolean): string {
  return shortcut(id)
    .chords[mac ? "mac" : "other"].map(ariaChord)
    .join(" ");
}

/** **Back and forward**: two icon buttons, each unavailable with a reason at its end. */
export function ViewHistoryButtons({ history }: { history: HistorySteps }) {
  const mac = onAMac();
  return (
    <div data-scope="view-frame" data-part="history">
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Back"
        icon={<ChevronLeft aria-hidden="true" />}
        isDisabled={!history.canGoBack}
        tooltip={history.canGoBack ? keySaid("view.back", mac) : NO_BACK}
        aria-keyshortcuts={ariaKeys("view.back", mac)}
        onPress={history.back}
      />
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Forward"
        icon={<ChevronRight aria-hidden="true" />}
        isDisabled={!history.canGoForward}
        tooltip={history.canGoForward ? keySaid("view.forward", mac) : NO_FORWARD}
        aria-keyshortcuts={ariaKeys("view.forward", mac)}
        onPress={history.forward}
      />
    </div>
  );
}

/**
 * **The breadcrumb of a view's pages**: Library › deploy › Grades › New grade. Each name before
 * the last goes to that page (`onGo`, with its place in `path`); the last is where the person is.
 */
export function ViewCrumbs({
  path,
  onGo,
}: {
  path: readonly Crumb[];
  onGo: (crumb: Crumb, at: number) => void;
}) {
  const last = path.length - 1;
  const whole = path.map((one) => one.label).join(" › ");
  return (
    <Tooltip tip={whole}>
      <ol aria-label="Path" data-scope="view-frame" data-part="path">
        {path.map((one, at) => (
          <li key={`${one.id} ${at}`}>
            {at === last ? (
              <span data-scope="view-frame" data-part="crumb" aria-current="page">
                {one.label}
              </span>
            ) : (
              <Button variant="quiet" size="sm" inline onPress={() => onGo(one, at)}>
                {one.label}
              </Button>
            )}
            {at < last && (
              <span aria-hidden="true" data-scope="view-frame" data-part="crumb-separator">
                ›
              </span>
            )}
          </li>
        ))}
      </ol>
    </Tooltip>
  );
}

/** **The bar**: back and forward, the path, then the view's own controls at its end. */
export function ViewBar({
  history,
  path,
  onGo,
  children,
}: {
  history: HistorySteps;
  path: readonly Crumb[];
  onGo: (crumb: Crumb, at: number) => void;
  /** The view's own controls: a scope picker, New skill…. */
  children?: ReactNode;
}) {
  return (
    <div data-scope="view-frame" data-part="bar">
      <ViewHistoryButtons history={history} />
      <ViewCrumbs path={path} onGo={onGo} />
      {children !== undefined && (
        <div data-scope="view-frame" data-part="controls">
          {children}
        </div>
      )}
    </div>
  );
}
