// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type {
  IBufferLine,
  IBufferRange,
  IDisposable,
  ILink,
  ILinkHandler,
  IMarker,
  Terminal,
} from "@xterm/xterm";
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
 * - **A link a program marked itself (OSC 8, #6218) reaches the same core**, through xterm's
 *   `linkHandler`: its words and the target the program named, as the pane was handed them.
 *   The core reads the target again and asks first where the words read as another address.
 *   xterm draws it as a link under the pointer whatever it is, so ⌘-click on one the core
 *   refuses says why. Where a program marks links is the program's: purlis changes no
 *   harness's environment to coax them out of it.
 */

/** A pattern's candidate on a line: its kind hint, its text and where in the line it sits. */
export type Found = LinkCandidate & { start: number; end: number };

/**
 * An address as a pane prints it, `scheme://` and up to the first space, quote or angle bracket:
 * http(s), which opens, and any other scheme, which the core says it will not open.
 */
const WEB = /(?<![\p{L}\p{N}+.-])\p{L}[\p{L}\p{N}+.-]*:\/\/[^\s<>"'`\p{Cc}]+/giu;

/**
 * **A chat's number** (#6217): the word `chat` and a number, as a word of its own. A bare `#641`
 * is an issue's number and never a chat, and a title never links: the core looks the number up
 * in the pane's project, and one no chat there had is no link. At most nine digits and no leading
 * zero, as the core reads one (`panelinks/chat.rs`, `MOST_DIGITS`).
 */
const CHAT = /(?<![\p{L}\p{N}_#/.-])[Cc]hat [1-9]\d{0,8}(?![\p{L}\p{N}_])/gu;

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
 * **A path as a harness prints it** (#6216): absolute, `~/`, `./`, `../` or relative, with an
 * optional `:line[:col]`. Not after a letter, a digit or a path's own characters, so the middle
 * of a word or an address is no start.
 */
const PATH =
  /(?<![\p{L}\p{N}_./~@+-])(?:~\/|\.{1,2}\/|\/)?(?:[\p{L}\p{N}_.@+-]+\/)*[\p{L}\p{N}_.@+-]+(?::\d+(?::\d+)?)?/gu;

/** A file name and its extension, what makes a word with no `/` in it read as a file's name:
 *  two characters of name at least, so `e.g` and `i.e` are none. */
const EXTENSION = /[\p{L}\p{N}_-]{2}\.[\p{L}][\p{L}\p{N}]{0,11}$/u;

/** Full stops after a path, which end its sentence. */
const TRAILING_DOTS = /\.+$/u;

/** The most paths one line is asked about: each is a look at the disk. */
const MOST_PATHS = 32;

/** Whether `text`, a path's words, reads as a file's path: it has a `/`, or a name and extension. */
function readsAsAPath(text: string): boolean {
  const words = text.replace(/(?::\d+){1,2}$/u, "");
  return words.includes("/") ? /[\p{L}\p{N}]/u.test(words) : EXTENSION.test(words);
}

/** A `file:` address, in any case: `FILE:` is the same scheme. */
const FILE_ADDRESS = /^file:/iu;

/**
 * **The candidates in `line`**, left to right: addresses (`scheme://…`, a `file://` one a file),
 * then file paths outside them, with the punctuation a sentence puts after one left out, then
 * chats' numbers (`chat 641`, #6217) outside both.
 */
export function candidatesIn(line: string): Found[] {
  const found: Found[] = [];
  for (const match of line.matchAll(WEB)) {
    const text = balanced(match[0].replace(TRAILING, ""));
    if (text.length === 0) continue;
    // A `file:` address names a file, under a file's rules (#6216, spec Q2).
    const kind = FILE_ADDRESS.test(text) ? "file" : "url";
    found.push({ kind, text, start: match.index, end: match.index + text.length });
  }
  const addresses = [...found];
  let paths = 0;
  for (const match of line.matchAll(PATH)) {
    if (paths >= MOST_PATHS) break;
    const start = match.index;
    // A sentence's full stop after a path is not the path's.
    const text = match[0].replace(TRAILING_DOTS, "");
    const end = start + text.length;
    if (text.length === 0 || !readsAsAPath(text)) continue;
    if (addresses.some((one) => start < one.end && end > one.start)) continue;
    found.push({ kind: "file", text, start, end });
    paths += 1;
  }
  const before = [...found];
  for (const match of line.matchAll(CHAT)) {
    const start = match.index;
    const end = start + match[0].length;
    // Words inside an address or a path are theirs.
    if (before.some((one) => start < one.end && end > one.start)) continue;
    found.push({ kind: "chat", text: match[0], start, end });
  }
  return found.sort((a, b) => a.start - b.start);
}

/**
 * The most rows one wrapped line is read across. A line longer is read in part, and a candidate
 * that runs off the part read is no link: it would open cut short.
 */
const MOST_ROWS = 24;

/** The most answers a pane keeps: the oldest goes first. */
const MOST_KEPT = 256;

/**
 * **How long a file's answer is kept** (#6216): a file is made and deleted while a chat works,
 * so a path hovered or listed again after this is asked again, and a deleted file stops linking.
 */
export const FILE_ANSWER_LIFE = 5_000;

/** **Cells of one row** a link is drawn over: from column `x`, `width` of them, on buffer row
 *  `y` (zero-based). */
type CellRun = { x: number; y: number; width: number };

/** A pane's kept answers, by {@link linkKey}. */
type Kept = Map<string, Promise<LinkSeen | null>>;

/** When each kept file answer stops counting: {@link FILE_ANSWER_LIFE} after it was asked. */
const lives = new WeakMap<Promise<LinkSeen | null>, number>();

/** Whether `link`'s answer looks at the disk, and so is kept only {@link FILE_ANSWER_LIFE}: a
 *  path, or a link a program marked whose target is a `file:` address. */
const namesAFile = (link: LinkCandidate) =>
  link.kind === "file" || (link.kind === "osc8" && FILE_ADDRESS.test(link.target ?? ""));

/** **The answer kept for `key`**, while it still counts: a file's for {@link FILE_ANSWER_LIFE}. */
function held(kept: Kept, key: string): Promise<LinkSeen | null> | undefined {
  const had = kept.get(key);
  if (had === undefined) return undefined;
  if ((lives.get(had) ?? Infinity) > Date.now()) return had;
  kept.delete(key);
  return undefined;
}

/** **What a link is kept and listed by**: its kind, its text and, for one a program marked, the
 *  target it named. */
export const linkKey = (link: Pick<LinkCandidate, "kind" | "text" | "target">) =>
  `${link.kind}\u0000${link.text}\u0000${link.target ?? ""}`;

/** **`link` as the core is handed it**: its kind and text, and a marked link's target. */
export const candidateOf = (link: LinkCandidate): LinkCandidate =>
  link.target === undefined || link.target === null
    ? { kind: link.kind, text: link.text }
    : { kind: link.kind, text: link.text, target: link.target };

/**
 * **Keeps the core's answer for `link`**, the oldest let go of once a pane keeps
 * {@link MOST_KEPT}. An answer the core could not give is not kept: it is asked again.
 */
function keep(
  kept: Kept,
  link: LinkCandidate,
  asked: Promise<LinkSeen | null>,
): Promise<LinkSeen | null> {
  const key = linkKey(link);
  const answer: Promise<LinkSeen | null> = asked.catch(() => {
    if (kept.get(key) === answer) kept.delete(key);
    return null;
  });
  if (namesAFile(link)) lives.set(answer, Date.now() + FILE_ANSWER_LIFE);
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

/** **A link under the pointer**: what the core said it is, its words, where it is drawn, and
 *  whether a program marked it (OSC 8), whose tooltip says where it goes. */
export type HoveredLink = { seen: LinkSeen; text: string; place: LinkPlace; marked?: boolean };

/**
 * **The core's two link commands**, handed to the hook by its pane (`paneLinkCore.ts` holds the
 * window's), so a screen that must reach no core, the theme specimen's, draws no links.
 */
export type LinkCore = {
  /** What each candidate on a line is: `pane_link_resolve`. */
  resolve: (from: LinkFrom, candidates: LinkCandidate[]) => Promise<(LinkSeen | null)[]>;
  /** Opens a candidate, resolved again: `pane_link_open`. `token` answers an ask: the one-use
   *  token the core issued with it (#6216). */
  open: (from: LinkFrom, candidate: LinkCandidate, token: string | null) => Promise<LinkOpened>;
};

/** What a pane does with a link's hover, and with the open command's answer. */
export type LinkHands = {
  /** The core that decides what its links are and opens them. */
  core: LinkCore;
  /** Where its candidates were found: the pane's project and chat, where it has them. */
  from: () => LinkFrom;
  /** The pointer is on a link, or has left it (`null`). */
  hovered: (link: HoveredLink | null) => void;
  /** What the open command answered for the link ⌘-clicked, drawn at `place` where it is on
   *  screen: a chat's card stands beside it. */
  answered: (answer: LinkOpened, candidate: LinkCandidate, place?: LinkPlace) => void;
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
      open: (from, candidate, token) =>
        to()?.core.open(from, candidate, token) ??
        Promise.reject(new Error("no pane shows this terminal")),
    },
    from: () => to()?.from() ?? { plane: null, session: null },
    hovered: (link) => to()?.hovered(link),
    answered: (answer, candidate, place) => to()?.answered(answer, candidate, place),
    // Drawn only over a link a pane resolved, so a pane shows it and says its colour.
    colour: () => to()?.colour() ?? "",
    mac: () => to()?.mac() ?? false,
  };
}

/** **Opens `candidate`**, drawn at `place`, resolved again by the core, and hands on its answer.
 *  `token` is the token of the ask the person accepted. */
export function openLink(
  candidate: LinkCandidate,
  hands: Pick<LinkHands, "core" | "from" | "answered">,
  token: string | null = null,
  place?: LinkPlace,
): Promise<void> {
  return hands.core.open(hands.from(), candidateOf(candidate), token).then(
    (answer) => hands.answered(answer, candidate, place),
    (why: unknown) => {
      // The detail is the log's: the window says it in a sentence.
      console.warn("purlis could not ask the core to open a link", why);
      hands.answered({ answer: "refused", reason: UNASKED }, candidate, place);
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
 *  offers the one under the pointer (#6219): its candidate and the core's answer for it, and
 *  where it was drawn as it was read, which a chat's card opened from either stands beside
 *  (#6442): both close on the press, so the row it was picked on is gone. */
export type ScreenLink = { candidate: LinkCandidate; seen: LinkSeen; place?: LinkPlace };

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
 * **A link a program marked on the screen** (OSC 8): the target it named, and the cells from
 * where it opened the link to where it closed it, held by markers so they move with the
 * buffer. Recorded as the program writes it ({@link marking}), so the whole of its words are
 * read, across the rows they wrap over.
 */
type Marked = {
  target: string;
  /** Which buffer it was written to: the normal one, or the alternate screen. */
  buffer: "normal" | "alternate";
  start: IMarker;
  startX: number;
  end: IMarker;
  endX: number;
  /**
   * Whether the program moved the cursor or changed the screen while the link was open (a
   * CSI or ESC sequence): what the link's cells show may then lie outside the span from its
   * start to its end, so its whole rows are read.
   */
  moved: boolean;
};

/** The longest target a marked link is handed on with: one longer is cut one past it, and the
 *  core refuses it unread (`osc8::MOST_TARGET`). */
export const MOST_TARGET = 2048;

/** The final characters of the CSI sequences that move the cursor or change what the screen
 *  shows: cursor moves, erases, inserts and deletes, scrolls and the saved cursor. */
const MOVES_CSI = "ABCDEFGHIJKLMPSTXZ@`abdefjkrsu";

/** The ESC sequences that do the same: the saved cursor, index, reverse index, next line and a
 *  reset. */
const MOVES_ESC = "78DEMc";

/** A marked link's target as it is kept and handed on: cut one past {@link MOST_TARGET}, so the
 *  core sees it is too long. */
const cutTarget = (target: string) => target.slice(0, MOST_TARGET + 1);

/** The most marked links a pane keeps: the oldest goes first. */
const MOST_MARKED = 256;

/** The cell `[x, y]` comes before cell `[x2, y2]`, read as text is. */
const precedes = (y: number, x: number, y2: number, x2: number) => y < y2 || (y === y2 && x < x2);

/** Whether buffer cell (`x`, `y`) is one of `marked`'s. */
const covers = (marked: Marked, x: number, y: number) =>
  !precedes(y, x, marked.start.line, marked.startX) && precedes(y, x, marked.end.line, marked.endX);

/**
 * **Records each link a program marks on `pane`** (OSC 8): `ESC ] 8 ; params ; target ST`
 * opens one at the cursor and an empty target closes it there. xterm's own handler still runs:
 * it draws the link and hands it to the pane's `linkHandler`.
 */
function marking(pane: Terminal, marked: Marked[]): IDisposable {
  let open: Omit<Marked, "end" | "endX"> | undefined;
  /** A sequence that moves the cursor or changes the screen marks the open link moved. */
  const moves = () => {
    if (open !== undefined) open.moved = true;
    // The terminal's own handler runs it.
    return false;
  };
  const watched: IDisposable[] = [
    ...[...MOVES_CSI].map((final) => pane.parser.registerCsiHandler({ final }, moves)),
    ...[...MOVES_ESC].map((final) => pane.parser.registerEscHandler({ final }, moves)),
  ];
  const at = () => {
    const buffer = pane.buffer.active;
    // A cursor waiting to wrap stands past the last column: the link ends at the row's end.
    return Math.min(buffer.cursorX, pane.cols);
  };
  const handler = pane.parser.registerOscHandler(8, (data) => {
    const split = data.indexOf(";");
    const target = split < 0 ? "" : data.slice(split + 1);
    const buffer = pane.buffer.active.type;
    if (open !== undefined) {
      const end = pane.registerMarker(0);
      const was = open;
      open = undefined;
      if (end !== undefined && was.buffer === buffer) {
        const one = { ...was, end, endX: at() };
        if (one.moved || precedes(one.start.line, one.startX, end.line, one.endX)) {
          marked.push(one);
          while (marked.length > MOST_MARKED) {
            const oldest = marked.shift();
            oldest?.start.dispose();
            oldest?.end.dispose();
          }
        } else {
          was.start.dispose();
          end.dispose();
        }
      } else {
        was.start.dispose();
        end?.dispose();
      }
    }
    if (target !== "") {
      const start = pane.registerMarker(0);
      if (start !== undefined)
        open = {
          target: cutTarget(target),
          buffer,
          start,
          startX: at(),
          moved: false,
        };
    }
    // xterm's own handler draws the link.
    return false;
  });
  return {
    dispose() {
      handler.dispose();
      for (const one of watched) one.dispose();
      open?.start.dispose();
      for (const one of marked.splice(0)) {
        one.start.dispose();
        one.end.dispose();
      }
    },
  };
}

/**
 * **`marked`'s words**, read from the buffer across the rows they wrap over, at most
 * {@link MOST_ROWS} of them: from its start to its end, or its whole rows where the program
 * moved the cursor while it was open.
 */
function wordsOf(pane: Terminal, marked: Marked): string {
  const buffer = pane.buffer.active;
  const first = Math.min(marked.start.line, marked.end.line);
  const last = Math.min(Math.max(marked.start.line, marked.end.line), first + MOST_ROWS - 1);
  let text = "";
  for (let y = first; y <= last; y += 1) {
    const line = buffer.getLine(y);
    if (line === undefined) break;
    const from = !marked.moved && y === marked.start.line ? marked.startX : 0;
    const to = !marked.moved && y === marked.end.line ? marked.endX : line.length;
    let row = "";
    let blank = "";
    for (let x = from; x < to; x += 1) {
      const cell = line.getCell(x);
      if (cell === undefined || cell.getWidth() === 0) continue;
      const chars = cell.getChars();
      if (chars === "") {
        blank += " ";
        continue;
      }
      row += blank + chars;
      blank = "";
    }
    // A row's cells no program wrote, to its end, are no space inside one link: its words go on
    // on the next row, where a hard line break put them.
    text += marked.moved || y === last ? row + blank : row;
  }
  if (marked.moved) return text.trim();
  return withBeside(
    pane,
    text,
    { y: marked.start.line, x: marked.startX },
    { y: marked.end.line, x: marked.endX },
  );
}

/** Where a cell is on the buffer, zero-based. */
type Cell = { y: number; x: number };

/**
 * **A link's words `text` with the characters drawn against them** ({@link besideOf}), the link
 * running from cell `start` to just before `end`: text drawn against the link reads as one word
 * with it; across a blank, it does not.
 */
function withBeside(pane: Terminal, text: string, start: Cell, end: Cell): string {
  const { before, after } = besideOf(pane, start, end);
  return ((/^\s/u.test(text) ? "" : before) + text + (/\s$/u.test(text) ? "" : after)).trim();
}

/**
 * **The characters drawn next to a link, with no space between**: from the cell before `start`
 * back, and from `end` (just past the link) on, across the rows they wrap over, to a blank or
 * {@link MOST_TARGET} characters. A link's words are read whole with them: a program may print
 * text against its link's cells, and the reader reads them as one word.
 */
function besideOf(pane: Terminal, start: Cell, end: Cell): { before: string; after: string } {
  const buffer = pane.buffer.active;
  const blank = (chars: string): boolean => chars === "" || /\s/u.test(chars);
  let before = "";
  for (let { y, x } = { y: start.y, x: start.x - 1 }; before.length < MOST_TARGET;) {
    if (x < 0) {
      if (buffer.getLine(y)?.isWrapped !== true) break;
      y -= 1;
      x = pane.cols - 1;
      continue;
    }
    const cell = buffer.getLine(y)?.getCell(x);
    if (cell === undefined) break;
    x -= 1;
    if (cell.getWidth() === 0) continue;
    const chars = cell.getChars();
    if (blank(chars)) break;
    before = chars + before;
  }
  let after = "";
  for (let { y, x } = end; after.length < MOST_TARGET;) {
    if (x >= pane.cols) {
      if (buffer.getLine(y + 1)?.isWrapped !== true) break;
      y += 1;
      x = 0;
      continue;
    }
    const cell = buffer.getLine(y)?.getCell(x);
    if (cell === undefined) break;
    x += 1;
    if (cell.getWidth() === 0) continue;
    const chars = cell.getChars();
    if (blank(chars)) break;
    after += chars;
  }
  return { before, after };
}

/** The marked links still on `pane`'s buffer now, of the buffer it shows. */
function markedNow(pane: Terminal, marked: Marked[]): Marked[] {
  const buffer = pane.buffer.active.type;
  return marked.filter(
    (one) =>
      one.buffer === buffer &&
      !one.start.isDisposed &&
      !one.end.isDisposed &&
      one.start.line >= 0 &&
      one.end.line >= 0,
  );
}

/** The cells of xterm's `range` (one-based, its end taken in) on the rows they are on. */
function rowsOf(range: IBufferRange, cols: number): CellRun[] {
  const rows: CellRun[] = [];
  for (let y = range.start.y; y <= range.end.y; y += 1) {
    const from = y === range.start.y ? range.start.x - 1 : 0;
    const to = y === range.end.y ? range.end.x : cols;
    if (to > from) rows.push({ x: from, y: y - 1, width: to - from });
  }
  return rows;
}

/**
 * **Registers the link hook on `pane`**: its links are found, resolved by the core and opened
 * as {@link LinkHands} says. Disposing it lets go of the provider and of the colour drawn.
 */
export function linkPane(pane: Terminal, hands: LinkHands): LinkedPane {
  /** The core's answers, per pane, by kind and text: a line hovered again asks nothing, until a
   *  file's answer is past {@link FILE_ANSWER_LIFE}. */
  const kept: Kept = new Map();
  const seenOf = (candidate: LinkCandidate): Promise<LinkSeen | null> => {
    const key = linkKey(candidate);
    const had = held(kept, key);
    if (had !== undefined) return had;
    return keep(
      kept,
      candidate,
      hands.core.resolve(hands.from(), [candidateOf(candidate)]).then(([seen]) => seen ?? null),
    );
  };

  /** The links the program marked on the screen (OSC 8), as it wrote them. */
  const marked: Marked[] = [];
  const markings = marking(pane, marked);

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
  const leaveSoon = () => {
    clearTimeout(leaving);
    leaving = setTimeout(leave, 0);
  };
  /** Draws cells of `rows` in the link colour, where the terminal draws decorations. */
  const draw = (rows: CellRun[]) => {
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
  /** **The pointer is on a link** drawn over `rows`, known `here` by where it is and what it
   *  says: its tooltip, and its colour where it opens. */
  const hoverOn = (
    here: string,
    rows: CellRun[],
    candidate: LinkCandidate,
    link: Omit<HoveredLink, "place">,
  ) => {
    if (leaving !== undefined) {
      clearTimeout(leaving);
      leaving = undefined;
      if (on === here) return;
      leave();
    }
    on = here;
    const top = rows[0];
    const place = placeOf(pane, top.x, top.y, top.width);
    current = { candidate: candidateOf(candidate), seen: link.seen, place };
    if (link.seen.does.does !== "refused") draw(rows);
    hands.hovered({ ...link, place });
  };

  /** The marked link a range xterm hands the link handler is part of, with its target. */
  const markedAt = (range: IBufferRange, target: string): Marked | undefined =>
    markedNow(pane, marked).find(
      (one) => one.target === target && covers(one, range.start.x - 1, range.start.y - 1),
    );
  /**
   * **What a marked link is, as the core is handed it**: the target the program named, as xterm
   * hands it (cut past {@link MOST_TARGET}), and its words: the cells xterm links under the
   * pointer with those drawn against them ({@link besideOf}), and the words read whole where the
   * link was recorded, where they hold more. The
   * core asks first if any of them names another target.
   */
  const markedCandidate = (target: string, range: IBufferRange): LinkCandidate => {
    const cut = cutTarget(target);
    const runs = rowsOf(range, pane.cols);
    // As in wordsOf, a row's blank tail inside the link is no space.
    const cells = runs
      .map(
        ({ x, y, width }, at) =>
          pane.buffer.active.getLine(y)?.translateToString(at < runs.length - 1, x, x + width) ??
          "",
      )
      .join("");
    const linked = withBeside(
      pane,
      cells,
      { y: range.start.y - 1, x: range.start.x - 1 },
      { y: range.end.y - 1, x: range.end.x },
    );
    const one = markedAt(range, cut);
    const words = one === undefined ? "" : wordsOf(pane, one);
    const text = words.includes(linked) ? words : [words, linked].filter(Boolean).join(" ");
    return { kind: "osc8", text, target: cut };
  };
  /** Which hover of a marked link the core's answer is for: one that came after it wins. */
  let markedHovers = 0;
  const handler: ILinkHandler = {
    // Every scheme reaches the core, which says why it will not open one (#6218).
    allowNonHttpProtocols: true,
    activate(event, target, range) {
      // A plain click is the selection's, as in any terminal.
      if (!opensALink(event, hands.mac())) return;
      event.preventDefault();
      // The core resolves it again, and a refusal is shown: xterm drew it as a link.
      void openLink(markedCandidate(target, range), hands);
    },
    hover(_event, target, range) {
      const candidate = markedCandidate(target, range);
      const rows = rowsOf(range, pane.cols);
      if (rows.length === 0) return;
      const here = `osc8\u0000${range.start.x},${range.start.y}\u0000${linkKey(candidate)}`;
      markedHovers += 1;
      const asked = markedHovers;
      void seenOf(candidate).then((seen) => {
        if (seen === null || asked !== markedHovers) return;
        hoverOn(here, rows, candidate, { seen, text: candidate.text, marked: true });
      });
    },
    leave() {
      markedHovers += 1;
      leaveSoon();
    },
  };
  pane.options.linkHandler = handler;

  const provider = pane.registerLinkProvider({
    provideLinks(line, give) {
      const y = line - 1;
      const wrapped = wrappedAt(pane, y);
      if (wrapped === undefined) return give(undefined);
      const markedHere = markedNow(pane, marked);
      const onRow = candidatesIn(wrapped.text).filter((found) => {
        const cells = wrapped.cells.slice(found.start, found.end);
        return (
          !cutShort(found, wrapped) &&
          cells.some((cell) => cell.y === y) &&
          // A program's own link over the same words is the link there (#6218).
          !cells.some((cell) => markedHere.some((one) => covers(one, cell.x, cell.y)))
        );
      });
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
          const rows = new Map<number, CellRun>();
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
              const top = [...rows.values()][0];
              void openLink(candidate, hands, null, placeOf(pane, top.x, top.y, top.width));
            },
            hover() {
              hoverOn(`${first.x},${first.y}\u0000${found.text}`, [...rows.values()], candidate, {
                seen,
                text: found.text,
              });
            },
            leave: leaveSoon,
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
      markings.dispose();
      if (pane.options.linkHandler === handler) pane.options.linkHandler = INERT_LINKS;
      kept.clear();
    },
    onScreen: () => screenLinks(pane, hands, kept, markedNow(pane, marked)),
  };
}

/**
 * **The link handler of a terminal that links nothing** (#6218): a program's marked link
 * (OSC 8) opens nothing there. Without one, xterm would ask with the browser's own dialog and
 * open the address itself.
 */
export const INERT_LINKS: ILinkHandler = { activate: () => {} };

/** A candidate on screen, the cell it starts at, which orders the list, and where it is drawn. */
type OnScreen = { candidate: LinkCandidate; y: number; x: number; place: LinkPlace };

/** The candidates on `pane`'s screen, newest first, each once: {@link LinkedPane.onScreen}.
 *  `marked` are the program's own links, which the patterns' candidates under them give way to. */
function onScreenNewestFirst(pane: Terminal, marked: Marked[]): OnScreen[] {
  const buffer = pane.buffer.active;
  const top = buffer.viewportY;
  const bottom = top + pane.rows - 1;
  const found: OnScreen[] = [];
  for (let y = top; y <= bottom;) {
    const wrapped = wrappedAt(pane, y);
    if (wrapped === undefined) break;
    const rows = wrapped.cells.map((cell) => cell.y);
    const last = rows.length === 0 ? y : Math.max(y, ...rows);
    for (const one of candidatesIn(wrapped.text)) {
      if (cutShort(one, wrapped)) continue;
      const cells = wrapped.cells.slice(one.start, one.end);
      const under = cells.some((cell) => marked.some((link) => covers(link, cell.x, cell.y)));
      // Where it is drawn: its first row on screen, from its first cell there.
      const shown = cells.filter((cell) => cell.y >= top && cell.y <= bottom);
      const first = shown[0];
      if (under || first === undefined) continue;
      const onItsRow = shown.filter((cell) => cell.y === first.y);
      const width = onItsRow[onItsRow.length - 1].x - first.x + 1;
      found.push({
        candidate: { kind: one.kind, text: one.text },
        y: cells[0].y,
        x: cells[0].x,
        place: placeOf(pane, first.x, first.y, width),
      });
    }
    y = last + 1;
  }
  for (const link of marked) {
    if (link.end.line < top || link.start.line > bottom) continue;
    const text = wordsOf(pane, link);
    // Where it is drawn: its first row on screen, from its first cell there.
    const y = Math.max(link.start.line, top);
    const x = y === link.start.line ? link.startX : 0;
    const width = (y === link.end.line ? link.endX : pane.cols) - x;
    found.push({
      candidate: { kind: "osc8", text, target: link.target },
      y: link.start.line,
      x: link.startX,
      place: placeOf(pane, x, y, Math.max(width, 1)),
    });
  }
  found.sort((a, b) => a.y - b.y || a.x - b.x);
  const once = new Set<string>();
  return found.reverse().filter((one) => {
    const key = linkKey(one.candidate);
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
  marked: Marked[],
): Promise<ScreenLink[]> {
  const found = onScreenNewestFirst(pane, marked);
  /** Each candidate's answer, held here too: one the pane lets go of still counts now. */
  const answerOf = new Map<string, Promise<LinkSeen | null>>();
  const unasked: LinkCandidate[] = [];
  for (const { candidate } of found) {
    const had = held(kept, linkKey(candidate));
    if (had === undefined) unasked.push(candidate);
    else answerOf.set(linkKey(candidate), had);
  }
  if (unasked.length > 0) {
    const asked = hands.core.resolve(hands.from(), unasked.map(candidateOf));
    unasked.forEach((one, at) => {
      const key = linkKey(one);
      answerOf.set(
        key,
        keep(
          kept,
          one,
          asked.then((answers) => answers[at] ?? null),
        ),
      );
    });
  }
  const answers = await Promise.all(
    found.map((one) => answerOf.get(linkKey(one.candidate)) ?? null),
  );
  return found.flatMap((one, at) => {
    const seen = answers[at];
    return seen === null ? [] : [{ candidate: candidateOf(one.candidate), seen, place: one.place }];
  });
}
