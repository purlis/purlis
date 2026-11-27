// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type {
  IBufferCell,
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
 *   **This is the one part that runs while output streams** ({@link marking}): it notes where
 *   each marked link opens and closes, and whether a cursor move or erase came between, through
 *   30 CSI and 6 ESC handlers that each return at once. Measured on 2026-11-18 (#6601), with
 *   xterm's parser alone and nothing drawn: 8 MB of a busy TUI's output, five such sequences a
 *   line, parsed about 4% slower; nothing but cursor moves, about 11% (10 ns a sequence).
 *   xterm draws it as a link under the pointer whatever it is, so ⌘-click on one the core
 *   refuses says why. Where a program marks links is the program's: purlis changes no
 *   harness's environment to coax them out of it.
 */

/**
 * A pattern's candidate on a line: its kind hint, its text and where in the line it sits. One
 * joined across a line break the program wrote at the pane's edge ({@link candidatesAcross})
 * keeps the `pieces` each row read alone, which stand where the core says the joined words
 * are no link.
 */
export type Found = LinkCandidate & { start: number; end: number; pieces?: Found[] };

/** An address's scheme and the `://` after it, as {@link WEB} reads one. */
const SCHEME = String.raw`\p{L}[\p{L}\p{N}+.-]*:\/\/`;

/**
 * An address as a pane prints it, `scheme://` and up to the first space, quote or angle bracket:
 * http(s), which opens, and any other scheme, which the core says it will not open.
 */
const WEB = new RegExp(String.raw`(?<![\p{L}\p{N}+.-])${SCHEME}[^\s<>"'${"`"}\p{Cc}]+`, "giu");

/**
 * **A chat's number** (#6217): the word `chat` and a number, as a word of its own. A bare `#641`
 * is an issue's number and never a chat, and a title never links: the core looks the number up
 * in the pane's project, and one no chat there had is no link. At most nine digits and no leading
 * zero, as the core reads one (`panelinks/chat.rs`, `MOST_DIGITS`).
 */
const CHAT = /(?<![\p{L}\p{N}_#/.-])[Cc]hat [1-9]\d{0,8}(?![\p{L}\p{N}_])/gu;

/**
 * **An issue or pull request's number** (#6223): `#6223`, or `owner/repo#6223` (a GitLab group may
 * have subgroups), as a word of its own. Not after a letter, a digit, `&` (an HTML entity), `/`,
 * `#` or a word's own characters, and at most nine digits with no leading zero, as the core reads
 * one (`panelinks/issue.rs`). The core finds the forge from the chat's folder's remote; with none,
 * it is no link and says nothing.
 */
const ISSUE =
  /(?<![\p{L}\p{N}_.&#/-])(?:[A-Za-z0-9][\w.-]*(?:\/[A-Za-z0-9][\w.-]*)+)?#[1-9]\d{0,8}(?![\p{L}\p{N}_#])/gu;

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
 * then issue and pull request numbers outside them (`#6223`, `owner/repo#6223`, #6223), then file
 * paths outside both, with the punctuation a sentence puts after one left out, then chats'
 * numbers (`chat 641`, #6217) outside all of them.
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
  for (const match of line.matchAll(ISSUE)) {
    const start = match.index;
    const end = start + match[0].length;
    if (found.some((one) => start < one.end && end > one.start)) continue;
    // `owner/repo#N` may be a file's words too (`docs/guide.md#12`): both readings are offered
    // and the core decides on the disk, a file where one is there, else an issue.
    found.push({ kind: "issue", text: match[0], start, end });
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
 *  path, a link a program marked whose target is a `file:` address, or an issue's number, read
 *  from the git remote of the folder the chat is in now (#6223). */
const namesAFile = (link: LinkCandidate) =>
  link.kind === "file" ||
  link.kind === "issue" ||
  (link.kind === "osc8" && FILE_ADDRESS.test(link.target ?? ""));

/** **The answer kept for `key`**, while it still counts: a file's for {@link FILE_ANSWER_LIFE}. */
function held(kept: Kept, key: string): Promise<LinkSeen | null> | undefined {
  const had = kept.get(key);
  if (had === undefined) return undefined;
  if ((lives.get(had) ?? Infinity) > Date.now()) return had;
  kept.delete(key);
  return undefined;
}

/** **What a link is kept and listed by**: its kind, its text, for one a program marked the
 *  target it named, and whether it was read across rows the program broke (#7819), which the
 *  core answers apart. */
export const linkKey = (link: Pick<LinkCandidate, "kind" | "text" | "target" | "joined">) =>
  `${link.kind}\u0000${link.text}\u0000${link.target ?? ""}${link.joined === true ? "\u0000joined" : ""}`;

/** **`link` as the core is handed it**: its kind and text, a marked link's target, and whether
 *  it was read across rows the program broke at the pane's edge (#7819): a web address read so
 *  asks before it opens. */
export const candidateOf = (link: LinkCandidate): LinkCandidate => ({
  kind: link.kind,
  text: link.text,
  ...(link.target === undefined || link.target === null ? {} : { target: link.target }),
  ...(link.joined === true ? { joined: true } : {}),
});

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
 * A line as xterm holds it across the rows it wraps over: its text, each character's cell,
 * whether the line went on before or after the part read, its first and last rows, and where in
 * its text the program broke it at the pane's edge and the reading joined it again (`joints`).
 */
type Wrapped = {
  text: string;
  cells: { x: number; y: number }[];
  cutBefore: boolean;
  cutAfter: boolean;
  first: number;
  last: number;
  joints: number[];
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
  return { text, cells, cutBefore, cutAfter, first, last, joints: [] };
}

/** A character that draws a blank, as a space does: White_Space, the braille blank U+2800 and
 *  the Hangul fillers. */
const LOOKS_BLANK = /^[\s\u115F\u1160\u2800\u3164\uFFA0]$/u;

/**
 * A cell's colour as it was written: the default foreground and the default background are
 * colours of their own, each the theme's, and never one another.
 */
function colourOf(cell: IBufferCell, ground: "fg" | "bg"): string {
  if (ground === "fg") {
    if (cell.isFgDefault()) return "fg";
    return `${cell.isFgRGB() ? "rgb" : "palette"} ${cell.getFgColor()}`;
  }
  if (cell.isBgDefault()) return "bg";
  return `${cell.isBgRGB() ? "rgb" : "palette"} ${cell.getBgColor()}`;
}

/**
 * Whether `cell`'s characters are drawn in the colour they are drawn on: they show nothing.
 * Inverting them (SGR 7) swaps the two and changes nothing, and the default colours inverted
 * still differ. Bold drawn in a bright colour is taken as the colour written, so such a cell is
 * read as drawing nothing, which only ever keeps a row from joining.
 */
const inItsBackground = (cell: IBufferCell) => colourOf(cell, "fg") === colourOf(cell, "bg");

/**
 * **What cell `x` of `line` draws** (#7819): its characters, or `""` where it draws nothing. A
 * cell no program wrote draws nothing, as do one the program concealed (SGR 8), one drawn in
 * its own background colour ({@link inItsBackground}) and one whose
 * characters are each a blank ({@link LOOKS_BLANK}) or draw nothing by the core's rule
 * (`nothing`, {@link LinkCore.drawsNothing}). The one rule the pane reads a row's ink by: its
 * indent, whether it reaches the edge, and where its written cells end.
 */
function drawnAt(line: IBufferLine | undefined, x: number, nothing: DrawsNothing): string {
  const cell = line?.getCell(x);
  // A wide character's second half is drawn by its first.
  const own = cell?.getWidth() === 0 ? line?.getCell(x - 1) : cell;
  if (own === undefined || own.isInvisible() || inItsBackground(own)) return "";
  const chars = own.getChars();
  return [...chars].every((ch) => LOOKS_BLANK.test(ch) || nothing(ch)) ? "" : chars;
}

/** How many cells row `line` starts with that draw nothing: the indent a program put on it. */
function indentOf(line: IBufferLine, nothing: DrawsNothing): number {
  let x = 0;
  while (x < line.length && drawnAt(line, x, nothing) === "") x += 1;
  return x;
}

/**
 * The markers a row may open with before words the rows after it hang under (#7930): the
 * bullets Claude Code (`⏺ `), Markdown and common command-line programs put before a reply or
 * a list item. A short fixed set, so a row opening with any other character and a blank hangs
 * nothing.
 */
const MARKERS = new Set([..."\u23FA\u2022\u25CF\u25E6\u25AA\u2023\u002D\u002A\u002B"]);

/** The most digits a numbered list item's number is read with. */
const MOST_DIGITS = 3;

/**
 * Where a numbered list item's marker that opens at column `at` of `line` ends: one to
 * {@link MOST_DIGITS} digits then `.` or `)`, as `1. ` and `12) ` number a list. Just past it,
 * or `undefined` where no such marker opens there.
 */
function pastANumber(line: IBufferLine, at: number, nothing: DrawsNothing): number | undefined {
  let x = at;
  while (x - at < MOST_DIGITS && /^\d$/u.test(drawnAt(line, x, nothing))) x += 1;
  if (x === at || !/^[.)]$/u.test(drawnAt(line, x, nothing))) return undefined;
  return x + 1;
}

/** Where a marker that opens at column `at` of `line` ends, a bullet ({@link MARKERS}) or a
 *  number ({@link pastANumber}): just past it, or `undefined` where none opens there. */
function pastAMarker(line: IBufferLine, at: number, nothing: DrawsNothing): number | undefined {
  if (MARKERS.has(drawnAt(line, at, nothing)))
    return at + Math.max(1, line.getCell(at)?.getWidth() ?? 1);
  return pastANumber(line, at, nothing);
}

/**
 * **The hanging indent of `line`, the first row of a line** (#7930): the blank cells it starts
 * with, and where those are followed by a marker ({@link pastAMarker}) and one blank before a
 * written cell, the marker's cells and that blank as well, where the line's words start.
 */
function hangingIndentOf(line: IBufferLine, nothing: DrawsNothing): number {
  const indent = indentOf(line, nothing);
  const past = pastAMarker(line, indent, nothing);
  if (past === undefined) return indent;
  const words = past + 1;
  const hangs = drawnAt(line, past, nothing) === "" && drawnAt(line, words, nothing) !== "";
  return hangs ? words : indent;
}

/**
 * Whether row `line`, past `indent`, opens as an item of its own: one character that is no
 * letter or digit (a list's marker, a bullet, Claude Code's `⎿`), or a list item's number
 * ({@link pastANumber}), then a blank. Under a row that opens with a marker it is the next
 * item, never words going on (#7930).
 */
function opensAnItem(line: IBufferLine, indent: number, nothing: DrawsNothing): boolean {
  const opens = drawnAt(line, indent, nothing);
  if (opens === "") return false;
  const after = /^[\p{L}\p{N}]/u.test(opens)
    ? pastANumber(line, indent, nothing)
    : indent + Math.max(1, line.getCell(indent)?.getWidth() ?? 1);
  return after !== undefined && drawnAt(line, after, nothing) === "";
}

/**
 * A character a link goes on with past a break the program wrote: a letter (with its marks), a
 * digit, or the punctuation an address or a path holds (RFC 3986's unreserved and reserved
 * characters, and `%`). A frame's border, a bullet, a reply's marker, a check mark, an arrow or
 * an ellipsis is none of them, so a link never runs on into one or out of one.
 */
const GOES_ON_WITH = /^[\p{L}\p{M}\p{N}\-._~:/?#[\]@!$&'()*+,;=%]$/u;

/** Whether every character of `text` is one a link goes on with ({@link GOES_ON_WITH}). */
const goesOnWith = (text: string) => [...text].every((ch) => GOES_ON_WITH.test(ch));

/**
 * **Where a row's words may go on onto the next row the program wrote** (#7819), and the indent
 * they go on after: `row` fills the pane to its last column, and the row after it is a line of
 * its own (no soft wrap) that starts with as many blank cells as the hanging indent of the first
 * row of `row`'s line ({@link hangingIndentOf}), then a cell a link goes on with
 * ({@link GOES_ON_WITH}) that does not open an item of its own ({@link opensAnItem}). What
 * draws is read by {@link drawnAt}. Only then may a link's words
 * that reach the edge be read on there. On the alternate screen (a full-screen program such as
 * an editor, a pager or a multiplexer) a row is never read on: its rows are a picture the
 * program drew, which may be cut at the edge, not text it broke there.
 */
function goesOnAfter(pane: Terminal, row: number, nothing: DrawsNothing): number | undefined {
  const buffer = pane.buffer.active;
  if (buffer.type === "alternate") return undefined;
  const line = buffer.getLine(row);
  const next = buffer.getLine(row + 1);
  if (line === undefined || next === undefined || next.isWrapped) return undefined;
  if (drawnAt(line, line.length - 1, nothing) === "") return undefined;
  let first = row;
  while (first > 0 && row - first < MOST_ROWS && buffer.getLine(first)?.isWrapped) first -= 1;
  const start = buffer.getLine(first);
  const indent = start === undefined ? 0 : hangingIndentOf(start, nothing);
  if (indentOf(next, nothing) !== indent || opensAnItem(next, indent, nothing)) return undefined;
  const opens = drawnAt(next, indent, nothing);
  return opens === "" || !goesOnWith(opens) ? undefined : indent;
}

/** A row that starts a link of its own, an address or a path from `~` or `.`: never the end of
 *  the link on the row before. */
const STARTS_A_LINK = new RegExp(String.raw`^(?:${SCHEME}|~\/|\.{1,2}\/)`, "u");

/**
 * `line` with the blank cells it starts with, its first `indent` columns, left out: the
 * characters of its first row's cells left of column `indent`, however many each holds (a
 * combining mark, a wide character's two columns).
 */
function unindented(line: Wrapped, indent: number): Wrapped {
  let cut = 0;
  while (
    cut < line.cells.length &&
    line.cells[cut].y === line.first &&
    line.cells[cut].x < indent
  )
    cut += 1;
  return { ...line, text: line.text.slice(cut), cells: line.cells.slice(cut) };
}

/** `before` and `after`, the lines either side of a line break the program wrote at the pane's
 *  edge, read as one, the break a joint. */
function joined(before: Wrapped, after: Wrapped): Wrapped {
  return {
    text: before.text + after.text,
    cells: [...before.cells, ...after.cells],
    cutBefore: before.cutBefore,
    cutAfter: after.cutAfter,
    first: before.first,
    last: after.last,
    joints: [
      ...before.joints,
      before.text.length,
      ...after.joints.map((at) => at + before.text.length),
    ],
  };
}

/** Whether a candidate of `line` reaches its very end: a link the next row may go on with. */
const endsInALink = (line: Wrapped) =>
  candidatesAcross(line).some((one) => one.end === line.text.length);

/** How many rows `line` is drawn over. */
const rowsIn = (line: Wrapped) => line.last - line.first + 1;

/**
 * **`line` read on across the line breaks the program wrote after it** while a link's words
 * reach the pane's edge ({@link goesOnAfter}) and the next line, past its indent, does not
 * start a link of its own, to at most {@link MOST_ROWS} rows; a line that goes on past them is
 * cut.
 */
function readOn(pane: Terminal, from: Wrapped, nothing: DrawsNothing): Wrapped {
  let line = from;
  for (;;) {
    const indent = line.cutAfter ? undefined : goesOnAfter(pane, line.last, nothing);
    if (indent === undefined || !endsInALink(line)) break;
    const next = wrappedAt(pane, line.last + 1);
    if (next === undefined) break;
    const after = unindented(next, indent);
    if (STARTS_A_LINK.test(after.text)) break;
    if (rowsIn(line) + rowsIn(after) > MOST_ROWS) return { ...line, cutAfter: true };
    line = joined(line, after);
  }
  return line;
}

/**
 * **Buffer line `y` as it reads, across the rows it wraps over** (#7819): the rows the terminal
 * wrapped (`isWrapped`), and the lines a link's words go on with past a line break the program
 * wrote at the pane's edge ({@link readOn}). It is read from the first row of the run of such
 * breaks `y` is in, so every row of a link reads the same line whichever the pointer is on.
 */
function lineAt(pane: Terminal, y: number, nothing: DrawsNothing): Wrapped | undefined {
  let start = wrappedAt(pane, y);
  if (start === undefined) return undefined;
  let rows = rowsIn(start);
  let cut = false;
  for (;;) {
    const indent =
      start.cutBefore || start.first === 0
        ? undefined
        : goesOnAfter(pane, start.first - 1, nothing);
    if (indent === undefined || STARTS_A_LINK.test(unindented(start, indent).text)) break;
    const before = wrappedAt(pane, start.first - 1);
    if (before === undefined) break;
    rows += rowsIn(before);
    if (rows > MOST_ROWS) {
      cut = true;
      break;
    }
    start = before;
  }
  let line = readOn(pane, cut ? { ...start, cutBefore: true } : start, nothing);
  while (line.last < y) {
    const next = wrappedAt(pane, line.last + 1);
    if (next === undefined) return undefined;
    // After a line cut at its end, the next goes on from words not read.
    line = readOn(pane, line.cutAfter ? { ...next, cutBefore: true } : next, nothing);
  }
  return line;
}

/**
 * A number at a line's edge, an issue's or a chat's, or a path's line and column: digits on the
 * next row are as likely a word of their own, and taken on they name another target (#7819).
 */
const endsInItsNumber = (one: Found) =>
  one.kind === "issue" || one.kind === "chat" || (one.kind === "file" && /:\d+$/u.test(one.text));

/** An issue's number as a word of its own at a row's start, `#12` then a blank or the end. */
const OPENS_WITH_A_NUMBER = /^#\d+(?![\p{L}\p{N}_-])/u;

/**
 * **Whether `last`, a candidate that reaches a joint, goes on as `whole` into `next`**, the words
 * past the joint: `whole` is read from the same start, is of the same kind and reaches past the
 * joint, and the join takes in nothing that reads as words of their own. Nothing either side of
 * the joint a link does not go on with ({@link GOES_ON_WITH}: a frame's border, a bullet, a
 * marker), no digits onto a number ({@link endsInItsNumber}),
 * no issue's number (`#12`) onto an address as its fragment, and no second `/` after one.
 */
function goesOn(last: Found, whole: Found, at: number, next: string): boolean {
  if (whole.start !== 0 || whole.kind !== last.kind) return false;
  if (last.start + whole.end <= at) return false;
  // Both sides of the joint, and every character taken on past it, are a link's.
  const joint = at - last.start;
  if (!goesOnWith(whole.text.slice(joint - 1))) return false;
  if (endsInItsNumber(last) && /^\d/u.test(next)) return false;
  if (OPENS_WITH_A_NUMBER.test(next)) return false;
  return !(last.text.endsWith("/") && next.startsWith("/"));
}

/**
 * **The candidates of `line`, read across its joints** (#7819): each part between them is read
 * alone ({@link candidatesIn}), and a candidate that reaches a joint is read on into the next
 * part where the words joined make one candidate of the same kind from the same start. The
 * joined one keeps the `pieces` it was read from. Every candidate still goes to the core, which
 * decides what it is as it does any other: nothing is opened on the strength of the join.
 */
function candidatesAcross(line: Wrapped): Found[] {
  const bounds = [0, ...line.joints, line.text.length];
  const found: Found[] = [];
  for (let at = 0; at + 1 < bounds.length; at += 1) {
    const from = bounds[at];
    const to = bounds[at + 1];
    const here = candidatesIn(line.text.slice(from, to)).map((one) => ({
      ...one,
      start: one.start + from,
      end: one.end + from,
    }));
    const last = found.at(-1);
    if (last !== undefined && last.end === from) {
      const [whole] = candidatesIn(line.text.slice(last.start, to));
      if (whole !== undefined && goesOn(last, whole, from, line.text.slice(from))) {
        const end = last.start + whole.end;
        found.pop();
        const pieces = [...(last.pieces ?? [last]), ...here.filter((one) => one.start < end)];
        found.push(
          { kind: whole.kind, text: whole.text, start: last.start, end, pieces, joined: true },
          ...here.filter((one) => one.start >= end),
        );
        continue;
      }
    }
    found.push(...here);
  }
  return found;
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
  /** Whether the character `ch` draws nothing, by the core's rule (`purlis_unicode`'s
   *  invisible set): text either side of a cell holding only such characters reads as one word
   *  ({@link besideOf}). */
  drawsNothing: (ch: string) => boolean;
};

/** What a pane does with a link's hover, and with the open command's answer. */
export type LinkHands = {
  /** The core that decides what its links are and opens them. */
  core: LinkCore;
  /** Where its candidates were found: the pane's project and chat, where it has them. */
  from: () => LinkFrom;
  /** The pointer is on a link, or has left it (`null`). */
  hovered: (link: HoveredLink | null) => void;
  /** What the open command answered for the link ⌘-clicked, drawn at `place`: a chat's card
   *  stands beside it, and an ask answered there opens it from there again. */
  answered: (answer: LinkOpened, candidate: LinkCandidate, place: LinkPlace) => void;
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
      // With no pane, nothing is resolved, so nothing is read beside a link either.
      drawsNothing: (ch) => to()?.core.drawsNothing(ch) ?? false,
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
 *  `token` is the token of the ask the person accepted. A link is always opened from where it
 *  is drawn, so a chat's card stands at it (#6488). */
export function openLink(
  candidate: LinkCandidate,
  hands: Pick<LinkHands, "core" | "from" | "answered">,
  place: LinkPlace,
  token: string | null = null,
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

/** The first of `rows` on screen, which a link's tooltip and card stand beside: a link whose
 *  first rows have scrolled off is placed where it shows (#7819). */
function shownFirst(pane: Terminal, rows: CellRun[]): CellRun | undefined {
  const top = pane.buffer.active.viewportY;
  return rows.find((row) => row.y >= top && row.y < top + pane.rows) ?? rows[0];
}

/** How many columns the character at `cell` takes: two for a wide one, else one. */
const widthAt = (pane: Terminal, cell: Cell) =>
  Math.max(1, pane.buffer.active.getLine(cell.y)?.getCell(cell.x)?.getWidth() ?? 1);

/** The column just past the last cell of `line` that draws ({@link drawnAt}), or 0: blanks
 *  after a row's words are no part of them. */
function writtenTo(line: IBufferLine | undefined, nothing: DrawsNothing): number {
  if (line === undefined) return 0;
  for (let x = line.length - 1; x >= 0; x -= 1) if (drawnAt(line, x, nothing) !== "") return x + 1;
  return 0;
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
export type ScreenLink = { candidate: LinkCandidate; seen: LinkSeen; place: LinkPlace };

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
  /** **The folder the pane's program last said it is in** (OSC 7, #6367), as it said it, or
   *  `null`: handed to the core with each ask ({@link withFolder}), which reads it as
   *  untrusted and only ever as where a relative path starts. */
  folder: () => string | null;
};

/** **`from` with the folder a pane's program said it is in** (OSC 7, #6367), where it said one:
 *  a pane whose program said none is handed on as it was. */
export const withFolder = (from: LinkFrom, folder: string | null | undefined): LinkFrom =>
  folder === null || folder === undefined ? from : { ...from, folder };

/**
 * **A link a program marked on the screen** (OSC 8): the target it named, and the cells from
 * where it opened the link to where it closed it, held by markers so they move with the
 * buffer. Recorded as the program writes it ({@link marking}), so the whole of its words are
 * read, across the rows they wrap over.
 */
type Marked = {
  target: string;
  /**
   * Which link it is ({@link linkOf}): the cells of two links with the same identity are one
   * link, whichever was written last.
   */
  link: string;
  /** Which buffer it was written to: the normal one, or the alternate screen. */
  buffer: "normal" | "alternate";
  /** Its first cell. */
  start: MarkedCell;
  /** The cell just past its last. */
  end: MarkedCell;
  /**
   * Whether the program moved the cursor or changed the screen while the link was open (a
   * CSI or ESC sequence): what the link's cells show may then lie outside the span from its
   * start to its end, so its whole rows are read.
   */
  moved: boolean;
  /**
   * What its cells held as the program closed it ({@link spanOf}), for one it did not move the
   * cursor inside: a later write over them (the program going back up and drawing again) leaves
   * the record standing over cells that no longer hold the link.
   */
  drew?: string;
};

/**
 * **What the cells of `marked` hold now**, from its start to its end, at most
 * {@link MOST_ROWS} rows: each cell's characters, a cell no program wrote as a blank.
 */
function spanOf(pane: Terminal, marked: Pick<Marked, "start" | "end">): string {
  const buffer = pane.buffer.active;
  const last = Math.min(marked.end.y, marked.start.y + MOST_ROWS - 1);
  let text = "";
  for (let y = marked.start.y; y <= last; y += 1) {
    const line = buffer.getLine(y);
    if (line === undefined) break;
    const from = y === marked.start.y ? marked.start.x : 0;
    const to = y === marked.end.y ? marked.end.x : line.length;
    for (let x = from; x < to; x += 1) text += line.getCell(x)?.getChars() || " ";
    text += "\n";
  }
  return text;
}

/** **A cell held by a marker**: its row moves with the buffer as lines scroll off, so it is
 *  read from the marker each time, and the cell is a {@link Cell} wherever it is read. */
type MarkedCell = Cell & { readonly marker: IMarker };

/** The cell at column `x` of `marker`'s row, as the marker moves. */
const markedCell = (marker: IMarker, x: number): MarkedCell => ({
  marker,
  x,
  get y() {
    return marker.line;
  },
});

/** Whether `marked`'s markers are still held, and on the buffer. */
const stillHeld = (marked: Marked) =>
  !marked.start.marker.isDisposed &&
  !marked.end.marker.isDisposed &&
  marked.start.y >= 0 &&
  marked.end.y >= 0;

/** Lets go of `marked`'s markers. */
const letGo = (marked: Pick<Marked, "start"> & Partial<Pick<Marked, "end">>) => {
  marked.start.marker.dispose();
  marked.end?.marker.dispose();
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

/**
 * **A marked link's identity** (OSC 8): its target, with the `id` the program gave it where it
 * gave one. Two links with the same target and id are the same link, as the terminal reads them.
 */
function linkOf(params: string, target: string): string {
  const id = params
    .split(":")
    .find((one) => one.startsWith("id="))
    ?.slice("id=".length);
  return id === undefined || id === "" ? target : `${id}\u0000${target}`;
}

/** Whether buffer cell (`x`, `y`) is one of `marked`'s. */
const covers = (marked: Marked, x: number, y: number) =>
  !precedes(y, x, marked.start.y, marked.start.x) && precedes(y, x, marked.end.y, marked.end.x);

/** Whether the program wrote cell (`x`, `y`) while `marked` was open, as its record tells: a
 *  cell of its span, or of its rows where it moved the cursor inside it. */
function wroteTo(marked: Marked, x: number, y: number): boolean {
  if (!marked.moved) return covers(marked, x, y);
  const first = Math.min(marked.start.y, marked.end.y);
  return y >= first && y <= Math.max(marked.start.y, marked.end.y);
}

/** **Which link cell (`x`, `y`) holds now** ({@link Marked.link}): the last one of `marked`
 *  written over it, or none. */
function linkAtCell(marked: Marked[], x: number, y: number): string | undefined {
  for (let at = marked.length - 1; at >= 0; at -= 1)
    if (wroteTo(marked[at], x, y)) return marked[at].link;
  return undefined;
}

/**
 * **Records each link a program marks on `pane`** (OSC 8): `ESC ] 8 ; params ; target ST`
 * opens one at the cursor and an empty target closes it there. xterm's own handler still runs:
 * it draws the link and hands it to the pane's `linkHandler`.
 */
function marking(pane: Terminal, marked: Marked[]): IDisposable {
  let open: Omit<Marked, "end"> | undefined;
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
    const params = split < 0 ? "" : data.slice(0, split);
    const buffer = pane.buffer.active.type;
    if (open !== undefined) {
      const end = pane.registerMarker(0);
      const was = open;
      open = undefined;
      if (end !== undefined && was.buffer === buffer) {
        const one: Marked = { ...was, end: markedCell(end, at()) };
        if (one.moved || precedes(one.start.y, one.start.x, one.end.y, one.end.x)) {
          if (!one.moved) one.drew = spanOf(pane, one);
          marked.push(one);
          while (marked.length > MOST_MARKED) {
            const oldest = marked.shift();
            if (oldest !== undefined) letGo(oldest);
          }
        } else {
          letGo(one);
        }
      } else {
        letGo(was);
        end?.dispose();
      }
    }
    if (target !== "") {
      const start = pane.registerMarker(0);
      if (start !== undefined)
        open = {
          target: cutTarget(target),
          link: linkOf(params, cutTarget(target)),
          buffer,
          start: markedCell(start, at()),
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
      if (open !== undefined) letGo(open);
      for (const one of marked.splice(0)) letGo(one);
    },
  };
}

/**
 * **`marked`'s words**, read from the buffer across the rows they wrap over, at most
 * {@link MOST_ROWS} of them: from its start to its end, or its whole rows where the program
 * moved the cursor while it was open.
 */
function wordsOf(pane: Terminal, marked: Marked, nothing: DrawsNothing): string {
  const buffer = pane.buffer.active;
  const first = Math.min(marked.start.y, marked.end.y);
  const last = Math.min(Math.max(marked.start.y, marked.end.y), first + MOST_ROWS - 1);
  let text = "";
  for (let y = first; y <= last; y += 1) {
    const line = buffer.getLine(y);
    if (line === undefined) break;
    const from = !marked.moved && y === marked.start.y ? marked.start.x : 0;
    const to = !marked.moved && y === marked.end.y ? marked.end.x : line.length;
    let row = "";
    let blank = "";
    for (let x = from; x < to; x += 1) {
      const cell = line.getCell(x);
      if (cell === undefined || cell.getWidth() === 0) continue;
      // A cell that draws nothing ({@link drawnAt}) is a blank, as the pane reads every row.
      const chars = drawnAt(line, x, nothing);
      if (chars === "") {
        blank += " ";
        continue;
      }
      row += blank + chars;
      blank = "";
    }
    // A row's cells that draw nothing, to its end, are no space inside one link: its words go
    // on on the next row, where a hard line break put them.
    text += marked.moved || y === last ? row + blank : row;
  }
  if (marked.moved) return text.trim();
  return withBeside(pane, nothing, text, marked.start, marked.end);
}

/** Where a cell is on the buffer, zero-based. */
type Cell = { y: number; x: number };

/** The core's rule for a character that draws nothing ({@link LinkCore.drawsNothing}). */
type DrawsNothing = (ch: string) => boolean;

/**
 * **A link's words `text`, and again with the characters drawn against them** ({@link besideOf}),
 * the link running from cell `start` to just before `end`. A reader may read text drawn against
 * the link as one word with it, or the link's own words alone, so the core reads both: the
 * joined word never stands in for the drawn one. Across a blank, nothing is joined.
 */
function withBeside(
  pane: Terminal,
  nothing: DrawsNothing,
  text: string,
  start: Cell,
  end: Cell,
): string {
  const { before, after } = besideOf(pane, nothing, start, end);
  const own = text.trim();
  const joined = (
    (/^\s/u.test(text) ? "" : before) +
    text +
    (/\s$/u.test(text) ? "" : after)
  ).trim();
  return joined === own ? own : `${own} ${joined}`;
}

/**
 * **The characters drawn next to a link, with no space between**: from the cell before `start`
 * back, and from `end` (just past the link) on, across the rows they wrap over, to a blank or
 * {@link MOST_TARGET} characters. A link's words are read whole with them: a program may print
 * text against its link's cells, and the reader reads them as one word.
 */
function besideOf(
  pane: Terminal,
  nothing: DrawsNothing,
  start: Cell,
  end: Cell,
): { before: string; after: string } {
  const buffer = pane.buffer.active;
  // What a cell draws: its characters but those that draw nothing (a line or paragraph
  // separator, an invisible), as the core reads them. xterm folds a zero-width character into
  // the cell before it, so a separator's cell may hold a mark that draws (U+2028 U+0301): the
  // mark is part of the word. `undefined` for a cell no program wrote, which reads as a blank.
  const drawn = (chars: string): string | undefined =>
    chars === "" ? undefined : [...chars].filter((ch) => !nothing(ch)).join("");
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
    const chars = drawn(cell.getChars());
    if (chars === undefined || /\p{White_Space}/u.test(chars)) break;
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
    const chars = drawn(cell.getChars());
    if (chars === undefined || /\p{White_Space}/u.test(chars)) break;
    after += chars;
  }
  return { before, after };
}

/** The marked links still on `pane`'s buffer now, of the buffer it shows. */
function markedNow(pane: Terminal, marked: Marked[]): Marked[] {
  const buffer = pane.buffer.active.type;
  return marked.filter((one) => one.buffer === buffer && stillHeld(one));
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
 * **`found` with the core's answer for it** and, where it was joined across a line break the
 * program wrote (#7819) and the core says the joined words are no link, each of its `pieces`
 * that `keeps` with its answer instead: the words each row read alone link as they did before.
 */
async function answeredWithPieces(
  found: Found,
  keeps: (piece: Found) => boolean,
  seenOf: (candidate: LinkCandidate) => Promise<LinkSeen | null>,
): Promise<[Found, LinkSeen | null][]> {
  const seen = await seenOf(found);
  if (seen !== null || found.pieces === undefined) return [[found, seen]];
  const pieces = found.pieces.filter(keeps);
  const answers = await Promise.all(pieces.map(seenOf));
  return pieces.map((piece, at) => [piece, answers[at]]);
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

  /** **The folder the program last said it is in** (OSC 7, `file://host/path`), cut as a
   *  marked link's target is: the core reads it, never the window. A line's answers may name
   *  files from the folder before, so they are let go of when it moves. */
  let folder: string | null = null;
  const saidFolder = pane.parser.registerOscHandler(7, (data) => {
    const said = cutTarget(data);
    if (said !== folder) {
      folder = said;
      kept.clear();
    }
    // Nothing else of the terminal's reads it.
    return false;
  });

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
    const top = shownFirst(pane, rows);
    if (top === undefined) return;
    if (leaving !== undefined) {
      clearTimeout(leaving);
      leaving = undefined;
      if (on === here) return;
      leave();
    }
    on = here;
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
      hands.core.drawsNothing,
      cells,
      { y: range.start.y - 1, x: range.start.x - 1 },
      { y: range.end.y - 1, x: range.end.x },
    );
    const one = markedAt(range, cut);
    const words = one === undefined ? "" : wordsOf(pane, one, hands.core.drawsNothing);
    // A word of the cells' that the recorded words hold already (a wrapped row's part) adds
    // nothing.
    const more = linked.split(/\s+/u).filter((word) => word !== "" && !words.includes(word));
    const text = [words, ...more].filter(Boolean).join(" ");
    return { kind: "osc8", text, target: cut };
  };
  /**
   * **The rows a marked link is drawn over** (#7819): xterm hands the link handler one row's part
   * of a link whose words wrap, so the whole of it, as it was recorded, is drawn in the link
   * colour from any of its rows. One the program moved the cursor inside is only the part
   * xterm hands, as is one whose cells were written over after it closed, whose other rows may
   * now hold another link: other words, or a later link of another identity
   * ({@link Marked.link}) over any of its cells, whatever words it draws. `at` says which link
   * it is, the same from each of its rows.
   */
  const drawnOver = (target: string, range: IBufferRange): { rows: CellRun[]; at: string } => {
    const one = markedAt(range, cutTarget(target));
    // As its words are read (wordsOf): at most MOST_ROWS rows. Hovered past them, it is the
    // part xterm hands, as is one the program moved the cursor inside.
    const last = one === undefined ? 0 : Math.min(one.end.y, one.start.y + MOST_ROWS - 1);
    const xterms = { rows: rowsOf(range, pane.cols), at: `${range.start.x},${range.start.y}` };
    if (
      one === undefined ||
      one.moved ||
      range.start.y - 1 > last ||
      spanOf(pane, one) !== one.drew
    )
      return xterms;
    const rows: CellRun[] = [];
    // On a row it goes on from, only the cells written.
    for (let y = one.start.y; y <= last; y += 1) {
      const from = y === one.start.y ? one.start.x : 0;
      const to =
        y === one.end.y
          ? one.end.x
          : writtenTo(pane.buffer.active.getLine(y), hands.core.drawsNothing);
      if (to > from) rows.push({ x: from, y, width: to - from });
    }
    // A cell a later link was written over, even with the same words, is that link's.
    const now = markedNow(pane, marked);
    const another = rows.some((run) =>
      Array.from({ length: run.width }, (_, at) => run.x + at).some(
        (x) => linkAtCell(now, x, run.y) !== one.link,
      ),
    );
    return another ? xterms : { rows, at: `${one.start.x + 1},${one.start.y + 1}` };
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
      const top = shownFirst(pane, drawnOver(target, range).rows);
      if (top === undefined) return;
      void openLink(markedCandidate(target, range), hands, placeOf(pane, top.x, top.y, top.width));
    },
    hover(_event, target, range) {
      const candidate = markedCandidate(target, range);
      const { rows, at } = drawnOver(target, range);
      if (rows.length === 0) return;
      const here = `osc8\u0000${at}\u0000${linkKey(candidate)}`;
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
      const wrapped = lineAt(pane, y, hands.core.drawsNothing);
      if (wrapped === undefined) return give(undefined);
      const markedHere = markedNow(pane, marked);
      const onTheRow = (found: Found) => {
        const cells = wrapped.cells.slice(found.start, found.end);
        return (
          !cutShort(found, wrapped) &&
          cells.some((cell) => cell.y === y) &&
          // A program's own link over the same words is the link there (#6218).
          !cells.some((cell) => markedHere.some((one) => covers(one, cell.x, cell.y)))
        );
      };
      const onRow = candidatesAcross(wrapped).filter(onTheRow);
      if (onRow.length === 0) return give(undefined);
      void Promise.all(onRow.map((found) => answeredWithPieces(found, onTheRow, seenOf))).then(
        (answered) => {
          const links: ILink[] = [];
          answered.flat().forEach(([found, seen]) => {
            if (seen === null) return;
            const cells = wrapped.cells.slice(found.start, found.end);
            const first = cells[0];
            const end = cells[cells.length - 1];
            // Each row the link is drawn over, and the cells of it.
            const rows = new Map<number, CellRun>();
            for (const cell of cells) {
              const row = rows.get(cell.y);
              const past = cell.x + widthAt(pane, cell);
              if (row === undefined)
                rows.set(cell.y, { x: cell.x, y: cell.y, width: past - cell.x });
              else row.width = past - row.x;
            }
            const candidate = candidateOf(found);
            // What purlis will not open says why on its hover, and is drawn as no link.
            const opens = seen.does.does !== "refused";
            links.push({
              range: {
                start: { x: first.x + 1, y: first.y + 1 },
                end: { x: end.x + widthAt(pane, end), y: end.y + 1 },
              },
              text: found.text,
              decorations: { pointerCursor: opens, underline: opens },
              activate(event) {
                // A plain click is the selection's, as in any terminal.
                if (!opens || !opensALink(event, hands.mac())) return;
                event.preventDefault();
                const top = shownFirst(pane, [...rows.values()]);
                if (top === undefined) return;
                void openLink(candidate, hands, placeOf(pane, top.x, top.y, top.width));
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
        },
      );
    },
  });
  return {
    under: () => current,
    folder: () => folder,
    dispose() {
      clearTimeout(leaving);
      undraw();
      provider.dispose();
      markings.dispose();
      saidFolder.dispose();
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

/** A candidate on screen, the cell it starts at, which orders the list, and where it is drawn;
 *  for one joined across a line break the program wrote (#7819), the pieces listed where the
 *  core says the joined words are no link. */
type OnScreen = {
  candidate: LinkCandidate;
  y: number;
  x: number;
  place: LinkPlace;
  pieces?: OnScreen[];
};

/** The candidates on `pane`'s screen, newest first, each once: {@link LinkedPane.onScreen}.
 *  `marked` are the program's own links, which the patterns' candidates under them give way to. */
function onScreenNewestFirst(pane: Terminal, marked: Marked[], nothing: DrawsNothing): OnScreen[] {
  const buffer = pane.buffer.active;
  const top = buffer.viewportY;
  const bottom = top + pane.rows - 1;
  const found: OnScreen[] = [];
  for (let y = top; y <= bottom;) {
    const wrapped = lineAt(pane, y, nothing);
    if (wrapped === undefined) break;
    /** Where `one` is drawn: its first row on screen, from its first cell there. */
    const shownAs = (one: Found): OnScreen | undefined => {
      if (cutShort(one, wrapped)) return undefined;
      const cells = wrapped.cells.slice(one.start, one.end);
      const under = cells.some((cell) => marked.some((link) => covers(link, cell.x, cell.y)));
      const shown = cells.filter((cell) => cell.y >= top && cell.y <= bottom);
      const first = shown[0];
      if (under || first === undefined) return undefined;
      const onItsRow = shown.filter((cell) => cell.y === first.y);
      const end = onItsRow[onItsRow.length - 1];
      const width = end.x + widthAt(pane, end) - first.x;
      return {
        candidate: candidateOf(one),
        y: cells[0].y,
        x: cells[0].x,
        place: placeOf(pane, first.x, first.y, width),
      };
    };
    for (const one of candidatesAcross(wrapped)) {
      const shown = shownAs(one);
      if (shown === undefined) continue;
      const pieces = one.pieces?.flatMap((piece) => shownAs(piece) ?? []);
      found.push(pieces === undefined ? shown : { ...shown, pieces });
    }
    y = Math.max(y, wrapped.last) + 1;
  }
  for (const link of marked) {
    const { start, end } = link;
    if (end.y < top || start.y > bottom) continue;
    const text = wordsOf(pane, link, nothing);
    // Where it is drawn: its first row on screen, from its first cell there.
    const y = Math.max(start.y, top);
    const x = y === start.y ? start.x : 0;
    const width = (y === end.y ? end.x : pane.cols) - x;
    found.push({
      candidate: { kind: "osc8", text, target: link.target },
      y: start.y,
      x: start.x,
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
  const found = onScreenNewestFirst(pane, marked, hands.core.drawsNothing);
  const answers = await answersFrom(
    hands,
    kept,
    found.map((one) => one.candidate),
  );
  const answerOf = (one: OnScreen) => answers.get(linkKey(one.candidate)) ?? null;
  // A joined link the core says is none lists the pieces its rows read alone (#7819).
  const listed = found.flatMap((one) => (answerOf(one) === null ? (one.pieces ?? []) : []));
  const pieceAnswers = await answersFrom(
    hands,
    kept,
    listed.map((one) => one.candidate),
  );
  const whole = new Set(found.map((one) => linkKey(one.candidate)));
  const once = new Set<string>();
  return found.flatMap((one): ScreenLink[] => {
    const seen = answerOf(one);
    if (seen !== null) return [{ candidate: candidateOf(one.candidate), seen, place: one.place }];
    return (one.pieces ?? []).flatMap((each) => {
      const key = linkKey(each.candidate);
      const its = pieceAnswers.get(key) ?? null;
      if (its === null || once.has(key) || whole.has(key)) return [];
      once.add(key);
      return [{ candidate: candidateOf(each.candidate), seen: its, place: each.place }];
    });
  });
}

/** The core's answer for each of `candidates`, by its {@link linkKey}: from `kept`, or asked in
 *  one call and kept there. The same words twice are asked once. */
async function answersFrom(
  hands: Pick<LinkHands, "core" | "from">,
  kept: Kept,
  candidates: LinkCandidate[],
): Promise<Map<string, LinkSeen | null>> {
  /** Each candidate's answer, held here too: one the pane lets go of still counts now. */
  const answerOf = new Map<string, Promise<LinkSeen | null>>();
  const unasked = new Map<string, LinkCandidate>();
  for (const candidate of candidates) {
    const key = linkKey(candidate);
    if (answerOf.has(key) || unasked.has(key)) continue;
    const had = held(kept, key);
    if (had === undefined) unasked.set(key, candidate);
    else answerOf.set(key, had);
  }
  if (unasked.size > 0) {
    const ones = [...unasked.values()];
    const asked = hands.core.resolve(hands.from(), ones.map(candidateOf));
    ones.forEach((one, at) => {
      answerOf.set(
        linkKey(one),
        keep(
          kept,
          one,
          asked.then((answers) => answers[at] ?? null),
        ),
      );
    });
  }
  const keys = [...answerOf.keys()];
  const answers = await Promise.all(answerOf.values());
  return new Map(keys.map((key, at) => [key, answers[at]]));
}
