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
  ".tab .closer",
  ".project .closer",
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
  // A menu's row that cannot run (#2909), and an option of a select's list that cannot be
  // chosen.
  '[data-scope="menu"][data-part="root"][data-disabled]',
  '[data-scope="menu"][data-part="root"][data-unavailable]',
  ".ui-menu-item[data-disabled]",
]);

/** Informational words, each drawn in quiet: line numbers, placeholders, counts, paths, group
 *  labels and the meta role (an age, a kind, a count, a shortcut). */
const QUIET: readonly string[] = [
  ".ui-text-meta",
  ".ui-select[data-placeholder]",
  ".pane-find-count",
  ".ui-level-count",
  // A status cell's word before its count (#2913).
  '[data-static]:has(> [data-part="count"]) > [data-scope="status-item"][data-part="label"]',
  ".panel-day",
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
