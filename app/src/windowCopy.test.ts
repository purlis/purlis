// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it, vi } from "vitest";
import { copyFaults, refusedWord, retiredTerms } from "./copy";
import { onPool, onThisOs, windowCopy } from "./windowCopy";

// `counted` as it is, watched: W5's and W123's counts go through it (`counted_rows_use_counted`).
vi.mock("./dispatches", async (original) => {
  const real = await original<typeof import("./dispatches")>();
  return { ...real, counted: vi.fn(real.counted) };
});

/**
 * **Phase 1a's Window copy table and the copy module that holds it** (T1a-75, #6755). The table
 * is `windowCopy.table.md`, spec.md's table as the operator approved it; the window's half is
 * `windowCopy.ts`, and core's is `crates/purlis-core/src/window_copy.rs`, read from its source as
 * `copy.rust.test.ts` reads the Rust places it names.
 */
const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "../..");
const TABLE = readFileSync(resolve(HERE, "windowCopy.table.md"), "utf8");
const RUST = readFileSync(resolve(ROOT, "crates/purlis-core/src/window_copy.rs"), "utf8");

type Row = { w: string; string: string; kind: string; place: string };

/** The table's rows: from its header to the next heading, one per line that starts `| W`. */
function rowsOf(table: string): Row[] {
  const lines = table.split("\n");
  const from = lines.findIndex((line) => line.startsWith("| # | String |"));
  const to = lines.findIndex((line, at) => at > from && line.startsWith("#"));
  return lines
    .slice(from + 2, to < 0 ? undefined : to)
    .filter((line) => line.startsWith("| W"))
    .map((line) => {
      const cells = line
        .slice(2, -2)
        .split(/(?<!\\)\|/)
        .map((cell) => cell.trim());
      return { w: cells[0], string: cells[1], kind: cells[2], place: cells[3] };
    });
}

/**
 * The forms a row's String cell holds, word for word: each quoted line (`*"…"*`), but one it
 * only completes (`completes *"Asked you …"*`), else its code span (W3). A choice written in
 * the slot (`<1Password \| HashiCorp Vault>`) is a form per choice.
 */
function formsOf(cell: string): string[] {
  const quoted = [...cell.matchAll(/\*"([^"]*)"\*/g)]
    .filter((one) => !/completes $/.test(cell.slice(0, one.index)))
    .map((one) => one[1]);
  const forms = quoted.length > 0 ? quoted : [/`([^`]+)`/.exec(cell)?.[1] ?? ""];
  return forms.flatMap((form) => {
    const choice = /<([^<>]*\\\|[^<>]*)>/.exec(form.replace(/\\\|/g, "\\|"));
    if (choice === null) return [form];
    return choice[1].split(/\s*\\\|\s*/).map((one) => form.replace(choice[0], one));
  });
}

/** A table form matches a module form: word for word, or, elided (`… to use <chat>.`), within. */
function matches(table: string, module: string): boolean {
  return table.startsWith("… ") ? module.includes(table.slice(2)) : table === module;
}

/**
 * Which half holds a row, by its Kind and Place: a line only core says (at the turn's end,
 * printed by Claude Code, the Blocked log); a line core writes and the window draws (every
 * other Refusal and Notice, and an Ask in Needs you); and every other line, the window's alone.
 */
type Half = "core" | "both" | "window";
function halfOf({ kind, place }: Row): Half {
  if (/^(the turn's end(?! *;)|printed by Claude Code|the Blocked log)/.test(place)) return "core";
  if (kind === "Refusal" || kind === "Notice") return "both";
  if (kind === "Ask" && place.includes("Needs you")) return "both";
  return "window";
}

/** `whatWouldWork` as the table writes its slot: `<what would work>`. */
const slotted = (key: string) => `<${key.replace(/[A-Z]/g, (c) => ` ${c.toLowerCase()}`)}>`;

/** Slots filled with their own placeholders, so a line reads as the table writes it. */
const placeholders = (count?: number) =>
  new Proxy({} as Record<string, unknown>, {
    get: (_, key) => (key === "count" ? count : typeof key === "string" ? slotted(key) : undefined),
  });

/** The rows whose line is counted: their function takes `count`, read at 2, 1 and 0. */
const COUNTED = new Set(
  rowsOf(TABLE)
    .filter((row) => row.string.startsWith("through `counted`"))
    .map((row) => row.w),
);

/** A module line's forms as `[key, text]`, its slots as placeholders: `W16.later`. */
function rendered(key: string, value: unknown): [string, string][] {
  if (typeof value === "string") return [[key, value]];
  if (typeof value === "function") {
    const w = key.split(".")[0];
    const counts = COUNTED.has(w) ? [2, 1, 0] : [undefined];
    return counts.map((n) => [key, value(placeholders(n)) as string]);
  }
  return Object.entries(value as object).flatMap(([form, inner]) =>
    rendered(`${key}.${form}`, inner),
  );
}

const WINDOW: [string, string][] = Object.entries(windowCopy).flatMap(([w, value]) =>
  rendered(w, value),
);

/** Core's `pub const`s: name and text. */
const CORE: [string, string][] = [
  ...RUST.matchAll(/pub const (W\w+): &str =\s*"((?:[^"\\]|\\.)*)";/g),
].map((one) => [one[1], JSON.parse(`"${one[2]}"`) as string]);

/** The Rust name of a window form: `W16.later` is `W16_LATER`, `W26a` is `W26A`. */
const rustName = (key: string) =>
  key
    .replace(/[A-Z]/g, (c) => `_${c}`)
    .replace(".", "_")
    .replace(/^_?W/, "W")
    .replace(/__/g, "_")
    .toUpperCase();

/** The W-number a module key or a Rust name is for. */
function wOf(key: string): string {
  const [, number, letter] = /^W(\d+)([a-z]?)/i.exec(key) ?? [key, key, ""];
  return `W${number}${letter.toLowerCase()}`;
}

const forms = (all: [string, string][], w: string) =>
  all.filter(([key]) => wOf(key) === w).map(([, text]) => text);

describe("the copy module (T1a-75)", () => {
  const rows = rowsOf(TABLE);

  it("reads the whole table", () => {
    // A reading that lost rows would pass by having fewer to hold.
    expect(rows.length).toBeGreaterThanOrEqual(135);
    expect(rows.at(-1)?.w).toBe("W140");
  });

  it("every_w_string_is_in_the_copy_module", () => {
    const wrong: string[] = [];
    for (const row of rows) {
      const half = halfOf(row);
      const table = formsOf(row.string);
      const held = [
        ...(half === "core" ? [] : [["window", forms(WINDOW, row.w)] as const]),
        ...(half === "window" ? [] : [["core", forms(CORE, row.w)] as const]),
      ];
      for (const [where, module] of held) {
        if (module.length === 0) wrong.push(`${row.w}: not in the ${where}'s half`);
        for (const one of table)
          if (!module.some((text) => matches(one, text)))
            wrong.push(`${row.w}: the ${where}'s half lacks ${JSON.stringify(one)}`);
        for (const text of module)
          if (!table.some((one) => matches(one, text)))
            wrong.push(`${row.w}: the ${where}'s half says ${JSON.stringify(text)}, not the table`);
      }
      if (half === "core" && forms(WINDOW, row.w).length > 0)
        wrong.push(`${row.w}: only core says it, so the window's half holds no copy`);
      if (half === "window" && forms(CORE, row.w).length > 0)
        wrong.push(`${row.w}: only the window says it, so core's half holds no copy`);
    }
    const known = new Set(rows.map((row) => row.w));
    for (const [key] of [...WINDOW, ...CORE])
      if (!known.has(wOf(key))) wrong.push(`${key}: no row of the table`);
    expect(wrong).toEqual([]);
  });

  it("the_rust_table_matches_the_window_half", () => {
    const window = new Map(WINDOW.map(([key, text]) => [rustName(key), text]));
    const differ = CORE.filter(([name]) => window.has(name))
      .filter(([name, text]) => window.get(name) !== text)
      .map(([name]) => name);
    expect(differ).toEqual([]);
    // Every line core writes and the window draws is under one name in both halves.
    const both = new Set(rows.filter((row) => halfOf(row) === "both").map((row) => row.w));
    const unpaired = CORE.filter(([name]) => both.has(wOf(name)) && !window.has(name)).map(
      ([name]) => name,
    );
    expect(unpaired).toEqual([]);
  });

  it("holds every W-string to the copy guide's rules, as the window shows it", () => {
    const faults = [...WINDOW, ...CORE].flatMap(([key, text]) =>
      [
        ...copyFaults(text, "shown"),
        ...retiredTerms(text, "shown"),
        ...refusedWord(text, "shown"),
      ].map((fault) => `${key} ${JSON.stringify(text)}: ${fault}`),
    );
    expect(faults).toEqual([]);
  });
});

describe("the table-wide rules, each a function of the module", () => {
  it("computer_reads_mac_on_macos", () => {
    const line = windowCopy.W21.here;
    expect(onThisOs(line, "macos")).toBe(
      "Anthropic's certificate isn't trusted on this Mac. purlis uses this Mac's proxy settings and trusted certificates.",
    );
    expect(onThisOs(line, "linux")).toBe(line);
    expect(onThisOs(line, "windows")).toBe(line);
  });

  it("your_keychain_reads_your_system_keychain_off_macos", () => {
    expect(onThisOs(windowCopy.W10, "linux")).toBe(
      "Your system keychain is locked. Unlock it to start this chat.",
    );
    expect(onThisOs(windowCopy.W6, "windows")).toBe(
      "This key can't be read. Unlock your system keychain to use it.",
    );
    expect(onThisOs(windowCopy.W10, "macos")).toBe(windowCopy.W10);
  });

  it("a_runner_line_names_its_pool_after_the_thing_it_names", () => {
    expect(windowCopy.W26a({ lock: onPool("The model lock", "gpu") })).toBe(
      "The model lock on gpu now blocks this chat's requests.",
    );
    expect(windowCopy.W26a({ lock: onPool("The model lock", null) })).toBe(
      "The model lock now blocks this chat's requests.",
    );
    expect(windowCopy.W64({ file: onPool("policy.toml", "gpu"), error: "it isn't TOML" })).toBe(
      "policy.toml on gpu can't be read: it isn't TOML. Chats on a key can't start until it's fixed.",
    );
  });
});

describe("counted rows", () => {
  it("counted_rows_use_counted", async () => {
    // test-setup.ts reaches `windowCopy` (through `tabs` and `chatLog`) before this file's mock
    // applies, so the module imported above holds the real `counted`. A fresh graph loads both
    // after the mock, and the watched `counted` is the one W5 and W123 call.
    vi.resetModules();
    const { counted } = await import("./dispatches");
    const { windowCopy } = await import("./windowCopy");
    const said = (count: number) => windowCopy.W123({ count, pool: "gpu" });
    vi.mocked(counted).mockClear();
    expect(said(2)).toBe(
      "Stop sending this key to gpu? 2 chats there stop at their next request, and their conversations are kept.",
    );
    expect(said(1)).toBe(
      "Stop sending this key to gpu? 1 chat there stops at its next request, and its conversation is kept.",
    );
    expect(said(0)).toBe("Stop sending this key to gpu? No chats there use it.");
    expect(vi.mocked(counted).mock.calls).toEqual([
      [2, "chat", "chats"],
      [1, "chat", "chats"],
    ]);
    vi.mocked(counted).mockClear();
    expect(windowCopy.W5({ count: 2 })).toMatch(/^2 chats use this key\./);
    expect(vi.mocked(counted).mock.calls).toEqual([[2, "chat", "chats"]]);
  });
});
