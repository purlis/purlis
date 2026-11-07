// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  useContext,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
  type RefObject,
} from "react";
import Markdown, { type Components } from "react-markdown";
import remarkBreaks from "remark-breaks";
import remarkGfm from "remark-gfm";
import { ExternalLink } from "./ReleaseNotes";
import { Button } from "./ui/button/Button";

/**
 * **What a chat wrote, as markdown under a safe subset** (#5214; the person's ruling of
 * 2026-11-04, a security call). One component for a chat's own words: its message, a task's
 * question, a note and a report, in the Inbox and on a pane's band. Before it they were text
 * nodes, so a chat's bold and lists read as raw `**` and `-`.
 *
 * **Drawn, and nothing else** (`ALLOWED`): paragraphs and line breaks (every one the chat wrote,
 * `remark-breaks`), lists, bold and italics, inline code and code blocks, tables (GitHub's
 * markdown, `remark-gfm`), and a heading, drawn as bold words in a paragraph: never larger, and
 * never a heading of the page around it. Anything past that, a quote or struck words, is drawn
 * as its plain words.
 *
 * **Never drawn:**
 * - **Raw HTML.** `skipHtml` drops it, as memories and release notes do (`ReleaseNotes.tsx`):
 *   `react-markdown` builds React elements from the syntax tree, nothing reaches `innerHTML`,
 *   and there is no `rehype-raw` and there must never be.
 * - **A picture.** No `<img>` is ever made, so no address is ever fetched: a picture's address
 *   can carry data out as it loads. A markdown image shows its alt text in its place, and inside
 *   a link it is that link's words.
 *
 * **A link** shows where it goes, its host beside its words unless those words already name
 * that host, and opens in the browser only when pressed, through the opener (`ExternalLink`, the
 * app's one way to open a link). Anything but an http or https address, a `javascript:` or
 * `data:` one above all, is drawn as its words.
 *
 * **A long message folds** to {@link FOLD_LINES} lines (three in an Inbox's row, `foldAt`) with a fade and a "Show all · N lines"
 * button; opened, it shows whole and "Show less" folds it. The fold is only to the eye: the
 * whole text is in the page for a screen reader all along.
 */
export function ChatText({
  text,
  id,
  className,
  foldStop = (button) => button,
  whole = false,
  foldAt = FOLD_LINES,
}: {
  /** The chat's words, as it wrote them. */
  text: string;
  /** The id of the words' box, for an `aria-labelledby` or `aria-describedby` that names it. */
  id?: string;
  /** Places it in its view; never a look. */
  className?: string;
  /** Its fold's button as a stop of the roving focus it stands in (the Inbox's list), where
   *  it stands in one. */
  foldStop?: (button: ReactElement) => ReactElement;
  /** **It never folds** (#5215): a pane's band opened it to show it whole. */
  whole?: boolean;
  /** **The most lines it shows folded** (#5649): {@link FOLD_LINES}, or three in an Inbox's
   *  row (`ASK_FOLD_LINES`). */
  foldAt?: 3 | typeof FOLD_LINES;
}) {
  const own = useId();
  const box = id ?? `${own}-text`;
  const body = useRef<HTMLDivElement>(null);
  const lines = useDrawnLines(body, text);
  const [open, setOpen] = useState(false);
  const folds = !whole && lines > foldAt;
  return (
    <div className={className === undefined ? "chat-text" : `chat-text ${className}`}>
      <div
        ref={body}
        id={box}
        // `release-notes`: a markdown render's own colours, read in one place (`App.css`).
        className="chat-text-body release-notes"
        data-folded={folds ? String(!open) : undefined}
        data-fold-at={foldAt === FOLD_LINES ? undefined : foldAt}
      >
        <Folded.Provider value={folds && !open}>
          <Markdown
            skipHtml
            remarkPlugins={REMARK}
            components={COMPONENTS}
            allowedElements={ALLOWED}
            unwrapDisallowed
          >
            {text}
          </Markdown>
        </Folded.Provider>
      </div>
      {folds &&
        foldStop(
          <Button
            variant="link"
            size="sm"
            aria-expanded={open}
            aria-controls={box}
            onPress={() => setOpen((was) => !was)}
          >
            {open ? "Show less" : `Show all · ${lines} lines`}
          </Button>,
        )}
    </div>
  );
}

/** The most lines a message shows before it folds (#5214). */
export const FOLD_LINES = 6;

/** **The lines a chat's text runs over as written**: every line, a blank one between paragraphs
 *  included, none counted after its last words. */
export function linesIn(text: string): number {
  const trimmed = text.trimEnd();
  return trimmed === "" ? 0 : trimmed.split("\n").length;
}

/**
 * **How many lines the words take as drawn**: measured as they are laid out and again whenever
 * their width changes, so a paragraph that wraps counts each line it wraps to. Where nothing is
 * laid out (a test's page), the lines as written.
 */
function useDrawnLines(body: RefObject<HTMLDivElement | null>, text: string): number {
  const [lines, setLines] = useState(() => linesIn(text));
  useLayoutEffect(() => {
    const element = body.current;
    const measure = () => {
      const drawn = element === null ? 0 : drawnLines(element);
      setLines(drawn > 0 ? drawn : linesIn(text));
    };
    measure();
    if (element === null || typeof ResizeObserver === "undefined") return;
    let width = element.clientWidth;
    const watch = new ResizeObserver(() => {
      if (element.clientWidth === width) return;
      width = element.clientWidth;
      measure();
    });
    watch.observe(element);
    return () => watch.disconnect();
  }, [body, text]);
  return lines;
}

/**
 * **The lines its words are laid out on, folded or not**: the boxes of its text, those starting
 * within half a line of one another being one line, so inline code a pixel off its line is that
 * line, and the gaps between paragraphs and a table's cells count for nothing. 0 where nothing
 * is laid out.
 */
function drawnLines(element: HTMLElement): number {
  const range = document.createRange();
  range.selectNodeContents(element);
  if (typeof range.getClientRects !== "function") return 0;
  const style = getComputedStyle(element);
  const written = Number.parseFloat(style.lineHeight);
  const half = (Number.isFinite(written) ? written : Number.parseFloat(style.fontSize) * 1.2) / 2;
  const tops = Array.from(range.getClientRects())
    .filter((box) => box.height > 0)
    .map((box) => box.top)
    .sort((one, other) => one - other);
  let lines = 0;
  let last = Number.NEGATIVE_INFINITY;
  for (const top of tops) {
    if (top - last >= half) {
      lines += 1;
      last = top;
    }
  }
  return lines;
}

/** What is drawn (#5214). A picture is drawn as its alt text, never as a picture. */
const ALLOWED = [
  "p",
  "br",
  "ul",
  "ol",
  "li",
  "strong",
  "em",
  "code",
  "pre",
  "table",
  "thead",
  "tbody",
  "tr",
  "th",
  "td",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "a",
  "img",
];

/** GitHub's markdown, and a line break where the chat broke its line: a chat writes for a
 *  terminal, where a new line is a new line. */
const REMARK = [remarkGfm, remarkBreaks];

/** A heading drawn as bold words in a paragraph: never larger, never one of the page's. */
const heading: Components["h1"] = ({ children }) => (
  <p>
    <strong>{children}</strong>
  </p>
);

/** A link's web address, or `undefined` where it is not one. */
function webAddress(href: string | undefined): URL | undefined {
  if (href === undefined || !/^https?:\/\//i.test(href)) return undefined;
  try {
    return new URL(href);
  } catch {
    return undefined;
  }
}

/** Whether a link's words name the host it goes to, so nothing beside them need say it. Never
 *  an address with a user before its host, `https://paypal.com@evil.example`, which reads as
 *  the user. */
function saysItsHost(words: ReactNode, address: URL): boolean {
  if (typeof words !== "string" || address.username !== "" || address.password !== "") return false;
  const said = webAddress(/^https?:\/\//i.test(words) ? words : `https://${words}`);
  return said !== undefined && said.username === "" && said.host === address.host;
}

/** Whether the words are folded: a link in them is then out of the Tab order, since it may be
 *  cut out of sight, until Show all opens them. It is still pressed, and read by a screen
 *  reader, which has the whole text. */
const Folded = createContext(false);

/** A link: where it goes beside its words, and opened only on a press. */
function Link({ href, children }: { href?: string; children?: ReactNode }) {
  const folded = useContext(Folded);
  const address = webAddress(href);
  return (
    <>
      <ExternalLink href={address?.href} reachable={!folded}>
        {children}
      </ExternalLink>
      {address !== undefined && !saysItsHost(children, address) && (
        <>
          {" "}
          <span className="chat-text-host">({address.host})</span>
        </>
      )}
    </>
  );
}

const COMPONENTS: Components = {
  h1: heading,
  h2: heading,
  h3: heading,
  h4: heading,
  h5: heading,
  h6: heading,
  // **No picture is ever loaded** (#5214): its alt text stands in its place.
  img: ({ alt }) => <span>{alt || "Picture not shown"}</span>,
  a: ({ href, children }) => <Link href={href}>{children}</Link>,
};
