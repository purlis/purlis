// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The mock of a page's port (`purlis artifact mock`, #5975), run as a page outside purlis runs
// it: in a document of its own, before or after the page's declaration, with its fixtures and
// data files fetched from beside the page.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { MessageChannel, MessagePort } from "node:worker_threads";
import { JSDOM } from "jsdom";
import { afterEach, describe, expect, it, vi } from "vitest";

// Kept in the core, which compiles it into `purlis artifact mock` (`artifact::kit`), and read as
// text: outside the app's root, so not through Vite's `?raw`.
const MOCK = readFileSync(
  join(process.cwd(), "..", "crates", "purlis-core", "artifact-kit", "purlis-page-mock.js"),
  "utf8",
);

type Answer = {
  id: number | null;
  data?: unknown;
  error?: { kind: string; message?: string };
  at: string;
  cursor?: string;
};

const pages: JSDOM[] = [];

afterEach(() => {
  for (const page of pages.splice(0)) {
    const port = (page.window as unknown as { purlis?: { port?: unknown } }).purlis?.port;
    if (port instanceof MessagePort) port.close();
    page.window.close();
  }
});

/** A page outside purlis declaring `block`, whose folder holds `files`, with the mock loaded. */
function page(block: unknown, files: Record<string, unknown> = {}, before?: (w: Window) => void) {
  const declaration =
    block === undefined
      ? ""
      : `<script type="application/json" id="purlis">${typeof block === "string" ? block : JSON.stringify(block)}</script>`;
  const dom = new JSDOM(`<!doctype html><body>${declaration}</body>`, {
    url: "http://localhost:8000/board/index.html",
    runScripts: "outside-only",
  });
  pages.push(dom);
  const w = dom.window as unknown as Window & { eval(code: string): unknown };
  Object.assign(w, { MessageChannel });
  const fetch = vi.fn(async (url: string) => {
    const name = new URL(url, "http://localhost:8000/board/").pathname.replace("/board/", "");
    if (!(name in files)) return { ok: false, status: 404, json: async () => null };
    const body = files[name];
    return {
      ok: true,
      status: 200,
      json: async () => (typeof body === "string" ? JSON.parse(body) : body),
    };
  });
  Object.assign(w, { fetch });
  const warn = vi.fn();
  const info = vi.fn();
  Object.assign(w, { console: { warn, info, log: vi.fn(), error: vi.fn() } });
  before?.(w);
  w.eval(MOCK);
  const purlis = (w as unknown as { purlis: { api: number; port: MessagePort } }).purlis;
  const ask = (asked: unknown) =>
    new Promise<Answer>((resolve) => {
      purlis.port.onmessage = (event: MessageEvent<Answer>) => resolve(event.data);
      purlis.port.postMessage(asked);
    });
  return { purlis, ask, fetch, warn, info, window: w };
}

const DECLARED = {
  api: 1,
  data: ["data.json", "data/rows.json", "data/gone.json"],
  queries: { "purlis.todo_list@1": ["done"], "purlis.change_status@1": [] },
};

describe("the mock of purlis.port", () => {
  it("hands the page purlis.port at API 1, frozen, as purlis does", () => {
    const { purlis, window } = page(DECLARED);
    expect(purlis.api).toBe(1);
    expect(Object.isFrozen(purlis)).toBe(true);
    expect(Object.getOwnPropertyDescriptor(window, "purlis")?.writable).toBe(false);
  });

  it("steps aside when purlis's own port is already there", () => {
    const own = Object.freeze({ api: 1, port: "purlis's own" });
    const { purlis } = page(DECLARED, {}, (w) =>
      Object.defineProperty(w, "purlis", { value: own }),
    );
    expect(purlis).toBe(own);
  });

  it("answers a declared data file from its fixture, else from the page's folder, else null", async () => {
    const { ask } = page(DECLARED, {
      "purlis-fixtures.json": { data: { "data.json": { rows: 2 } } },
      "data/rows.json": [1, 2],
    });
    expect(await ask({ id: 1, read: "data.json" })).toMatchObject({ id: 1, data: { rows: 2 } });
    expect(await ask({ id: 2, read: "data/rows.json" })).toMatchObject({ id: 2, data: [1, 2] });
    const gone = await ask({ id: 3, read: "data/gone.json" });
    expect(gone).toMatchObject({ id: 3, data: null });
    expect(gone.at).toMatch(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
  });

  it("answers an undeclared read or query not-approved and never fetches it", async () => {
    const { ask, fetch } = page(DECLARED, { "secret.json": { no: 1 } });
    expect(await ask({ id: 1, read: "secret.json" })).toMatchObject({
      id: 1,
      error: { kind: "not-approved" },
    });
    expect(await ask({ id: 2, query: "purlis.memory_search@1" })).toMatchObject({
      error: { kind: "not-approved" },
    });
    expect(await ask({ id: 3, query: "purlis.todo_list" })).toMatchObject({
      error: { kind: "not-approved" },
    });
    expect(fetch.mock.calls.map(([url]) => url)).not.toContain("secret.json");
  });

  it("answers a declared query from its fixture, with its cursor, or the error the fixture names", async () => {
    const todos = { done: true, todos: [{ slug: "fix-login", title: "Fix login", opened: "" }] };
    const { ask } = page(DECLARED, {
      "purlis-fixtures.json": {
        queries: {
          "purlis.todo_list@1": { data: todos, cursor: "c1" },
          "purlis.change_status@1": { error: { kind: "needs-extension" } },
        },
      },
    });
    expect(await ask({ id: 4, query: "purlis.todo_list@1", params: { done: true } })).toMatchObject(
      {
        id: 4,
        data: todos,
        cursor: "c1",
      },
    );
    expect(await ask({ id: 5, query: "purlis.change_status@1", params: null })).toMatchObject({
      id: 5,
      error: { kind: "needs-extension" },
    });
  });

  it("hands on the sentence a fixture's error carries, as purlis does", async () => {
    const said = "purlis could not reach GitHub for the milestones of acme/api.";
    const { ask } = page(DECLARED, {
      "purlis-fixtures.json": {
        queries: { "purlis.change_status@1": { error: { kind: "failed", message: said } } },
      },
    });
    expect(await ask({ id: 6, query: "purlis.change_status@1", params: null })).toEqual(
      expect.objectContaining({ id: 6, error: { kind: "failed", message: said } }),
    );
  });

  it("refuses a parameter the page did not declare, and a query with no fixture fails with a warning", async () => {
    const { ask, warn } = page(DECLARED, { "purlis-fixtures.json": {} });
    expect(
      await ask({ id: 6, query: "purlis.change_status@1", params: { change: "x" } }),
    ).toMatchObject({
      error: { kind: "not-approved" },
    });
    expect(
      await ask({ id: 7, query: "purlis.todo_list@1", params: { done: false } }),
    ).toMatchObject({
      error: { kind: "failed" },
    });
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("purlis.todo_list@1"));
  });

  it("reads nothing when the declaration is not one purlis reads, and says why", async () => {
    for (const block of [
      { api: 2, data: ["data.json"] },
      { api: 1, data: ["../data.json"] },
      "{ not json",
      { api: 1, queries: { "purlis.todo_list": [] } },
    ]) {
      const { ask, warn } = page(block, { "data.json": { n: 1 } });
      expect(await ask({ id: 1, read: "data.json" }), JSON.stringify(block)).toMatchObject({
        error: { kind: "not-approved" },
      });
      expect(warn).toHaveBeenCalled();
    }
  });

  it("reads the declaration when the page has drawn it, even after the mock loaded", async () => {
    const { ask, window } = page(undefined, { "data.json": { n: 1 } });
    window.document.body.innerHTML = `<script type="application/json" id="purlis">${JSON.stringify(DECLARED)}</script>`;
    expect(await ask({ id: 1, read: "data.json" })).toMatchObject({ data: { n: 1 } });
  });

  it("takes a set of offers in closed types and says how many, and refuses any other shape", async () => {
    const { ask, info } = page(DECLARED);
    expect(
      await ask({
        id: 8,
        offers: [
          { does: "todo.done", subject: "fix-login" },
          { does: "todo.delete", subject: "old" },
        ],
      }),
    ).toMatchObject({ id: 8, data: { offered: 2 } });
    expect(info).toHaveBeenCalled();
    for (const offers of [
      [{ does: "todo.done", subject: "x", title: "Mine" }],
      [{ does: "Done", subject: "x" }],
      [{ does: "todo.done", subject: "a\nb" }],
      Array.from({ length: 9 }, (_, i) => ({ does: "todo.done", subject: `t${i}` })),
      [
        { does: "todo.done", subject: "x" },
        { does: "todo.done", subject: "x" },
      ],
    ]) {
      expect(await ask({ id: 9, offers }), JSON.stringify(offers)).toMatchObject({
        id: 9,
        error: { kind: "refused" },
      });
    }
  });

  it("refuses an ask that is no ask, naming its id when it is a number", async () => {
    const { ask } = page(DECLARED);
    expect(await ask({ id: -1, read: "data.json" })).toMatchObject({
      id: -1,
      error: { kind: "refused" },
    });
    expect(await ask({ id: 2, write: "data.json" })).toMatchObject({
      id: 2,
      error: { kind: "refused" },
    });
    expect(await ask("data.json")).toMatchObject({ id: null, error: { kind: "refused" } });
  });
});
