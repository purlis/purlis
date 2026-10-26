// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Informational text is quiet, never faint** (#2902, ruling #2803·2).
 *
 * The engine holds `text.faint` at 3:1, the floor of a mark, so the contrast check cannot tell a
 * path drawn in faint from a tab's × drawn in it: both clear. This is the line it cannot draw.
 * Faint is for what is not words — a tab's ×, a separator, a pin, a chevron — and for disabled
 * words, so every rule that draws in `--text-faint` is one of {@link FAINT}, and the window's
 * informational words ({@link QUIET}) are drawn in `--text-quiet`.
 */

/// <reference types="node" />
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(process.cwd(), "src");

/** The rules that may draw in faint: each is a mark that is not words (or disabled words). */
const FAINT: ReadonlySet<string> = new Set([
  // A session tab's `×`, a mark (`ui/tabs/tabs.css`, #2914).
  '[data-scope="tabs"][data-part="tab"] [data-part="closer"][data-scope="tabs"]',
  ".pinned",
  ".pane-crumbs .crumb-sep",
  ".tab-task-sep",
  ".ui-tree-twist",
  // A disabled button's words (#2897's palette rules, #2904), and a disabled status item's.
  '[data-scope="button"][data-part="root"][data-disabled]',
  // An unavailable box's words (#2905).
  '[data-scope="text-input"][data-part="root"][data-disabled]',
  '[data-scope="textarea"][data-part="root"][data-disabled]',
  '[data-scope="status-item"][data-part="root"][data-disabled]',
  // An empty state's mark, above its words (#2913).
  '[data-scope="empty-state"][data-part="icon"]',
  // The slash between the title bar's project and its pinned workspaces, a mark (#2916, R156).
  '[data-part="slash"][data-scope="titlebar"]',
  // A switcher row's pin, a mark (R107, R156), and a row that cannot run, the disabled tier.
  '[data-part="pin"][data-scope="titlebar"]',
  '[data-part="row"][data-scope="titlebar"][data-unavailable]',
  // A menu's row that cannot run (#2909), and an option of a select's list that cannot be
  // chosen, which is a menu's row (#2906).
  '[data-scope="menu"][data-part="root"][data-disabled]',
  '[data-scope="menu"][data-part="root"][data-unavailable]',
  // A choice's marks (#2907): the edge of a box and of a radio's circle at rest, and every
  // choice's words and marks while it cannot be used.
  '[data-scope="checkbox"][data-part="box"]',
  '[data-scope="radio"][data-part="mark"]',
  '[data-scope="checkbox"][data-part="root"][data-disabled]',
  '[data-scope="radio"][data-part="root"][data-disabled]',
  '[data-scope="switch"][data-part="root"][data-disabled]',
  '[data-scope="checkbox"][data-part="root"][data-variant="chip"][data-disabled]',
  '[data-scope="checkbox"][data-part="root"][data-disabled][data-selected] [data-scope="checkbox"][data-part="box"]',
  '[data-scope="checkbox"][data-part="root"][data-disabled][data-indeterminate] [data-scope="checkbox"][data-part="box"]',
  '[data-scope="radio"][data-part="root"][data-disabled][data-selected] [data-scope="radio"][data-part="mark"]',
  '[data-scope="switch"][data-part="root"][data-disabled] [data-scope="switch"][data-part="thumb"]',
  '[data-scope="switch"][data-part="root"][data-disabled][data-selected] [data-scope="switch"][data-part="track"]',
  '[data-scope="segment"][data-part="root"][data-disabled]',
  // The chosen segment's 1px edge under it, a mark (R39).
  '[data-scope="segment"][data-part="root"][data-selected]',
  '[data-scope="slider"][data-part="root"][data-disabled]',
  '[data-scope="slider"][data-part="root"][data-disabled] [data-scope="slider"][data-part="fill"]',
  '[data-scope="slider"][data-part="root"][data-disabled] [data-scope="slider"][data-part="thumb"]',
  '[data-scope="option-card"][data-part="root"][data-disabled]',
  '[data-scope="option-card"][data-part="root"][data-variant="tile"][data-disabled]',
  // A list's row that cannot be used (#2911).
  '[data-scope="list"][data-part="root"][data-disabled]',
  '[data-scope="list"][data-part="root"][data-unavailable]',
  // A tree's row that cannot be used (#2911).
  '[data-scope="tree"][data-part="root"][data-disabled]',
  // An unavailable select's or combobox's words and marks (#2906).
  '[data-scope="select"][data-part="root"][data-disabled]',
  '[data-scope="combobox"][data-part="root"][data-disabled]',
  '[data-scope="select"][data-part="root"][data-disabled] [data-part="chevron"]',
  '[data-scope="select"][data-part="root"][data-disabled] [data-part="placeholder"]',
  '[data-scope="combobox"][data-part="root"][data-disabled] [data-part="chevron"]',
]);

/** Informational words, each drawn in quiet: line numbers, placeholders, counts, paths, group
 *  labels and the meta role (an age, a kind, a count, a shortcut). */
const QUIET: readonly string[] = [
  ".ui-text-meta",
  '[data-scope="select"][data-part="placeholder"]',
  ".pane-find-count",
  // A choice's line under its words, a card's detail and a segment at rest (#2907).
  '[data-scope="checkbox"][data-part="description"]',
  '[data-scope="radio"][data-part="description"]',
  '[data-scope="option-card"][data-part="detail"]',
  '[data-scope="segment"][data-part="root"]',
  // A status cell's word before its count (#2913).
  '[data-static]:has(> [data-part="count"]) > [data-scope="status-item"][data-part="label"]',
  // A list's group name (#2911): a day, a file.
  '[data-scope="list"][data-part="header"]',
  ".search-file-folder",
  ".search-file-rest",
  ".search-line-number",
  ".search-summary",
  ".ui-machine-path",
  ".activity-at",
  ".memory-meta",
];

/** Every stylesheet and source file of the window's own, tests aside. */
function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return files(path);
    return /\.(css|ts|tsx)$/.test(name) && !/\.test\.|\.testkit\./.test(name) ? [path] : [];
  });
}

const read = (path: string) => readFileSync(path, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** Each innermost rule of `css`: its selectors, one by one, and its declarations. */
function rules(css: string): { selectors: string[]; body: string }[] {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(([, head, body]) => ({
    selectors: (head ?? "")
      .split(",")
      .map((one) => one.replace(/\s+/g, " ").trim())
      .filter(Boolean),
    body: body ?? "",
  }));
}

const sheets = files(SRC).filter((path) => path.endsWith(".css"));
const sources = files(SRC).filter((path) => !path.endsWith(".css"));

describe("the faint tier", () => {
  it("is drawn only by the rules for marks", () => {
    const drawnFaint = sheets.flatMap((path) =>
      rules(read(path))
        .filter(({ body }) => /var\(--text-faint\)/.test(body))
        .flatMap(({ selectors }) => selectors.map((one) => `${relative(SRC, path)} ${one}`)),
    );
    // The Tailwind theme names it for a class; it draws nothing itself.
    const off = drawnFaint.filter(
      (one) => !one.startsWith("styles.css") && !FAINT.has(one.slice(one.indexOf(" ") + 1)),
    );
    expect(off, "words a person reads take quiet, not faint").toEqual([]);
    expect(drawnFaint.length).toBeGreaterThanOrEqual(FAINT.size);
  });

  it("is named by no component or script, so it cannot reach words around the sheets", () => {
    const named = sources
      .filter((path) => /--text-faint|text-text-faint/.test(read(path)))
      .map((path) => relative(SRC, path));
    expect(named).toEqual([]);
  });
});

describe("informational text", () => {
  const all = sheets.flatMap((path) => rules(read(path)));
  it.each(QUIET)("%s is drawn in quiet", (selector) => {
    const own = all.filter(({ selectors }) => selectors.includes(selector));
    const colours = own.flatMap(({ body }) =>
      [...body.matchAll(/(?:^|[;\s])color:\s*([^;]+);/g)].map((hit) => hit[1]?.trim()),
    );
    expect(own.length, selector).toBeGreaterThan(0);
    expect(colours, selector).toContain("var(--text-quiet)");
    expect(colours, selector).not.toContain("var(--text-faint)");
  });

  it("is drawn in quiet in the editor's line numbers", () => {
    const look = read(join(SRC, "editor", "look.ts"));
    expect(look).toMatch(/"\.cm-gutters":\s*\{\s*color:\s*"var\(--text-quiet\)"/);
  });
});
