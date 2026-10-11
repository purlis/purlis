import { inSlots, shownIn, type Arrangement } from "./regions";

/**
 * **F6: the keyboard from region to region** (spec #1736, P-13, #1741), as an editor has it.
 *
 * F6 goes to the next region in a fixed order — the left sidebar, the editor, the right sidebar,
 * the Panel's left half, its right half — and Shift+F6 to the one before, round from the end to
 * the start. A region that is away, or a half a narrow window does not draw, is skipped; the
 * editor is always there. The order is the reading order too: the sidebars and the editor are
 * one row, and the Panel is the row under them.
 *
 * **F6 inside a chat's terminal is the chat's** (`docs/ui-primitives.md`, "Which keys belong to
 * the chat"): a terminal sends it as a function key, and htop or Midnight Commander act on it.
 * Ctrl+Tab is the way out of a terminal, as it always was, and F6 into the editor lands on the
 * focused pane's own controls, not in its terminal, so the cycle never ends up somewhere F6 is
 * not the window's.
 */

/** A region F6 stops in. */
export type Stop = "left" | "centre" | "right" | "panel-left" | "panel-right";

/** Every region F6 can stop in, in its order. */
export const STOPS: readonly Stop[] = ["left", "centre", "right", "panel-left", "panel-right"];

/** The regions F6 stops in for this arrangement (the one drawn), in order, skipping what is away. */
export function cycleOf(arrangement: Arrangement): Stop[] {
  const slots = inSlots(arrangement);
  const out = (placed: typeof slots.left) => shownIn(placed).length > 0;
  return STOPS.filter((stop) => {
    if (stop === "centre") return true;
    if (stop === "left" || stop === "right") return out(slots[stop]);
    return out(slots.panel.filter((one) => one.id === stop));
  });
}

/**
 * Where a press goes from `at`: the next region of `cycle`, or with `back` the one before,
 * round at the ends. From outside every region, the first, or with `back` the last; from a
 * region that is no longer in the cycle, on from where it stands in {@link STOPS}.
 */
export function stepped(cycle: readonly Stop[], at: Stop | undefined, back: boolean): Stop {
  if (at === undefined) return back ? cycle[cycle.length - 1] : cycle[0];
  const rank = STOPS.indexOf(at);
  if (back) {
    const before = [...cycle].reverse().find((stop) => STOPS.indexOf(stop) < rank);
    return before ?? cycle[cycle.length - 1];
  }
  return cycle.find((stop) => STOPS.indexOf(stop) > rank) ?? cycle[0];
}

/** F6, or Shift+F6 going back; nothing with another modifier held. */
export function f6Of(e: KeyboardEvent): { back: boolean } | undefined {
  if (e.key !== "F6" || e.ctrlKey || e.metaKey || e.altKey) return undefined;
  return { back: e.shiftKey };
}
