// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  mkdtempSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { createServer, type Server } from "node:net";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { pressGrant } from "../acting.js";
import { bandNotice } from "../reading.js";
import { showView } from "../opening.js";
import { PROJECTS, ask, inFront, keepTheNotices, openFromTheStrip } from "../switching.js";

/**
 * **The up-front question of what a project expects** (ADR 0093 as amended 2026-10-13, ruling R3;
 * #2627), in the built app against the real core and this machine's real file. It is the sandbox
 * consent, asked on its row in Asks (`asking::consents`, #6000) and there alone while it can be
 * answered (R-205): no Notice carries it beside its row.
 *
 * - **Allow** keeps each item for this project on this machine: the ask goes, does not come
 *   back when the settings are read again, and Settings › Sandbox › Network › Allowed lists them.
 * - **Not now** takes the ask away and allows nothing; Settings still offers the Allow.
 * - **A newly declared item** asks again after a Not now.
 * - **An item that changed under the question**: a declared path that is a link to a real socket,
 *   moved to another socket between the drawing and the press, is not kept, and the ask says so
 *   and is drawn again as it stands (`EXPECTS_CHANGED`).
 *
 * `sandboxing/tests.rs`, `projectExpects.test.ts` and `ProjectExpectsNotice.test.tsx` hold each
 * rule against synthetic values; what only the window shows is the whole path, from the committed
 * file on disk through the watcher, the ask, the window-only commands and this machine's file, to
 * Settings.
 *
 * **The sockets are real, and outside every folder a chat may write** (#2626's set-up): the core
 * refuses a socket in the temp folder, in the project and in a harness's home
 * (`SocketGround::of`), and `sun_path` holds 104 bytes. So they are bound in a folder of this
 * user's own, under the runner's home (never the run's `$HOME`, which is in the temp folder), and
 * the folder goes at the end.
 *
 * **Its own project, copied into the run's tree**, and let go of at the end: the declaration, the
 * answers and the grants are all this project's, keyed by its clone, and one app process serves
 * the whole run.
 */

const NAME = "project-expects";
const CAUSE = "sandbox-expects";
const ALLOW = "Allow on this machine";
const NOT_NOW = "Not now";
/** The core's sentence for an item that changed under the question (`EXPECTS_CHANGED`). */
const CHANGED =
  "What this project expects changed on this machine since it was shown, so that was not allowed. It is shown again as it stands now.";

/** Services a project may declare: reverse-DNS names no machine runs. */
const FIRST = ["com.example.purlis-e2e.first", "com.example.purlis-e2e.second"];
const LATER = "com.example.purlis-e2e.later";
const NEWER = "com.example.purlis-e2e.newer";

const TABS = '[data-strip="Tabs"]';
const LEVEL = '[role="radiogroup"][aria-label="Level"]';
const GROUPS = 'nav[aria-label="Groups"]';
const EXPECTS_ROW = '[data-setting="project.sandbox.expects-here"]';
/** The consent's row in the Asks view. */
const CONSENT = `[role="tabpanel"][data-view="asks"] .inbox-ask[data-source="${CAUSE}"]`;
const ALLOWED_ROW = '[data-setting="project.sandbox.network.allowed"]';

/** This spec's own project: the fixture, renamed, with nothing to put back on open. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), NAME);
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  anEmptyRecord(root);
  return root;
})();

const manifest = join(mine, "charter.toml");
/** The fixture's manifest, which every declaration is written after. */
const fixtureManifest = readFileSync(manifest, "utf8");

/** Writes the committed file with the sandbox on and `items` declared as expected. */
function declare(items: readonly string[]): void {
  const listed = items.map((one) => JSON.stringify(one)).join(", ");
  writeFileSync(
    manifest,
    `${fixtureManifest.trimEnd()}\n\n[sandbox]\nmode = "on"\nexpects = [${listed}]\n`,
  );
}

// Declared before the project opens: the question is asked where the declaration is read.
declare(FIRST);

type Item = { what: string; target: string; named: string; refused: string | null };
type Expects = { items: Item[]; locked: string | null; asked: boolean } | null;
type Grant = { what: string; target: string; level: string };

const expects = async () =>
  (await ask<{ expects: Expects }>("sandbox_state", { plane: mine })).expects;
const grants = () => ask<Grant[]>("sandbox_grants", { plane: mine });

/** What the consent's row in Asks says now, or null where none is drawn. */
const rowSaid = () =>
  browser.execute(
    (row: string) => document.querySelector<HTMLElement>(row)?.innerText ?? null,
    CONSENT,
  );

/** Waits for the consent's row in Asks to say `text`, each of it at once when it is a list, and
 *  answers all it says. Asked in Asks alone (R-205): no Notice carries it beside its row. */
async function theAskSaying(text: string | readonly string[], why: string): Promise<string> {
  const all = typeof text === "string" ? [text] : text;
  let said = "";
  await browser.waitUntil(
    async () => {
      said = (await rowSaid()) ?? "";
      return said !== "" && all.every((one) => said.includes(one));
    },
    { timeout: 30_000, interval: 250, timeoutMsg: `${why}; the row in Asks said: ${said}` },
  );
  expect((await bandNotice(CAUSE)).state).toBe("absent");
  return said;
}

/** Waits for the consent's row in Asks, and answers what it says. */
const theAsk = (why: string) => theAskSaying([], `no ask of what the project expects: ${why}`);

/** Waits for the consent's row to go; one that stays says what it says, and what the window said
 *  of the press (a press too soon after a way out moved allows nothing, and says so). */
async function noAsk(why: string): Promise<void> {
  await browser
    .waitUntil(async () => (await rowSaid()) === null, {
      timeout: 30_000,
      interval: 250,
    })
    .catch(async () => {
      const seen = await browser.execute((row: string) => {
        const line = document.querySelector<HTMLElement>(row);
        const statuses = [...document.querySelectorAll<HTMLElement>('[role="status"]')]
          .map((one) => one.innerText.trim())
          .filter((one) => one !== "");
        const marked = document.querySelector<HTMLElement>("[data-e2e-pressed]");
        return `the row says: ${line?.innerText.replace(/\s+/g, " ") ?? "(none)"}; the window's statuses: ${statuses.join(" | ")}; focus on: ${document.activeElement?.textContent?.trim() ?? "nothing"}; the button pressed is ${marked === null ? "gone: drawn again since" : "still drawn"}`;
      }, CONSENT);
      throw new Error(`${why}; ${seen}; kept: ${await keepTheNotices("project-expects")}`);
    });
}

/** Presses one of the consent's buttons on its row. An answer that grants waits for the row to
 *  settle where it is drawn (`SETTLE_MS`, 1.5 s, #1695, #3142), as the person reads it before
 *  allowing it; a press before that allows nothing. */
async function press(label: string): Promise<void> {
  const button = await $(CONSENT).$(`button=${label}`);
  await button.waitForClickable({ timeout: 20_000 });
  // Marked, so a press the guard holds can say whether this is still the button drawn.
  await browser.execute(
    (el: HTMLElement) => {
      el.dataset.e2ePressed = String(Date.now());
    },
    await button,
  );
  if (label !== ALLOW) {
    await button.click();
    return;
  }
  // **Pressed again where the window says so** (`MOVED_JUST_NOW`): a row above it that changes
  // height slides its Allow on screen. The guard holds that press and asks the person to read it
  // and press again, which is what this does, at most twice more.
  for (let tries = 0; tries < 3; tries += 1) {
    await pressGrant(button);
    const held = await browser
      .waitUntil(
        () =>
          browser.execute(() =>
            [...document.querySelectorAll<HTMLElement>('[role="status"]')].some((one) =>
              one.innerText.includes("moved just now"),
            ),
          ),
        { timeout: 2_500, interval: 250 },
      )
      .catch(() => false);
    if (!held || !(await button.isExisting())) return;
  }
}

/** Opens Settings for the project in front, at the Project level. */
async function openSettings(): Promise<void> {
  await browser.executeAsync((done: () => void) => {
    void window.__TAURI__.event.emit("settings-asked").then(done, done);
  });
  await $(LEVEL).waitForExist({ timeout: 30_000 });
  await browser.waitUntil(
    async () =>
      browser.execute((selector: string) => {
        const radios = [
          ...document.querySelectorAll<HTMLElement>(`${selector} input[type="radio"]`),
        ];
        const project = radios.find(
          (one) => one.closest("label")?.textContent?.trim() === "Project",
        );
        if ((project as HTMLInputElement | undefined)?.checked === true) return true;
        project?.click();
        return false;
      }, LEVEL),
    { timeout: 30_000, interval: 250, timeoutMsg: "Settings never came to the Project level" },
  );
}

/** Opens the group called `label` from Settings' nav. */
async function openGroup(label: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (nav: string, wanted: string) => {
          const button = [...document.querySelectorAll<HTMLElement>(`${nav} button`)].find(
            (one) => one.textContent?.trim() === wanted,
          );
          if (button?.getAttribute("aria-current") === "page") return true;
          button?.click();
          return false;
        },
        GROUPS,
        label,
      ),
    { timeout: 20_000, interval: 250, timeoutMsg: `Settings offered no group ${label}` },
  );
}

/** Closes every Settings tab, so the next one opened reads what stands now. */
async function closeSettings(): Promise<void> {
  await browser.execute((strip: string) => {
    for (const closer of document.querySelectorAll<HTMLElement>(
      `${strip} button[aria-label^="Close Settings"]`,
    ))
      closer.click();
  }, TABS);
  await browser.waitUntil(
    async () => !(await $(`${TABS} button[aria-label^="Close Settings"]`).isExisting()),
    { timeout: 20_000, timeoutMsg: "a Settings tab stayed open" },
  );
}

/** The text of the row `selector` in Settings' group on screen, once it is drawn with `text`. */
async function rowSaying(selector: string, text: string, why: string): Promise<string> {
  let said = "";
  await browser.waitUntil(
    async () => {
      said = await browser.execute(
        (row: string) => document.querySelector<HTMLElement>(row)?.innerText ?? "",
        selector,
      );
      return said.includes(text);
    },
    { timeout: 20_000, interval: 250, timeoutMsg: `${why}; the row said: ${said}` },
  );
  return said;
}

/** A Unix socket bound at `path`, listening until it is closed. */
async function aSocketAt(path: string): Promise<Server> {
  const server = createServer((connection) => connection.end());
  await new Promise<void>((bound, failed) => {
    server.once("error", failed);
    server.listen(path, () => bound());
  });
  return server;
}

describe("the question of what a project expects, in Asks", function () {
  this.timeout(240_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  /** The folder of this user's own the sockets are bound in, made by `it` and gone at the end. */
  let sockets = "";
  const servers: Server[] = [];

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    await openFromTheStrip(mine);
    await showView("Asks");
  });

  after(async () => {
    await closeSettings().catch(() => {});
    for (const server of servers) await new Promise<void>((done) => server.close(() => done()));
    // A folder `it` made under the runner's home, by its own prefix, and nothing else.
    const folder = sockets;
    if (folder !== "" && folder.startsWith(join(homedir(), ".purlis-e2e-")))
      rmSync(folder, { recursive: true, force: true });
    if ((await ask<string[]>("open_planes")).includes(mine))
      await ask("close_plane", { plane: mine });
    await browser.waitUntil(async () => !(await ask<string[]>("open_planes")).includes(mine), {
      timeout: 20_000,
      timeoutMsg: "this spec's project was not let go of",
    });
    // The launch's project in front again, from its own tab.
    await browser.waitUntil(
      async () => {
        if ((await inFront()) === first) return true;
        await browser.execute(
          (strip: string, path: string) =>
            [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
              .find((tab) => tab.getAttribute("data-directory") === path)
              ?.click(),
          PROJECTS,
          first,
        );
        return false;
      },
      { timeout: 20_000, interval: 250, timeoutMsg: "the launch's project never came back" },
    );
  });

  it("asks once, and Allow keeps each item for this project on this machine", async () => {
    const said = await theAsk("the project declares two services and opened");
    // Its own words, with each item.
    expect(said).toContain("This project expects more than this machine allows");
    for (const one of FIRST) expect(said).toContain(one);

    await press(ALLOW);
    await noAsk("the ask stayed up after Allow");

    // Kept, each, for this project on this machine, and nothing waits.
    const kept = await grants();
    for (const one of FIRST)
      expect(kept).toContainEqual(expect.objectContaining({ what: "service", target: one }));
    expect(await expects()).toBeNull();

    // Settings › Sandbox › Network › Allowed lists them, as the project expects them.
    await openSettings();
    await openGroup("Network");
    for (const one of FIRST)
      await rowSaying(
        ALLOWED_ROW,
        `Reach the system service ${one} (this project expects it)`,
        `Allowed did not list ${one}`,
      );
    await closeSettings();

    // The settings read again, as the watcher reads them when the file changes: the ask does
    // not return. The same declaration, written anew.
    await showView("Asks");
    declare(FIRST);
    await browser.pause(3_000);
    expect(await rowSaid()).toBeNull();
  });

  it("goes on Not now, allows nothing, and Settings still offers the Allow", async () => {
    // Something new to ask about, so the question returns (the next case holds that too).
    declare([...FIRST, LATER]);
    const said = await theAskSaying(LATER, "a newly declared service did not ask");
    // What is allowed already is not asked about again.
    for (const one of FIRST) expect(said).not.toContain(one);

    await press(NOT_NOW);
    await noAsk("the ask stayed up after Not now");

    // Nothing allowed: it still waits, and is not asked about.
    expect(await grants()).not.toContainEqual(expect.objectContaining({ target: LATER }));
    const waits = await expects();
    expect(waits?.asked).toBe(false);
    expect(waits?.items.map((one) => one.target)).toEqual([LATER]);

    // Settings › Sandbox still offers the Allow, for what waits.
    await openSettings();
    await openGroup("Sandbox");
    await rowSaying(EXPECTS_ROW, LATER, "Settings › Sandbox did not say what still waits");
    await expect($(EXPECTS_ROW).$(`button=${ALLOW}`)).toBeExisting();
    await closeSettings();

    // And the Not now lasts: the settings read again do not bring it back.
    await showView("Asks");
    declare([...FIRST, LATER]);
    await browser.pause(3_000);
    expect(await rowSaid()).toBeNull();
  });

  it("asks again, after a Not now, when the project declares something new", async () => {
    declare([...FIRST, LATER, NEWER]);
    const said = await theAskSaying(NEWER, "a service declared after Not now did not ask");
    // The question is the whole of what waits: the one answered Not now with the new one.
    expect(said).toContain(LATER);
    await press(NOT_NOW);
    await noAsk("the ask stayed up after the second Not now");
    expect(await grants()).not.toContainEqual(expect.objectContaining({ target: NEWER }));
  });

  it("keeps nothing that changed under the question, and draws it again as it stands", async () => {
    // Two real sockets in a folder of this user's own, and a declared path that is a link to A.
    sockets = mkdtempSync(join(homedir(), ".purlis-e2e-"));
    const a = join(sockets, "a.sock");
    const b = join(sockets, "b.sock");
    servers.push(await aSocketAt(a), await aSocketAt(b));
    const link = join(sockets, "expected.sock");
    symlinkSync(a, link);
    const named = { a: realpathSync(a), b: realpathSync(b) };

    // The link alone is declared, so an Allow has nothing else it could keep.
    declare([link]);
    // Named as an Allow would keep it.
    await theAskSaying(named.a, "the ask did not name the socket A link leads to");

    // The link moves to B between the drawing and the press.
    const next = join(sockets, "next.sock");
    symlinkSync(b, next);
    renameSync(next, link);
    expect(realpathSync(link)).toBe(named.b);
    // Still drawn as it was: nothing the window watches moved.
    expect(await theAsk("the ask went when the link moved")).toContain(named.a);

    await press(ALLOW);
    // Not kept, said, and drawn again naming B. The window draws B late: the sentence lands with
    // the refusal, and only then is the list read again, so the row drawn in between says the
    // sentence beside A's list (#4665). One wait, for both at once, never a read of either alone.
    await theAskSaying(
      [CHANGED, named.b],
      "the Allow was not refused as changed and drawn again naming B",
    );
    expect(await grants()).not.toContainEqual(expect.objectContaining({ what: "socket" }));

    // Allowed as it stands now, it is kept: B, the socket the Notice names.
    await press(ALLOW);
    await noAsk("the ask stayed up after Allow for the socket as it stands");
    const kept = await grants();
    expect(kept).toContainEqual(expect.objectContaining({ what: "socket", target: named.b }));
    expect(kept).not.toContainEqual(expect.objectContaining({ what: "socket", target: named.a }));
  });
});
