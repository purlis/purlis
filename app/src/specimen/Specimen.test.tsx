// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { existsSync, readFileSync } from "node:fs";
import { join, posix } from "node:path";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { DEFAULT_THEME } from "../theme/theme";
import { Specimen, type Drawing } from "./Specimen";
import { CHAT_LINES, EDITOR_TEXT, TERMINAL_LINES } from "./fixture";
import { FAILED, GIVEN, READY, drawn, failed, given } from "./page";
import { PURLIS_LIGHT } from "../theme/handed.testkit";
import { selectNamed } from "../ui/select/select.testkit";

/**
 * **The theme specimen** (#2330): what it draws, and that it draws nothing of the person's.
 *
 * The picture itself is the scenario test's (`e2e/specs/theme-specimen.e2e.ts`): jsdom paints
 * nothing. Here, the page's content and its one way out to the core, which it never takes.
 */

/** Every command the page asks the core for, through Tauri's IPC. It asks none. */
const asked: string[] = [];
vi.mock("@tauri-apps/api/core", async (real) => {
  const tauri = await real<typeof import("@tauri-apps/api/core")>();
  return {
    ...tauri,
    invoke: (command: string) => {
      asked.push(command);
      return Promise.reject(new Error("the specimen's window may invoke nothing"));
    },
  };
});

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
beforeAll(() => {
  globalThis.matchMedia ??= ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  })) as typeof matchMedia;
});

afterEach(() => {
  cleanup();
  asked.length = 0;
});

/** The specimen drawn, and what it waited on before it would say so. */
async function drawnSpecimen(theme = DEFAULT_THEME) {
  const writing: Promise<void>[] = [];
  const drawing: Drawing = { wait: (written) => writing.push(written) };
  const view = render(<Specimen theme={theme} drawing={drawing} />);
  await Promise.all(writing);
  return { view, writing };
}

/** A terminal's lines as a person reads them: without their colour codes. */
const plain = (lines: readonly string[]) =>
  // eslint-disable-next-line no-control-regex
  lines.map((line) => line.replace(/\u001b\[[\d;?]*[a-zA-Z]/g, ""));

describe("the theme specimen", () => {
  it("draws a chat, a terminal, an editor, a Settings page, a dialog and a Notice", async () => {
    await drawnSpecimen();
    const page = screen.getByRole("main", { name: "Theme specimen" });
    for (const region of ["Chat", "Terminal", "Editor", "Settings", "Dialog", "Notice"])
      expect(within(page).getByRole("region", { name: region })).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: /Delete the branch/ })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "sample.rs" })).toBeInTheDocument();
    expect(
      within(screen.getByRole("region", { name: "Notice" })).getByText(/could not be saved/),
    ).toBeInTheDocument();
  });

  it("draws every kind of control the Settings components have", async () => {
    await drawnSpecimen();
    const settings = screen.getByRole("region", { name: "Settings" });
    expect(within(settings).getByRole("textbox", { name: "Name" })).toBeInTheDocument();
    expect(within(settings).getByRole("slider", { name: "Text size" })).toBeInTheDocument();
    expect(within(settings).getByRole("radiogroup", { name: "Density" })).toBeInTheDocument();
    expect(selectNamed("Appearance", settings)).toBeInTheDocument();
    expect(within(settings).getByRole("switch", { name: "Reduce motion" })).toBeInTheDocument();
    expect(
      within(settings).getByRole("checkbox", { name: "Show line numbers" }),
    ).toBeInTheDocument();
    expect(within(settings).getByRole("group", { name: "Panels" })).toBeInTheDocument();
    expect(within(settings).getByRole("group", { name: "Show only" })).toBeInTheDocument();
    // The primary with its key, so a theme's `button.shortcut` is drawn too (#3510).
    const key = within(settings)
      .getByRole("button", { name: "Save" })
      .querySelector('[data-scope="button"][data-part="shortcut"]');
    expect(key).toBeInTheDocument();
    // Drawn, not only its room kept.
    expect(key).not.toHaveAttribute("data-off");
  });

  it("waits on each terminal's lines before it may say it is drawn", async () => {
    const { writing } = await drawnSpecimen();
    expect(writing).toHaveLength(2);
  });

  it("holds every ANSI colour, plain and bright, in its terminal", () => {
    const codes = TERMINAL_LINES.join("\n");
    for (let n = 0; n < 8; n++) {
      for (const code of [30 + n, 90 + n, 40 + n, 100 + n])
        expect(codes, `colour ${code}`).toContain(`\u001b[${code}m`);
    }
  });

  it("draws in the theme it is handed, each terminal from the theme's own tokens", async () => {
    const light = PURLIS_LIGHT;
    const { view } = await drawnSpecimen(light);
    const terminals = view.container.querySelectorAll(".specimen-terminal .xterm");
    expect(terminals).toHaveLength(2);
  });
});

describe("the theme specimen shows nothing of the person's", () => {
  /** A string no fixture holds, planted wherever the window keeps something of the person's. */
  const CANARY = "canary-7f3a-their-secret-project";

  it("asks the core for nothing, and draws none of what the window holds", async () => {
    // Planted where the window's own state lives: what the core hands it at creation, its
    // storage, its title. The specimen is handed none of it in its own window; this proves that,
    // even in a page that had all of it, nothing it draws comes from there.
    const held = globalThis as Record<string, unknown>;
    const atCreation = held.__CHARTER_AT_CREATION__ as Record<string, unknown>;
    const before = { ...atCreation };
    Object.assign(atCreation, {
      layout: { projects: [CANARY], chats: [{ title: CANARY, cwd: `/home/${CANARY}` }] },
      theirs: { path: `/home/${CANARY}/theme.json` },
    });
    globalThis.localStorage.setItem("purlis.layout", CANARY);
    globalThis.sessionStorage.setItem("purlis.folds", CANARY);
    document.title = CANARY;
    try {
      const { view } = await drawnSpecimen();
      expect(view.container.textContent).not.toContain(CANARY);
      expect(view.container.innerHTML).not.toContain(CANARY);
      expect(asked).toEqual([]);
    } finally {
      for (const key of Object.keys(atCreation)) Reflect.deleteProperty(atCreation, key);
      Object.assign(atCreation, before);
      globalThis.localStorage.clear();
    }
  });

  it("writes only its fixture into its terminals and its editor", async () => {
    const { view } = await drawnSpecimen();
    const editor = view.container.querySelector(".specimen-editor .cm-content");
    expect(editor?.textContent).toBe(EDITOR_TEXT.split("\n").join(""));
    // xterm keeps each row it drew; every non-blank row is a fixture line.
    const rows = [...view.container.querySelectorAll(".xterm-rows > div")]
      .map((row) => row.textContent?.replace(/\u00a0/g, " ").trimEnd() ?? "")
      .filter((row) => row !== "");
    const fixture = new Set(
      [...plain(CHAT_LINES), ...plain(TERMINAL_LINES)].map((l) => l.trimEnd()),
    );
    for (const row of rows) expect(fixture, row).toContain(row);
  });

  it("imports nothing that reads the person's state or the core, however deep", () => {
    // Every module the page loads, followed through imports of imports from its entry: none is
    // the generated bindings, the UI RPC, the window's prefs, its actions or the project in
    // front, and none loads Tauri's API. (Its window is granted no command either: `ipc.rs`,
    // the theme specimen's window may invoke nothing.)
    const loaded = loadedFrom("src/specimen/main.tsx");
    expect(loaded.size, "the walk found the page's modules").toBeGreaterThan(10);
    const reached = [...loaded].filter(
      (file) => FORBIDDEN.includes(file) || readFileSync(file, "utf8").match(TAURI_API) !== null,
    );
    expect(reached).toEqual([]);
    // And the page's own files keep away from the window's storage.
    for (const file of [...loaded].filter((one) => one.startsWith("src/specimen/"))) {
      const source = readFileSync(file, "utf8");
      for (const storage of ["localStorage", "sessionStorage"])
        expect(source, `${file} names ${storage}`).not.toContain(storage);
    }
  });
});

/** The modules that read the person's state or ask the core for anything. */
const FORBIDDEN = [
  "src/bindings.ts",
  "src/uiRpc.ts",
  "src/windowprefs.ts",
  "src/here.ts",
  "src/actions.ts",
  "src/textSize.ts",
];

/** A module that loads Tauri's API at all. */
const TAURI_API = /^\s*(?:import|export)\s(?!type\s)[^;]*?from\s+["']@tauri-apps\/api/m;

/**
 * Every module `entry` loads, by path from `app`: its static imports and re-exports followed to
 * the end, leaving out `import type`, which loads nothing.
 */
function loadedFrom(entry: string): Set<string> {
  const loaded = new Set<string>();
  const next = [entry];
  const edge = /^\s*(?:import|export)\s+(?!type\s)(?:[^;]*?\sfrom\s+)?["'](\.{1,2}\/[^"']+)["']/gm;
  while (next.length > 0) {
    const file = next.pop() as string;
    if (loaded.has(file)) continue;
    loaded.add(file);
    for (const [, spec] of readFileSync(file, "utf8").matchAll(edge)) {
      const base = posix.join(posix.dirname(file), spec);
      const found = ["", ".ts", ".tsx", "/index.ts", "/index.tsx"]
        .map((end) => base + end)
        .find((one) => /\.tsx?$/.test(one) && existsSync(one));
      if (found !== undefined) next.push(found);
    }
  }
  return loaded;
}

describe("the specimen page's contract with the app", () => {
  it("reads what it is handed, and says it is drawn or why not in its title", () => {
    const held = globalThis as Record<string, unknown>;
    expect(() => given()).toThrow(/without what it draws/);
    held[GIVEN] = { theme: DEFAULT_THEME, width: 1280, height: 800 };
    try {
      expect(given().width).toBe(1280);
      drawn();
      expect(document.title).toBe(READY);
      failed(new Error("no fonts"));
      expect(document.title).toBe(`${FAILED}no fonts`);
    } finally {
      Reflect.deleteProperty(held, GIVEN);
    }
  });

  it("speaks the words the app waits for", () => {
    const app = readFileSync(join(process.cwd(), "src-tauri", "src", "specimen.rs"), "utf8");
    expect(app).toContain(`pub const READY: &str = "${READY}";`);
    expect(app).toContain(`pub const FAILED: &str = "${FAILED}";`);
    expect(app).toContain(`pub const GIVEN: &str = "${GIVEN}";`);
  });
});
