// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { IBufferLine, IDisposable, ILink, Terminal } from "@xterm/xterm";
import type { LinkCandidate, LinkFrom, LinkOpened, LinkSeen } from "./bindings";

/**
 * **Links in a pane's output** (spec #6208, #6215): the one hook every xterm pane registers, a
 * chat's, a shell's, a one-job program's and a saved chat's read-only terminal (not the
 * sign-in terminal).
 *
 * - **The pane detects, the core decides.** xterm asks for a line's links when the pointer is on
 *   it ({@link linkPane}, through `registerLinkProvider`), never while output streams. The hook
 *   finds candidate text by pattern ({@link candidatesIn}) and asks the core what each is
 *   (`pane_link_resolve`), sending only the text, its kind hint and where it was found. The
 *   answers are cached per pane, so a line hovered again asks nothing.
 * - **Nothing marks a link at rest** (D1). Under the pointer it is underlined and drawn in the
 *   theme's link colour, and the pane shows its tooltip (`PaneLinkSurfaces.tsx`).
 * - **⌘-click (Ctrl+click off a Mac) opens it**: the core's open command (`pane_link_open`) is
 *   handed the candidate's text again and answers. A plain click selects text, as in any
 *   terminal: xterm's selection is never stopped.
 * - **An address purlis will not open says why on its hover** (`ftp://…`, one carrying a
 *   password): it is not underlined, and ⌘-click does nothing.
 */

/** A pattern's candidate on a line: its kind hint, its text and where in the line it sits. */
export type Found = LinkCandidate & { start: number; end: number };

/**
 * An address as a pane prints it, `scheme://` and up to the first space, quote or angle bracket:
 * http(s), which opens, and any other scheme, which the core says it will not open.
 */
const WEB = /(?<![\p{L}\p{N}+.-])\p{L}[\p{L}\p{N}+.-]*:\/\/[^\s<>"'`\p{Cc}]+/giu;

/** What a sentence, or Markdown's emphasis, puts after an address, which is not the address's. */
const TRAILING = /[.,;:!?*_]+$/u;

/** A closing bracket the address did not open, which the sentence around it did. */
const CLOSES: Record<string, string> = { ")": "(", "]": "[", "}": "{" };

/** `text` with the closing brackets it does not itself open taken off its end. */
function balanced(text: string): string {
  let kept = text;
  for (;;) {
    const last = kept.at(-1);
    const opens = last === undefined ? undefined : CLOSES[last];
    if (opens === undefined) return kept;
    const opened = [...kept].filter((c) => c === opens).length;
    const closed = [...kept].filter((c) => c === last).length;
    if (closed <= opened) return kept;
    kept = kept.slice(0, -1).replace(TRAILING, "");
  }
}

/**
 * **The candidates in `line`**, left to right: addresses (`scheme://…`), with the punctuation a
 * sentence puts after one left out. Later kinds (files, chats) add their patterns here.
 */
export function candidatesIn(line: string): Found[] {
  const found: Found[] = [];
  for (const match of line.matchAll(WEB)) {
    const text = balanced(match[0].replace(TRAILING, ""));
    if (text.length === 0) continue;
    found.push({ kind: "url", text, start: match.index, end: match.index + text.length });
  }
  return found;
}

/**
 * The most rows one wrapped line is read across. A line longer is read in part, and a candidate
 * that runs off the part read is no link: it would open cut short.
 */
const MOST_ROWS = 24;

/** The most answers a pane keeps: the oldest goes first. */
const MOST_KEPT = 256;

/** A pane's kept answers, by {@link linkKey}. */
type Kept = Map<string, Promise<LinkSeen | null>>;

/** **What a link is kept and listed by**: its kind and its text. */
export const linkKey = (link: Pick<LinkCandidate, "kind" | "text">) =>
  `${link.kind}\u0000${link.text}`;

/**
 * **Keeps the core's answer for `key`**, the oldest let go of once a pane keeps
 * {@link MOST_KEPT}. An answer the core could not give is not kept: it is asked again.
 */
function keep(kept: Kept, key: string, asked: Promise<LinkSeen | null>): Promise<LinkSeen | null> {
  const answer: Promise<LinkSeen | null> = asked.catch(() => {
    if (kept.get(key) === answer) kept.delete(key);
    return null;
  });
  if (!kept.has(key) && kept.size >= MOST_KEPT) {
    const oldest = kept.keys().next().value;
    if (oldest !== undefined) kept.delete(oldest);
  }
  kept.set(key, answer);
  return answer;
}

/**
 * A line as xterm holds it across the rows it wraps over: its text, each character's cell, and
 * whether the line went on before or after the part read.
 */
type Wrapped = {
  text: string;
  cells: { x: number; y: number }[];
  cutBefore: boolean;
  cutAfter: boolean;
};

/** Buffer line `y` (zero-based) as one line with the rows it wraps over, and each character's cell. */
function wrappedAt(pane: Terminal, y: number): Wrapped | undefined {
  const buffer = pane.buffer.active;
  let first = y;
  while (first > 0 && y - first < MOST_ROWS && buffer.getLine(first)?.isWrapped) first -= 1;
  let last = y;
  while (last - first < MOST_ROWS && buffer.getLine(last + 1)?.isWrapped) last += 1;
  const cutBefore = buffer.getLine(first)?.isWrapped === true;
  const cutAfter = buffer.getLine(last + 1)?.isWrapped === true;
  let text = "";
  const cells: { x: number; y: number }[] = [];
  for (let row = first; row <= last; row += 1) {
    const line: IBufferLine | undefined = buffer.getLine(row);
    if (line === undefined) return undefined;
    for (let x = 0; x < line.length; x += 1) {
      const cell = line.getCell(x);
      // The second half of a wide character is no character of its own.
      if (cell === undefined || cell.getWidth() === 0) continue;
      const chars = cell.getChars() || " ";
      text += chars;
      for (let unit = 0; unit < chars.length; unit += 1) cells.push({ x, y: row });
    }
  }
  return { text, cells, cutBefore, cutAfter };
}

/** Whether `found` runs off the part of its line read: it would open cut short. */
function cutShort(found: Found, wrapped: Wrapped): boolean {
  return (
    (wrapped.cutBefore && found.start === 0) ||
    (wrapped.cutAfter && found.end === wrapped.text.length)
  );
}

/** Where a hovered link is drawn, in the window's viewport: what its tooltip stands beside. */
export type LinkPlace = { left: number; top: number; width: number; height: number };

/** **A link under the pointer**: what the core said it is, its words, and where it is drawn. */
export type HoveredLink = { seen: LinkSeen; text: string; place: LinkPlace };

/**
 * **The core's two link commands**, handed to the hook by its pane (`paneLinkCore.ts` holds the
 * window's), so a screen that must reach no core, the theme specimen's, draws no links.
 */
export type LinkCore = {
  /** What each candidate on a line is: `pane_link_resolve`. */
  resolve: (from: LinkFrom, candidates: LinkCandidate[]) => Promise<(LinkSeen | null)[]>;
  /** Opens a candidate, resolved again: `pane_link_open`. */
  open: (from: LinkFrom, candidate: LinkCandidate, confirmed: string | null) => Promise<LinkOpened>;
};

/** What a pane does with a link's hover, and with the open command's answer. */
export type LinkHands = {
  /** The core that decides what its links are and opens them. */
  core: LinkCore;
  /** Where its candidates were found: the pane's project and chat, where it has them. */
  from: () => LinkFrom;
  /** The pointer is on a link, or has left it (`null`). */
  hovered: (link: HoveredLink | null) => void;
  /** What the open command answered for the link ⌘-clicked. */
  answered: (answer: LinkOpened, candidate: LinkCandidate) => void;
  /** The colour a link is drawn in under the pointer, as `#rrggbb`. */
  colour: () => string;
  /** Whether this is a Mac, where ⌘ opens a link, and not Ctrl. */
  mac: () => boolean;
};

/** Whether `event` is the click that opens a link: ⌘ held on a Mac, Ctrl elsewhere. */
export function opensALink(event: Pick<MouseEvent, "metaKey" | "ctrlKey">, mac: boolean): boolean {
  return mac ? event.metaKey : event.ctrlKey;
}

/** What the window says when the core could not be asked to open a link. */
export const UNASKED = "purlis could not open that link";

/**
 * **Hands that pass on to whichever pane shows the terminal now** (`to`), as a chat's terminal,
 * kept while no pane shows it, needs: with none, nothing is asked and nothing is drawn.
 */
export function relayed(to: () => LinkHands | undefined): LinkHands {
  return {
    core: {
      resolve: (from, candidates) =>
        to()?.core.resolve(from, candidates) ?? Promise.resolve(candidates.map(() => null)),
      open: (from, candidate, confirmed) =>
        to()?.core.open(from, candidate, confirmed) ??
        Promise.reject(new Error("no pane shows this terminal")),
    },
    from: () => to()?.from() ?? { plane: null, session: null },
    hovered: (link) => to()?.hovered(link),
    answered: (answer, candidate) => to()?.answered(answer, candidate),
    // Drawn only over a link a pane resolved, so a pane shows it and says its colour.
    colour: () => to()?.colour() ?? "",
    mac: () => to()?.mac() ?? false,
  };
}

/** **Opens `candidate`**, resolved again by the core, and hands on its answer. */
export function openLink(
  candidate: LinkCandidate,
  hands: Pick<LinkHands, "core" | "from" | "answered">,
  confirmed: string | null = null,
): Promise<void> {
  return hands.core
    .open(hands.from(), { kind: candidate.kind, text: candidate.text }, confirmed)
    .then(
      (answer) => hands.answered(answer, candidate),
      (why: unknown) => {
        // The detail is the log's: the window says it in a sentence.
        console.warn("purlis could not ask the core to open a link", why);
        hands.answered({ answer: "refused", reason: UNASKED }, candidate);
      },
    );
}

/** Where cells `x`..`x + width` of buffer row `y` are drawn in the window's viewport. */
function placeOf(pane: Terminal, x: number, y: number, width: number): LinkPlace {
  const screen = pane.element?.querySelector<HTMLElement>(".xterm-screen")?.getBoundingClientRect();
  const cellWidth = screen && pane.cols > 0 ? screen.width / pane.cols : 0;
  const cellHeight = screen && pane.rows > 0 ? screen.height / pane.rows : 0;
  const row = y - pane.buffer.active.viewportY;
  return {
    left: (screen?.left ?? 0) + x * cellWidth,
    top: (screen?.top ?? 0) + row * cellHeight,
    width: width * cellWidth,
    height: cellHeight,
  };
}

/** **A link on screen**, as *Links in this pane…* lists it (#6220) and the terminal's menu
 *  offers the one under the pointer (#6219): its candidate and the core's answer for it. */
export type ScreenLink = { candidate: LinkCandidate; seen: LinkSeen };

/** **A pane with its link hook registered**: let go of by `dispose`. */
export type LinkedPane = IDisposable & {
  /**
   * **The links on screen, newest first** (#6220, D5): the viewport's, not the history's, the
   * lowest first and, on one line, the rightmost. A link on screen twice is listed once, where
   * it is lowest. What the core answers is kept with the hover's, and the candidates it has not
   * answered yet are asked in one call. Text the core says nothing of (`null`) is left out.
   */
  onScreen: () => Promise<ScreenLink[]>;
  /** **The link the pointer is on now**, which the terminal's menu offers to open and copy
   *  (#6219): xterm keeps a link hovered only while the pointer is in its range. */
  under: () => ScreenLink | undefined;
};

/**
 * **Registers the link hook on `pane`**: its links are found, resolved by the core and opened
 * as {@link LinkHands} says. Disposing it lets go of the provider and of the colour drawn.
 */
export function linkPane(pane: Terminal, hands: LinkHands): LinkedPane {
  /** The core's answers, per pane, by kind and text: a line hovered again asks nothing. */
  const kept: Kept = new Map();
  const seenOf = (found: Found): Promise<LinkSeen | null> => {
    const key = linkKey(found);
    const had = kept.get(key);
    if (had !== undefined) return had;
    return keep(
      kept,
      key,
      hands.core
        .resolve(hands.from(), [{ kind: found.kind, text: found.text }])
        .then(([seen]) => seen ?? null),
    );
  };

  /** The colour drawn over the link under the pointer, let go of as it leaves. */
  let drawn: IDisposable[] = [];
  const undraw = () => {
    for (const one of drawn) one.dispose();
    drawn = [];
  };
  /** The link the pointer is on, by where it is and what it says. */
  let on: string | undefined;
  /** The link the pointer is on, as the terminal's menu reads it. */
  let current: ScreenLink | undefined;
  /**
   * **A leave is let happen a moment later.** Drawing the colour redraws the link's row, and
   * xterm lets go of a link on a row it redraws and finds it again at once: a leave and a hover
   * of the same link, which must not undraw and draw it again (that would loop) nor close and
   * open its tooltip. A hover of the same link before then keeps it as it is.
   */
  let leaving: ReturnType<typeof setTimeout> | undefined;
  const leave = () => {
    clearTimeout(leaving);
    leaving = undefined;
    on = undefined;
    current = undefined;
    undraw();
    hands.hovered(null);
  };
  /** Draws cells of `rows` in the link colour, where the terminal draws decorations. */
  const draw = (rows: { x: number; y: number; width: number }[]) => {
    undraw();
    const buffer = pane.buffer.active;
    const cursor = buffer.baseY + buffer.cursorY;
    try {
      for (const { x, y, width } of rows) {
        const marker = pane.registerMarker(y - cursor);
        if (marker === undefined) continue;
        drawn.push(marker);
        const decoration = pane.registerDecoration({
          marker,
          x,
          width,
          foregroundColor: hands.colour(),
        });
        if (decoration !== undefined) drawn.push(decoration);
      }
    } catch {
      // A terminal that draws no decorations still underlines the link.
    }
  };

  const provider = pane.registerLinkProvider({
    provideLinks(line, give) {
      const y = line - 1;
      const wrapped = wrappedAt(pane, y);
      if (wrapped === undefined) return give(undefined);
      const onRow = candidatesIn(wrapped.text).filter(
        (found) =>
          !cutShort(found, wrapped) &&
          wrapped.cells.slice(found.start, found.end).some((cell) => cell.y === y),
      );
      if (onRow.length === 0) return give(undefined);
      void Promise.all(onRow.map(seenOf)).then((answers) => {
        const links: ILink[] = [];
        onRow.forEach((found, at) => {
          const seen = answers[at];
          if (seen === null) return;
          const cells = wrapped.cells.slice(found.start, found.end);
          const first = cells[0];
          const end = cells[cells.length - 1];
          // Each row the link is drawn over, and the cells of it.
          const rows = new Map<number, { x: number; y: number; width: number }>();
          for (const cell of cells) {
            const row = rows.get(cell.y);
            if (row === undefined) rows.set(cell.y, { x: cell.x, y: cell.y, width: 1 });
            else row.width = cell.x - row.x + 1;
          }
          const candidate: LinkCandidate = { kind: found.kind, text: found.text };
          // What purlis will not open says why on its hover, and is drawn as no link.
          const opens = seen.does.does !== "refused";
          links.push({
            range: {
              start: { x: first.x + 1, y: first.y + 1 },
              end: { x: end.x + 1, y: end.y + 1 },
            },
            text: found.text,
            decorations: { pointerCursor: opens, underline: opens },
            activate(event) {
              // A plain click is the selection's, as in any terminal.
              if (!opens || !opensALink(event, hands.mac())) return;
              event.preventDefault();
              void openLink(candidate, hands);
            },
            hover() {
              const here = `${first.x},${first.y}\u0000${found.text}`;
              if (leaving !== undefined) {
                clearTimeout(leaving);
                leaving = undefined;
                if (on === here) return;
                leave();
              }
              on = here;
              current = { candidate, seen };
              const lines = [...rows.values()];
              if (opens) draw(lines);
              const top = lines[0];
              hands.hovered({
                seen,
                text: found.text,
                place: placeOf(pane, top.x, top.y, top.width),
              });
            },
            leave() {
              clearTimeout(leaving);
              leaving = setTimeout(leave, 0);
            },
          });
        });
        give(links.length > 0 ? links : undefined);
      });
    },
  });
  return {
    under: () => current,
    dispose() {
      clearTimeout(leaving);
      undraw();
      provider.dispose();
      kept.clear();
    },
    onScreen: () => screenLinks(pane, hands, kept),
  };
}

/** The candidates on `pane`'s screen, newest first, each once: {@link LinkedPane.onScreen}. */
function onScreenNewestFirst(pane: Terminal): Found[] {
  const buffer = pane.buffer.active;
  const top = buffer.viewportY;
  const bottom = top + pane.rows - 1;
  const found: Found[] = [];
  for (let y = top; y <= bottom;) {
    const wrapped = wrappedAt(pane, y);
    if (wrapped === undefined) break;
    const rows = wrapped.cells.map((cell) => cell.y);
    const last = rows.length === 0 ? y : Math.max(y, ...rows);
    for (const one of candidatesIn(wrapped.text)) {
      if (cutShort(one, wrapped)) continue;
      const shown = wrapped.cells
        .slice(one.start, one.end)
        .some((cell) => cell.y >= top && cell.y <= bottom);
      if (shown) found.push(one);
    }
    y = last + 1;
  }
  const once = new Set<string>();
  return found.reverse().filter((one) => {
    const key = linkKey(one);
    if (once.has(key)) return false;
    once.add(key);
    return true;
  });
}

/** The links on `pane`'s screen, answered from `kept` or asked in one call and kept there. */
async function screenLinks(
  pane: Terminal,
  hands: Pick<LinkHands, "core" | "from">,
  kept: Kept,
): Promise<ScreenLink[]> {
  const found = onScreenNewestFirst(pane);
  /** Each candidate's answer, held here too: one the pane lets go of still counts now. */
  const answerOf = new Map<string, Promise<LinkSeen | null>>();
  const unasked: Found[] = [];
  for (const one of found) {
    const had = kept.get(linkKey(one));
    if (had === undefined) unasked.push(one);
    else answerOf.set(linkKey(one), had);
  }
  if (unasked.length > 0) {
    const asked = hands.core.resolve(
      hands.from(),
      unasked.map((one) => ({ kind: one.kind, text: one.text })),
    );
    unasked.forEach((one, at) => {
      const key = linkKey(one);
      answerOf.set(
        key,
        keep(
          kept,
          key,
          asked.then((answers) => answers[at] ?? null),
        ),
      );
    });
  }
  const answers = await Promise.all(found.map((one) => answerOf.get(linkKey(one)) ?? null));
  return found.flatMap((one, at) => {
    const seen = answers[at];
    return seen === null ? [] : [{ candidate: { kind: one.kind, text: one.text }, seen }];
  });
}
