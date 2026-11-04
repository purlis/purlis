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
 * 2026-11-04, a security call). One component for every place the window shows a chat's own
 * words: its question, its message, its note, in the Inbox, a pane's notices and a task's
 * question card. Before it they were text nodes, so a chat's bold and lists read as raw `**`
 * and `-`.
 *
 * **Drawn**: paragraphs and line breaks (every one the chat wrote, `remark-breaks`), lists,
 * bold and italics, inline code and code blocks, tables (GitHub's markdown, `remark-gfm`), and
 * a heading, drawn as bold words in a paragraph: never larger, and never a heading of the page
 * around it.
 *
 * **Never drawn:**
 * - **Raw HTML.** `skipHtml` drops it, as memories and release notes do (`ReleaseNotes.tsx`):
 *   `react-markdown` builds React elements from the syntax tree, nothing reaches `innerHTML`,
 *   and there is no `rehype-raw` and there must never be.
 * - **A picture.** No `<img>` is ever made, so no address is ever fetched: a picture's address
 *   can carry data out as it loads. A markdown image shows its alt text in its place, and inside
 *   a link it is that link's words.
 *
 * **A link** shows where it goes, its host beside its words, and opens in the browser only when
 * pressed, through the opener (`ExternalLink`, the app's one way to open a link). Anything but
 * an http or https address, a `javascript:` or `data:` one above all, is drawn as its words.
 *
 * **A long message folds** to {@link FOLD_LINES} lines with a fade and a "Show all · N lines"
 * button; opened, it shows whole and "Show less" folds it. The fold is only to the eye: the
 * whole text is in the page for a screen reader all along. Where the caller has to show every
 * word (`fold={false}`), it never folds.
 */
export function ChatText({
  text,
  id,
  className,
  fold = true,
  foldStop = (button) => button,
}: {
  /** The chat's words, as it wrote them. */
  text: string;
  /** The id of the words' box, for an `aria-labelledby` or `aria-describedby` that names it. */
  id?: string;
  /** Places it in its view; never a look. */
  className?: string;
  /** Whether a message over {@link FOLD_LINES} lines is folded. */
  fold?: boolean;
  /** Its fold's button as a stop of the roving focus it stands in (the Inbox's list), where
   *  it stands in one. */
  foldStop?: (button: ReactElement) => ReactElement;
}) {
  const own = useId();
  const box = id ?? `${own}-text`;
  const body = useRef<HTMLDivElement>(null);
  const lines = useDrawnLines(body, text);
  const [open, setOpen] = useState(false);
  const folds = fold && lines > FOLD_LINES;
  return (
    <div className={className === undefined ? "chat-text" : `chat-text ${className}`}>
      <div
        ref={body}
        id={box}
        className="chat-text-body"
        data-folded={folds ? String(!open) : undefined}
      >
        <Folded.Provider value={folds && !open}>
          <Markdown skipHtml remarkPlugins={REMARK} components={COMPONENTS}>
            {text}
          </Markdown>
        </Folded.Provider>
      </div>
      {folds &&
        foldStop(
          <Button
            variant="link"
            size="sm"
            className="chat-text-fold"
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
 * **How many lines the words take as drawn**: its height over its line height, measured as it
 * is laid out and again whenever its width changes, so a paragraph that wraps counts each line
 * it wraps to. Where nothing is laid out (a test's page), the lines as written.
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

/** Its whole height, folded or not, over its line height: 0 where it is not laid out. */
function drawnLines(element: HTMLElement): number {
  const style = getComputedStyle(element);
  const size = Number.parseFloat(style.fontSize);
  const line = Number.parseFloat(style.lineHeight);
  const height = Number.isFinite(line) ? line : size * 1.2;
  if (!(height > 0) || element.scrollHeight === 0) return 0;
  return Math.round(element.scrollHeight / height);
}

/** GitHub's markdown, and a line break where the chat broke its line: a chat writes for a
 *  terminal, where a new line is a new line. */
const REMARK = [remarkGfm, remarkBreaks];

/** A heading drawn as bold words in a paragraph: never larger, never one of the page's. */
const heading: Components["h1"] = ({ children }) => (
  <p className="chat-text-heading">
    <strong>{children}</strong>
  </p>
);

/** The host a link goes to, or `undefined` where it is not a web address. */
function hostOf(href: string | undefined): string | undefined {
  if (href === undefined || !/^https?:\/\//i.test(href)) return undefined;
  try {
    return new URL(href).host;
  } catch {
    return undefined;
  }
}

/** Whether a link's words are its own address, which already says where it goes. */
function saysItsAddress(children: ReactNode, href: string): boolean {
  return typeof children === "string" && (children === href || href.endsWith(`//${children}`));
}

/** Whether the words are folded: a link in them is then out of the Tab order, since it may be
 *  cut out of sight, until Show all opens them. It is still pressed, and read by a screen
 *  reader, which has the whole text. */
const Folded = createContext(false);

/** A link: where it goes beside its words, and opened only on a press. */
function Link({ href, children }: { href?: string; children?: ReactNode }) {
  const folded = useContext(Folded);
  const host = hostOf(href);
  return (
    <>
      <ExternalLink href={host === undefined ? undefined : href} reachable={!folded}>
        {children}
      </ExternalLink>
      {host !== undefined && href !== undefined && !saysItsAddress(children, href) && (
        <>
          {" "}
          <span className="chat-text-host">({host})</span>
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
  img: ({ alt }) => <span className="chat-text-picture">{alt || "Picture not shown"}</span>,
  a: ({ href, children }) => <Link href={href}>{children}</Link>,
};
