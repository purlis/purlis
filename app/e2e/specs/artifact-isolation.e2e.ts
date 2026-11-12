// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { spawn, type ChildProcess } from "node:child_process";
import { cpus } from "node:os";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { focusWorkspace } from "../reach.js";
import { closeTabOf, iconFor, iconOf } from "../artifactTabs.js";

/**
 * **ADR 0122 §3's isolation tests, in the built app against the real core** (#5952's promise,
 * #5978's review): four of the five the probe `tools/artifact-isolation-probe` asked of a bare
 * webview, asked here of a pinned artifact as purlis renders it, its script approved, on macOS
 * and on Linux (ADR 0122, amended 2026-11-11).
 *
 * **How a page is heard.** WebDriver drives the window's webview, never an artifact's own, so a
 * page says what it saw the one way it can reach the window: by what it offers (#5972). Each page
 * offers `todo.done` for a subject naming what it found (`reach-none`, `walk-loaded-…`), which no
 * todo is, so purlis's bar refuses the row with a sentence naming that subject. A page that never
 * ran says nothing, and every test waits for its page's word first, so silence is never a pass.
 *
 * 1. **Reach**: no way a page could reach a Tauri command answers, the vaults' list among them.
 * 2. **Loop**: an endless loop in a page leaves the window answering and its controls working.
 * 3. **Speak**: a frame nested in a page is never heard on the page's channel.
 * 5. **Walk**: no request walks out of the artifact's folder, however its path is spelled.
 * 6. **Store**: what a page keeps (local storage, IndexedDB, a cookie, a cache) is its own
 *    approval's: another artifact and the window see none of it, and nor does the same folder's
 *    page once its template changed, or once it was unpinned and pinned again (ADR 0122,
 *    amended 2026-11-10).
 *
 * **Not here: 4, escape** (a script-free page navigates to nothing off its folder: a link, a
 * form, a meta refresh, a stylesheet's import). A page that got away could not say so, so it
 * needs a witness outside the app that each way out would reach; it stays in the probe, and so
 * do the probe and its workflow until it is carried here too.
 */


/** The page's port, and its one way to be heard: offering `todo.done` for `subject`. */
const SAY = `function say(subject) {
  purlis.port.postMessage({ id: Date.now(), offers: [{ does: "todo.done", subject }] });
}`;

/** Every way a page could reach a Tauri command (the probe's): `open_planes` and the vaults'
 *  list, which the window may call and a page never. */
const REACH = `${SAY}
const answered = [];
const late = (ms) => new Promise((_, no) => setTimeout(() => no(new Error("no answer")), ms));
async function attempt(name, fn) {
  try {
    await Promise.race([Promise.resolve().then(fn), late(1500)]);
    answered.push(name);
  } catch {}
}
const IPC = JSON.stringify({ cmd: "vault_list", callback: 1, error: 2, payload: {}, options: {} });
const headers = { "Content-Type": "application/json", "Tauri-Callback": "1", "Tauri-Error": "2" };
// A post returns nothing whether a command ran or not: Tauri answers one by calling its
// callback back (\`runCallback\`), so only that callback, called with success, is an answer.
function posted(post) {
  return new Promise((yes, no) => {
    const internals = window.__TAURI_INTERNALS__;
    if (internals === undefined || typeof internals.transformCallback !== "function") {
      post(IPC);
      return;
    }
    const callback = internals.transformCallback(yes, true);
    const error = internals.transformCallback(no, true);
    post(JSON.stringify({ cmd: "vault_list", callback, error, payload: {}, options: {} }));
  });
}
(async () => {
  await attempt("own-internals", () => window.__TAURI_INTERNALS__.invoke("vault_list"));
  await attempt("own-internals-planes", () => window.__TAURI_INTERNALS__.invoke("open_planes"));
  await attempt("parent-internals", () => parent.__TAURI_INTERNALS__.invoke("vault_list"));
  await attempt("top-internals", () => top.__TAURI_INTERNALS__.invoke("vault_list"));
  await attempt("ipc-scheme", () => fetch("ipc://localhost/open_planes", { method: "POST", body: "{}", headers }).then((r) => { if (!r.ok) throw r.status; }));
  await attempt("ipc-localhost", () => fetch("http://ipc.localhost/open_planes", { method: "POST", body: "{}", headers }).then((r) => { if (!r.ok) throw r.status; }));
  await attempt("webkit-handler", () => posted((body) => window.webkit.messageHandlers.ipc.postMessage(body)));
  await attempt("window-ipc", () => posted((body) => window.ipc.postMessage(body)));
  await attempt("event-emit", () => window.__TAURI_INTERNALS__.invoke("plugin:event|emit", { event: "x", payload: 1 }));
  say(answered.length === 0 ? "reach-none" : "reach-answered-" + answered.join("-"));
})();`;

/** The page that speaks, and the frame nested in it that tries to speak as it. */
const SPEAK = `${SAY}
say("speak-honest");`;
const NESTED = `<!doctype html><title>nested</title><script>
const forged = { id: 1, offers: [{ does: "todo.done", subject: "speak-forged" }] };
try { parent.purlis.port.postMessage(forged); } catch {}
try { top.purlis.port.postMessage(forged); } catch {}
try { parent.postMessage(forged, "*"); } catch {}
</script>`;

/** Requests that try to walk out of the folder; only its own picture may load (the probe's). */
const WALK = `${SAY}
const TRIES = [
  "../outside/secret.png",
  "%2e%2e/outside/secret.png",
  "..%2foutside%2fsecret.png",
  "..%2Foutside%2Fsecret.png",
  "%2e%2e%2foutside%2fsecret.png",
  "sub/..%2f..%2foutside%2fsecret.png",
  "..%5coutside%5csecret.png",
  "..\\\\outside\\\\secret.png",
];
const loads = (path) => new Promise((done) => {
  const img = new Image();
  img.onload = () => done(true);
  img.onerror = () => done(false);
  img.src = path;
});
(async () => {
  if (!(await loads("inside.png"))) return say("walk-inside-did-not-load");
  const out = [];
  for (const [n, path] of TRIES.entries()) if (await loads(path)) out.push(n);
  say(out.length === 0 ? "walk-none" : "walk-loaded-" + out.join("-"));
})();`;

/** The key every way of keeping something is written under, by the store pages. */
const KEPT = "purlis-e2e-kept";

/** Reads what this origin keeps under {@link KEPT}, then keeps it everywhere it can, and says
 *  both: `store-seen-<what it found>-wrote-<what it kept>`, each `none` when empty. */
const STORE = `${SAY}
const KEY = ${JSON.stringify(KEPT)};
const idb = (write) => new Promise((done) => {
  try {
    const open = indexedDB.open(KEY, 1);
    open.onupgradeneeded = () => open.result.createObjectStore("s");
    open.onerror = () => done(false);
    open.onsuccess = () => {
      const db = open.result;
      const tx = db.transaction("s", write ? "readwrite" : "readonly");
      const got = write ? tx.objectStore("s").put("kept", "k") : tx.objectStore("s").get("k");
      tx.oncomplete = () => { db.close(); done(write || got.result === "kept"); };
      tx.onerror = () => { db.close(); done(false); };
    };
  } catch { done(false); }
});
const tried = async (fn) => { try { return Boolean(await fn()); } catch { return false; } };
(async () => {
  const seen = [];
  if (await tried(() => localStorage.getItem(KEY) === "kept")) seen.push("ls");
  if (await idb(false)) seen.push("idb");
  if (await tried(() => document.cookie.includes(KEY + "=kept"))) seen.push("cookie");
  if (await tried(() => caches.has(KEY))) seen.push("cache");
  const wrote = [];
  if (await tried(() => { localStorage.setItem(KEY, "kept"); return localStorage.getItem(KEY) === "kept"; })) wrote.push("ls");
  if (await idb(true)) wrote.push("idb");
  if (await tried(() => { document.cookie = KEY + "=kept; max-age=3600"; return document.cookie.includes(KEY + "=kept"); })) wrote.push("cookie");
  if (await tried(() => caches.open(KEY).then(() => caches.has(KEY)))) wrote.push("cache");
  say("store-seen-" + (seen.join("-") || "none") + "-wrote-" + (wrote.join("-") || "none"));
})();`;

/** What a store page said it found, and what it kept. */
function stored(word: string): { seen: string[]; wrote: string[] } {
  const [, seen = "", wrote = ""] = /^store-seen-(.*)-wrote-(.*)$/.exec(word) ?? [];
  const list = (text: string) => (text === "none" ? [] : text.split("-"));
  return { seen: list(seen), wrote: list(wrote) };
}

/** A picture, one pixel: inside the walk's folder, and the secret outside it. */
const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
  "base64",
);

const page = (name: string, body = "") =>
  `<!doctype html><title>${name}</title><p>${name}</p>${body}<script src=app.js></script>`;

/** What the app answered a command with, insisting it answered at all. */
async function ask<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const answer = await browser.executeAsync(
    (
      name: string,
      passed: Record<string, unknown>,
      done: (out: { ok?: unknown; trouble?: string }) => void,
    ) => {
      void window.__TAURI__.core
        .invoke(name, passed)
        .then((ok) => done({ ok }))
        .catch((e: unknown) => done({ trouble: String(e) }));
    },
    command,
    args,
  );
  if (answer.trouble !== undefined) throw new Error(`${command} refused: ${answer.trouble}`);
  return answer.ok as T;
}

/** The name of the tab in front. */
async function inFront(): Promise<string | null> {
  return browser.execute(
    () =>
      document.querySelector('[data-strip="Tabs"] [role="tab"][aria-selected="true"]')
        ?.textContent ?? null,
  );
}

/** Every sentence purlis's bar says now of a row a page offered and it refused. */
async function refusedRows(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[data-testid="artifact-offer-refused"]')].map(
      (one) => one.textContent ?? "",
    ),
  );
}

/** Opens the artifact named `name` in front. */
async function openInFront(name: string): Promise<void> {
  await (await iconFor(name)).click();
  await browser.waitUntil(async () => (await inFront()) === name, {
    timeout: 20_000,
    timeoutMsg: `pressing ${name} did not open its tab in front`,
  });
}

/** Opens the artifact named `name` in front and approves its script, as the person does. */
async function openApproved(name: string): Promise<void> {
  await openInFront(name);
  const approve = await $('[data-testid="artifact-bar"]').$("aria/Approve script");
  await approve.waitForExist({ timeout: 20_000 });
  await approve.click();
}

/** Waits for the page in front to say a word starting with `prefix`, and returns it. */
async function heard(prefix: string): Promise<string> {
  let said = "";
  await browser.waitUntil(
    async () => {
      for (const sentence of await refusedRows()) {
        const word = sentence.split(" ").find((one) => one.startsWith(prefix));
        if (word !== undefined) said = word.replace(/,$/, "");
      }
      return said !== "";
    },
    { timeout: 30_000, timeoutMsg: `the page never said ${prefix}…: it did not run` },
  );
  console.log(`[DEBUG-6232] heard ${said} at ${Date.now()} rows=${JSON.stringify(await refusedRows())} front=${await inFront()}`);
  return said;
}

/** [LOOP-6232] temporary: what the window shows right after a close was pressed. */
async function settled(name: string): Promise<void> {
  const look = async () => `front=${await inFront()} rows=${(await refusedRows()).length} tabs=${await browser.execute(() => [...document.querySelectorAll('[data-strip="Tabs"] [role="tab"]')].map((one) => one.textContent).join("|"))} active=${await browser.execute(() => document.activeElement?.textContent?.slice(0, 40) ?? "")}`;
  console.log(`[DEBUG-6232] closed ${name} at ${Date.now()} now: ${await look()}`);
}

/** Sends the context menu to the control `selector` names, as a right press does. */
async function sendContextMenu(selector: string): Promise<boolean> {
  return browser.execute((css: string) => {
    const el = document.querySelector(css);
    if (!(el instanceof HTMLElement)) return false;
    const box = el.getBoundingClientRect();
    el.dispatchEvent(
      new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        clientX: Math.round(box.left + box.width / 2),
        clientY: Math.round(box.top + box.height / 2),
      }),
    );
    return true;
  }, selector);
}

/** What the window's own origin keeps under {@link KEPT}, every way a page could keep it. */
async function keptByTheWindow(): Promise<string[]> {
  return browser.executeAsync((key: string, done: (found: string[]) => void) => {
    void (async () => {
      const found: string[] = [];
      const look = async (name: string, kept: () => boolean | Promise<boolean>) => {
        try {
          if (await kept()) found.push(name);
        } catch {
          // A way of keeping the window's origin does not have keeps nothing.
        }
      };
      await look("ls", () => localStorage.getItem(key) !== null);
      await look("idb", async () => (await indexedDB.databases()).some((one) => one.name === key));
      await look("cookie", () => document.cookie.includes(key));
      await look("cache", () => caches.has(key));
      done(found);
    })();
  }, KEPT);
}

// [LOOP-6232] temporary: load every core while the spec runs, to raise the flake's rate.
const HOGS: ChildProcess[] = [];
before(() => {
  for (let i = 0; i < cpus().length; i += 1)
    HOGS.push(spawn("sh", ["-c", "while :; do :; done"], { stdio: "ignore" }));
  console.log(`[DEBUG-6232] hogs ${HOGS.length}`);
});
after(() => {
  for (const hog of HOGS) hog.kill("SIGKILL");
});
// [LOOP-6232] temporary: runs the describe PURLIS_LOOP times, new names, folders and pins each pass.
for (const pass of Array.from({ length: Number(process.env.PURLIS_LOOP ?? "1") }, (_, i) => i + 1)) {
const pp = String(pass).padStart(2, "0");
const NAME = { reach: `Isolation reach ${pass}`, speak: `Isolation speak ${pass}`, walk: `Isolation walk ${pass}` };
const LOOP = `Isolation loop ${pass}`;
const STORES = { a: `Isolation store A ${pass}`, b: `Isolation store B ${pass}` };
/** The store page A's line of the pins file: written again to pin it again. */
const STORE_A_PIN = `  { id = "e2e000${pp}00i5", folder = "isolation-store-a-${pass}", name = "${STORES.a}", icon = "shield", order = 5 },\n`;

/** The pins file every test starts from. */
const PINS = `version = 1
pin = [
  { id = "e2e000${pp}00i1", folder = "isolation-reach-${pass}", name = "${NAME.reach}", icon = "shield", order = 1 },
  { id = "e2e000${pp}00i2", folder = "isolation-speak-${pass}", name = "${NAME.speak}", icon = "shield", order = 2 },
  { id = "e2e000${pp}00i3", folder = "isolation-walk-${pass}", name = "${NAME.walk}", icon = "shield", order = 3 },
  { id = "e2e000${pp}00i4", folder = "isolation-loop-${pass}", name = "${LOOP}", icon = "shield", order = 4 },
${STORE_A_PIN}  { id = "e2e000${pp}00i6", folder = "isolation-store-b-${pass}", name = "${STORES.b}", icon = "shield", order = 6 },
]
`;

describe(`an artifact, as purlis renders it, is isolated from the window and its own folder (pass ${pass})`, function () {
  this.timeout(300_000);

  let at = "";

  before(async () => {
    const plane = (await ask<string[]>("open_planes"))[0];
    at = join(plane, "workspaces", "alpha", "artifacts");
    const write = (folder: string, files: Record<string, string | Buffer>) => {
      mkdirSync(join(at, folder), { recursive: true });
      for (const [name, bytes] of Object.entries(files))
        writeFileSync(join(at, folder, name), bytes);
    };
    write(`isolation-reach-${pass}`, { "index.html": page(NAME.reach), "app.js": REACH });
    write(`isolation-speak-${pass}`, {
      "index.html": page(NAME.speak, `<iframe src="nested.html"></iframe>`),
      "nested.html": NESTED,
      "app.js": SPEAK,
    });
    write(`isolation-walk-${pass}`, { "index.html": page(NAME.walk), "app.js": WALK, "inside.png": PNG });
    // Beside the walk's folder, unpinned: what a request that walked out would load.
    write("outside", { "secret.png": PNG });
    write(`isolation-loop-${pass}`, {
      "index.html": page(LOOP),
      "app.js": "addEventListener('load', () => setTimeout(() => { for (;;) {} }, 200));",
    });
    write(`isolation-store-a-${pass}`, { "index.html": page(STORES.a), "app.js": STORE });
    write(`isolation-store-b-${pass}`, { "index.html": page(STORES.b), "app.js": STORE });
    writeFileSync(join(at, "pins.toml"), PINS);
    await focusWorkspace("alpha");
  });

  after(async () => {
    for (const name of [...Object.values(NAME), ...Object.values(STORES), LOOP])
      await closeTabOf(name);
    rmSync(at, { recursive: true, force: true });
  });

  it("1. reach: no way a page could reach a Tauri command answers", async () => {
    await openApproved(NAME.reach);
    expect(await heard("reach-")).toBe("reach-none");
    await closeTabOf(NAME.reach);
  });

  it("3. speak: a frame nested in a page is never heard on the page's channel", async () => {
    await openApproved(NAME.speak);
    await heard("speak-honest");
    // A forged set would replace the honest one, so every look for a while is read.
    const end = Date.now() + 3_000;
    while (Date.now() < end) {
      expect((await refusedRows()).some((one) => one.includes("speak-forged"))).toBe(false);
      await browser.pause(250);
    }
    await closeTabOf(NAME.speak);
  });

  it("5. walk: no request walks out of the artifact's folder", async () => {
    await openApproved(NAME.walk);
    expect(await heard("walk-")).toBe("walk-none");
    await closeTabOf(NAME.walk);
  });

  it("6. store: what a page keeps is its approval's alone", async () => {
    // A keeps something every way it can, and finds it again on its next load: so what the
    // others do not find is kept apart, not lost.
    await openApproved(STORES.a);
    const first = stored(await heard("store-"));
    expect(first.seen).toEqual([]);
    expect(first.wrote).toEqual(expect.arrayContaining(["ls", "idb"]));
    await closeTabOf(STORES.a);
    await settled(STORES.a);
    await (await iconFor(STORES.a)).click();
    const again = stored(await heard("store-"));
    expect(again.seen).toEqual(first.wrote);
    await closeTabOf(STORES.a);
    await settled(STORES.a);

    // Another artifact, and the window, find none of it.
    await openApproved(STORES.b);
    expect(stored(await heard("store-")).seen).toEqual([]);
    await closeTabOf(STORES.b);
    await settled(STORES.b);
    expect(await keptByTheWindow()).toEqual([]);

    // A's template changed: approved again, it is a new page that finds nothing.
    writeFileSync(join(at, `isolation-store-a-${pass}`, "app.js"), `${STORE}\n// changed\n`);
    await openApproved(STORES.a);
    const changed = stored(await heard("store-"));
    expect(changed.seen).toEqual([]);
    await closeTabOf(STORES.a);
    await settled(STORES.a);

    // Unpinned here and pinned again, the same folder and command: asked again, it finds
    // nothing either.
    console.log(`[DEBUG-6232] before menu at ${Date.now()} front=${await inFront()} icon=${await $(iconOf(STORES.a)).isExisting()} active=${await browser.execute(() => document.activeElement?.outerHTML.slice(0, 200) ?? "")}`);
    expect(await sendContextMenu(iconOf(STORES.a))).toBe(true);
    const menu = await $('[role="menu"]');
    console.log(`[DEBUG-6232] after menu at ${Date.now()} menus=${await browser.execute(() => [...document.querySelectorAll('[role="menu"]')].length)}`);
    await menu.waitForDisplayed({ timeout: 20_000 });
    await menu.$('[role="menuitem"]=Unpin from workspace…').click();
    const dialog = await $('[role="alertdialog"]');
    await dialog.waitForDisplayed({ timeout: 20_000 });
    await dialog.$("aria/Unpin").click();
    await browser.waitUntil(async () => !(await $(iconOf(STORES.a)).isExisting()), {
      timeout: 20_000,
      timeoutMsg: `${STORES.a} stayed on the bar after its unpin`,
    });
    const unpinned = readFileSync(join(at, "pins.toml"), "utf8");
    writeFileSync(join(at, "pins.toml"), unpinned.replace("]\n", `${STORE_A_PIN}]\n`));
    await openApproved(STORES.a);
    expect(stored(await heard("store-")).seen).toEqual([]);
    await closeTabOf(STORES.a);
    await settled(STORES.a);
    expect(await keptByTheWindow()).toEqual([]);
  });

  // [LOOP-6232] temporary: the changed-template step and the menu after it, over and over.
  it("6b. hammer: changed template, then the pin's menu", async () => {
    let menuMiss = 0;
    let storeMiss = 0;
    const CYCLES = Number(process.env.PURLIS_HAMMER ?? "25");
    for (let cycle = 1; cycle <= CYCLES; cycle += 1) {
      writeFileSync(join(at, `isolation-store-a-${pass}`, "app.js"), `${STORE}\n// hammer ${cycle}\n`);
      await openApproved(STORES.a);
      const word = await heard("store-");
      if (stored(word).seen.length !== 0) {
        storeMiss += 1;
        console.log(`[DEBUG-6232] STORE-MISS cycle ${cycle} word=${word} rows=${JSON.stringify(await refusedRows())}`);
      }
      await closeTabOf(STORES.a);
      const sent = Date.now();
      await sendContextMenu(iconOf(STORES.a));
      const shown = await browser
        .waitUntil(async () => (await $('[role="menu"]').isDisplayed()), { timeout: 5_000 })
        .then(() => true, () => false);
      if (!shown) {
        menuMiss += 1;
        console.log(`[DEBUG-6232] MENU-MISS cycle ${cycle} sent=${sent} menus=${await browser.execute(() => document.querySelectorAll('[role="menu"]').length)} active=${await browser.execute(() => document.activeElement?.outerHTML.slice(0, 160) ?? "")} open=${await browser.execute((css: string) => document.querySelector(css)?.getAttribute("data-open"), iconOf(STORES.a))}`);
      } else {
        await browser.keys("Escape");
        await $('[role="menu"]').waitForDisplayed({ reverse: true, timeout: 5_000 });
      }
    }
    console.log(`[DEBUG-6232] HAMMER pass ${pass}: menu misses ${menuMiss}/${CYCLES}, store misses ${storeMiss}/${CYCLES}`);
  });

  it("2. loop: an endless loop in a page leaves the window answering", async () => {
    await openApproved(LOOP);
    await browser.waitUntil(
      async () =>
        (await $('[data-testid="artifact-bar"]').getText()).includes(
          "Script approved on this machine",
        ),
      { timeout: 20_000, timeoutMsg: "the approval did not reach the tab" },
    );
    // The page has looped for a while: the window still answers, and a press still works.
    await browser.pause(2_000);
    const started = Date.now();
    expect(await browser.execute(() => document.title)).not.toBe("");
    expect(Date.now() - started).toBeLessThan(5_000);
    await (await iconFor(NAME.reach)).click();
    await browser.waitUntil(async () => (await inFront()) === NAME.reach, {
      timeout: 10_000,
      timeoutMsg: "with a page looping, a press on another artifact did nothing",
    });
  });
});
}
