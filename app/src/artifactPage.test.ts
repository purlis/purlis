// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it, vi } from "vitest";

/**
 * **purlis's own script in every artifact page** (`src-tauri/src/artifact-page.js`, #5952,
 * #5955), run here as the platform runs it at the start of a load: against a page of its own,
 * whose window, storage and scroll are this test's. The webview itself is out of WebDriver's
 * reach (`e2e/specs/artifacts.e2e.ts`), so what the script does is proved here.
 */
const SCRIPT = readFileSync(join(process.cwd(), "src-tauri", "src", "artifact-page.js"), "utf8");

/** What one load of a page is: its window's events, its document, and where it is scrolled. */
type Load = {
  window: EventTarget & { __purlisTheme?: () => void };
  document: Document;
  asked: string[];
  scrolledTo: ReturnType<typeof vi.fn>;
};

/** One load of a page, `storage` its session storage, which a reload keeps; scrolled to `at`. */
function load(storage: Storage, at: [number, number] = [0, 0], parsed = ""): Load {
  const page = new EventTarget() as Load["window"];
  const document = window.document.implementation.createHTMLDocument("");
  // At the start of a load, the root and nothing in it yet; or what a test says was parsed.
  document.documentElement.innerHTML = parsed;
  const asked: string[] = [];
  class Image {
    set src(url: string) {
      asked.push(url);
    }
  }
  const scrolledTo = vi.fn();
  new Function(
    "window",
    "addEventListener",
    "document",
    "sessionStorage",
    "scrollX",
    "scrollY",
    "scrollTo",
    "Image",
    SCRIPT,
  )(page, page.addEventListener.bind(page), document, storage, at[0], at[1], scrolledTo, Image);
  return { window: page, document, asked, scrolledTo };
}

/** A session's storage, as the webview keeps it from one load to the next. */
function sessionStorage(): Storage {
  const held = new Map<string, string>();
  return {
    get length() {
      return held.size;
    },
    clear: () => held.clear(),
    getItem: (key) => held.get(key) ?? null,
    key: (n) => [...held.keys()][n] ?? null,
    removeItem: (key) => void held.delete(key),
    setItem: (key, value) => void held.set(key, String(value)),
  };
}

describe("purlis's script in an artifact's page", () => {
  it("links purlis's theme before anything of the page's, so the page's own rules win", () => {
    const { document } = load(sessionStorage());
    document.documentElement.insertAdjacentHTML(
      "beforeend",
      "<head><style>:root { --purlis-surface-base: red }</style></head>",
    );
    const first = document.documentElement.firstElementChild;
    expect(first?.tagName).toBe("LINK");
    expect(first?.getAttribute("rel")).toBe("stylesheet");
    expect(first?.getAttribute("href")).toBe("/.purlis/theme.css");
    expect(document.querySelectorAll("link")).toHaveLength(1);
  });

  it("links it first even where the page's head came before it", () => {
    const { document } = load(
      sessionStorage(),
      [0, 0],
      "<head><style></style></head><body></body>",
    );
    expect(document.documentElement.firstElementChild?.getAttribute("href")).toBe(
      "/.purlis/theme.css",
    );
  });

  it("asks for the theme again, from its own origin, when purlis says it changed", () => {
    const { window: page, document } = load(sessionStorage());
    page.__purlisTheme?.();
    const href = document.querySelector("link")?.getAttribute("href") ?? "";
    expect(href).toMatch(/^\/\.purlis\/theme\.css\?\d+$/);
  });

  it("keeps where the page was scrolled across a reload", () => {
    const storage = sessionStorage();
    const before = load(storage, [0, 640]);
    before.window.dispatchEvent(new Event("pagehide"));

    const after = load(storage);
    expect(after.scrolledTo).not.toHaveBeenCalled();
    after.window.dispatchEvent(new Event("load"));
    expect(after.scrolledTo).toHaveBeenCalledWith(0, 640);

    // Once: a load after that starts where it is put, not where an old one was.
    const again = load(storage);
    again.window.dispatchEvent(new Event("load"));
    expect(again.scrolledTo).not.toHaveBeenCalled();
  });

  it("scrolls nowhere for a kept place that is not two numbers", () => {
    for (const kept of ["[1]", '["a", 2]', "{}", "not json", "[1e999, 0]"]) {
      const storage = sessionStorage();
      storage.setItem("purlis:scroll", kept);
      const page = load(storage);
      page.window.dispatchEvent(new Event("load"));
      expect(page.scrolledTo, kept).not.toHaveBeenCalled();
    }
  });

  it("hands Esc and F6 to purlis by asking its own origin, and no other key", () => {
    const page = load(sessionStorage());
    for (const [key, shiftKey] of [
      ["Escape", false],
      ["F6", false],
      ["F6", true],
      ["Enter", false],
    ] as const)
      page.window.dispatchEvent(new KeyboardEvent("keydown", { key, shiftKey }));
    expect(page.asked).toEqual(["/.purlis/key/escape", "/.purlis/key/f6", "/.purlis/key/f6-back"]);
  });
});
