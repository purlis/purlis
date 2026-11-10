// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { focusWorkspace } from "../reach.js";
import { closeTabOf, iconFor } from "../artifactTabs.js";

/**
 * **ADR 0122 §3's isolation tests, in the built app against the real core** (#5952's promise,
 * #5978's review): four of the five the probe `tools/artifact-isolation-probe` asked of a bare
 * webview, asked here of a pinned artifact as purlis renders it, its script approved, on macOS
 * and Linux.
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
 *
 * **Not here: 4, escape** (a script-free page navigates to nothing off its folder: a link, a
 * form, a meta refresh, a stylesheet's import). A page that got away could not say so, so it
 * needs a witness outside the app that each way out would reach; it stays in the probe, and so
 * do the probe and its workflow until it is carried here too.
 */

const NAME = { reach: "Isolation reach", speak: "Isolation speak", walk: "Isolation walk" };
const LOOP = "Isolation loop";

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
(async () => {
  await attempt("own-internals", () => window.__TAURI_INTERNALS__.invoke("vault_list"));
  await attempt("own-internals-planes", () => window.__TAURI_INTERNALS__.invoke("open_planes"));
  await attempt("parent-internals", () => parent.__TAURI_INTERNALS__.invoke("vault_list"));
  await attempt("top-internals", () => top.__TAURI_INTERNALS__.invoke("vault_list"));
  await attempt("ipc-scheme", () => fetch("ipc://localhost/open_planes", { method: "POST", body: "{}", headers }).then((r) => { if (!r.ok) throw r.status; }));
  await attempt("ipc-localhost", () => fetch("http://ipc.localhost/open_planes", { method: "POST", body: "{}", headers }).then((r) => { if (!r.ok) throw r.status; }));
  await attempt("webkit-handler", () => window.webkit.messageHandlers.ipc.postMessage(IPC));
  await attempt("window-ipc", () => window.ipc.postMessage(IPC));
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

/** Opens the artifact named `name` in front and approves its script, as the person does. */
async function openApproved(name: string): Promise<void> {
  await (await iconFor(name)).click();
  await browser.waitUntil(async () => (await inFront()) === name, {
    timeout: 20_000,
    timeoutMsg: `pressing ${name} did not open its tab in front`,
  });
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
  return said;
}

describe("an artifact, as purlis renders it, is isolated from the window and its own folder", function () {
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
    write("isolation-reach", { "index.html": page(NAME.reach), "app.js": REACH });
    write("isolation-speak", {
      "index.html": page(NAME.speak, `<iframe src="nested.html"></iframe>`),
      "nested.html": NESTED,
      "app.js": SPEAK,
    });
    write("isolation-walk", { "index.html": page(NAME.walk), "app.js": WALK, "inside.png": PNG });
    // Beside the walk's folder, unpinned: what a request that walked out would load.
    write("outside", { "secret.png": PNG });
    write("isolation-loop", {
      "index.html": page(LOOP),
      "app.js": "addEventListener('load', () => setTimeout(() => { for (;;) {} }, 200));",
    });
    writeFileSync(
      join(at, "pins.toml"),
      `version = 1
pin = [
  { id = "e2e0000000i1", folder = "isolation-reach", name = "${NAME.reach}", icon = "shield", order = 1 },
  { id = "e2e0000000i2", folder = "isolation-speak", name = "${NAME.speak}", icon = "shield", order = 2 },
  { id = "e2e0000000i3", folder = "isolation-walk", name = "${NAME.walk}", icon = "shield", order = 3 },
  { id = "e2e0000000i4", folder = "isolation-loop", name = "${LOOP}", icon = "shield", order = 4 },
]
`,
    );
    await focusWorkspace("alpha");
  });

  after(async () => {
    for (const name of [...Object.values(NAME), LOOP]) await closeTabOf(name);
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
