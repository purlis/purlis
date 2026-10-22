// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $$, browser, expect } from "@wdio/globals";
import { focusWorkspace, projectRoot, stateOf } from "../acting.js";
import {
  SELECTED_TAB,
  bringChatForward,
  endChatsStartedHere,
  startAChat,
  type Started,
} from "../started.js";
import { ask } from "../switching.js";

/**
 * **A chat's theme draft, end to end** (#2332, spec #2313), against the real app: a draft in the
 * local folder of a chat working in a workspace is drawn there and named on the workspace's
 * strip; Compare draws the kept theme while it is held; a save that breaks a hard floor is not
 * drawn and the last good version stays; the revert key, Revert and Keep each end the draft; and
 * closing the chat asks Keep or Revert.
 *
 * The engine's tests hold where a draft is, how it is read, the hard floors and what Keep and
 * Revert leave on disk (`theme/drafts_tests.rs`), and the app's which chat's draft a workspace
 * draws (`themedrafts.rs`). What only the real app can say is that a save is heard, the window
 * draws it, and every press reaches the core.
 *
 * **It leaves the window as it found it**: its chat is ended, its drafts and kept folder removed,
 * and the choice it made taken back.
 */

const WORKSPACE = "alpha";
const FOLDER = "e2e-draft";
/** A save is drawn within a second (`reload::QUIET_FOR`, `reload::GAP`); a busy runner's file
 *  events can be late. */
const DRAWN_WITHIN = 10_000;
const STRIP = '[role="region"][aria-label="Theme draft"]';

/** A dark grey beside Purlis Classic's own window, which every word and mark clears. */
const FIRST = "#191917";
const SECOND = "#1a1a18";
const UNREADABLE = "#1b1b19";

/** What the window draws `surface.base` in now. */
async function surfaceDrawn(): Promise<string> {
  return browser.execute(() =>
    document.documentElement.style.getPropertyValue("--surface-base").trim().toLowerCase(),
  );
}

async function untilDrawn(surface: string, why: string): Promise<void> {
  await browser.waitUntil(async () => (await surfaceDrawn()) === surface, {
    timeout: DRAWN_WITHIN,
    interval: 50,
    timeoutMsg: `${why}: the window draws ${await surfaceDrawn()}, not ${surface}`,
  });
}

/** The chat's id (its ULID) from the reopen record the app keeps of its open chats. */
async function idOf(plane: string, session: number): Promise<string> {
  const record = join(stateOf(plane), "app", "reopen.json");
  let id = "";
  await browser.waitUntil(
    () => {
      if (!existsSync(record)) return false;
      try {
        const read = JSON.parse(readFileSync(record, "utf8")) as {
          chats?: { number?: number; id?: string }[];
        };
        id = read.chats?.find((one) => one.number === session)?.id ?? "";
      } catch {
        return false;
      }
      return id !== "";
    },
    { timeout: 20_000, interval: 200, timeoutMsg: `chat ${session} has no id in ${record}` },
  );
  return id;
}

/** Writes the chat's draft: one dark appearance drawing `surface.base` in `surface`, and
 *  `tokens` beside it. */
function draft(folder: string, surface: string, tokens: Record<string, string> = {}): void {
  mkdirSync(folder, { recursive: true });
  writeFileSync(
    join(folder, "theme.json"),
    JSON.stringify({
      name: "E2E draft",
      appearances: [
        { id: "dark", appearance: "dark", tokens: { "surface.base": surface, ...tokens } },
      ],
    }),
  );
}

async function stripSays(): Promise<string> {
  return browser.execute(
    (strip: string) => document.querySelector(strip)?.textContent ?? "",
    STRIP,
  );
}

async function untilStripSays(words: string): Promise<void> {
  await browser.waitUntil(async () => (await stripSays()).includes(words), {
    timeout: DRAWN_WITHIN,
    interval: 100,
    timeoutMsg: `the draft strip never said "${words}"; it says "${await stripSays()}"`,
  });
}

async function untilNoStrip(): Promise<void> {
  await browser.waitUntil(async () => (await $$(STRIP).getElements()).length === 0, {
    timeout: DRAWN_WITHIN,
    interval: 100,
    timeoutMsg: `the draft strip is still there: "${await stripSays()}"`,
  });
}

/** Presses the strip's button `words`, or sends it `events` in order. */
async function onStrip(words: string, events: string[] = ["click"]): Promise<void> {
  const pressed = await browser.execute(
    (strip: string, label: string, sent: string[]) => {
      const button = [
        ...(document.querySelector(strip)?.querySelectorAll<HTMLButtonElement>("button") ?? []),
      ].find((one) => one.textContent?.trim() === label);
      if (button === undefined) return false;
      for (const kind of sent) {
        if (kind === "click") button.click();
        else button.dispatchEvent(new PointerEvent(kind, { bubbles: true }));
      }
      return true;
    },
    STRIP,
    words,
    events,
  );
  if (!pressed) throw new Error(`the draft strip has no ${words}: "${await stripSays()}"`);
}

/** The workspace in front by the name its tab draws, or `null` at the plane root. */
async function workspaceInFront(): Promise<string | null> {
  return browser.execute(() => {
    const tab = document.querySelector<HTMLElement>(
      '[data-strip="Workspaces"] [role="tab"][aria-selected="true"]',
    );
    if (tab === null || tab.classList.contains("plane-root")) return null;
    return tab.querySelector(".workspace-name")?.textContent?.trim() ?? null;
  });
}

/** Back to the plane root, from its tab. */
async function focusThePlaneRoot(): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(() => {
        const root = document.querySelector<HTMLElement>(
          '[data-strip="Workspaces"] [role="tab"].plane-root',
        );
        if (root === null) return false;
        if (root.getAttribute("aria-selected") === "true") return true;
        root.click();
        return false;
      }),
    { timeout: 30_000, interval: 250, timeoutMsg: "the plane root was never in front again" },
  );
}

describe("a chat's theme draft", () => {
  let chat: Started | undefined;
  let was: string | null = null;
  let plane = "";
  let drafts = "";
  const kept = () => join(plane, "workspaces", WORKSPACE, "themes", FOLDER);

  before(async () => {
    plane = await projectRoot();
    was = await workspaceInFront();
    await focusWorkspace(WORKSPACE);
    chat = await startAChat();
    drafts = join(stateOf(plane), "theme-drafts", await idOf(plane, chat.session));
  });

  after(async () => {
    // A question a failed step left open is cancelled first, so the close below is asked again
    // with no draft to ask about.
    await browser.execute(() => {
      const asking = document.querySelector('[role="alertdialog"]');
      [...(asking?.querySelectorAll<HTMLButtonElement>("button") ?? [])]
        .find((one) => one.textContent?.trim() === "Cancel")
        ?.click();
    });
    rmSync(drafts, { recursive: true, force: true });
    rmSync(kept(), { recursive: true, force: true });
    // The workspace's `themes/` goes too when Keep made it and nothing else is in it.
    const themes = join(plane, "workspaces", WORKSPACE, "themes");
    if (existsSync(themes) && readdirSync(themes).length === 0) rmSync(themes, { recursive: true });
    await ask("theme_choose", { plane, workspace: WORKSPACE, theme: null, appearance: null });
    await endChatsStartedHere([chat]);
    if (was === null) await focusThePlaneRoot();
    else await focusWorkspace(was);
  });

  it("is drawn in its chat's workspace and named on the strip, and Compare shows the kept theme while held", async () => {
    const before = await surfaceDrawn();
    draft(join(drafts, FOLDER), FIRST);
    await untilDrawn(FIRST, "a draft saved");
    await untilStripSays(`Theme draft — ${FOLDER}, edited by`);
    await onStrip("Compare", ["pointerdown"]);
    await untilDrawn(before, "Compare held");
    await onStrip("Compare", ["pointerup"]);
    await untilDrawn(FIRST, "Compare let go");
  });

  it("is not drawn when a save breaks a hard floor, and its last good version stays", async () => {
    draft(join(drafts, FOLDER), SECOND);
    await untilDrawn(SECOND, "a second save");
    draft(join(drafts, FOLDER), UNREADABLE, { "surface.sunken": "#f2f2f2" });
    await untilStripSays("primary text is readable on every surface");
    expect(await surfaceDrawn()).toBe(SECOND);
  });

  it("is dropped at once by the revert key", async () => {
    const mac = await browser.execute(() => navigator.platform.startsWith("Mac"));
    await browser.execute((onAMac: boolean) => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Backspace",
          shiftKey: true,
          metaKey: onAMac,
          ctrlKey: !onAMac,
          bubbles: true,
        }),
      );
    }, mac);
    await untilNoStrip();
    expect(existsSync(join(drafts, FOLDER))).toBe(false);
  });

  it("is deleted by Revert", async () => {
    draft(join(drafts, FOLDER), FIRST);
    await untilStripSays(`Theme draft — ${FOLDER}`);
    await onStrip("Revert");
    await untilNoStrip();
    expect(existsSync(join(drafts, FOLDER))).toBe(false);
  });

  it("is kept into the workspace's themes by Keep, which asks once who draws it", async () => {
    draft(join(drafts, FOLDER), FIRST);
    await untilStripSays(`Theme draft — ${FOLDER}`);
    await onStrip("Keep");
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const asking = document.querySelector('[role="alertdialog"]');
          const mine = [...(asking?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find(
            (one) => one.textContent?.trim() === "Only for me",
          );
          mine?.click();
          return mine !== undefined;
        }),
      { timeout: DRAWN_WITHIN, interval: 100, timeoutMsg: "Keep never asked who draws it" },
    );
    await untilNoStrip();
    expect(existsSync(join(kept(), "theme.json"))).toBe(true);
    expect(existsSync(join(drafts, FOLDER))).toBe(false);
    await untilDrawn(FIRST, "the kept theme, chosen for this workspace");
  });

  it("is asked about when its chat closes, and Revert deletes it", async () => {
    draft(join(drafts, FOLDER), SECOND);
    await untilStripSays(`Theme draft — ${FOLDER}`);
    await bringChatForward(chat?.session ?? -1);
    await browser.execute((selected: string) => {
      document
        .querySelector(selected)
        ?.closest("span.tab")
        ?.querySelector<HTMLElement>("button.closer:not([data-minimise])")
        ?.click();
    }, SELECTED_TAB);
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const asking = document.querySelector('[role="alertdialog"]');
          if (!asking?.textContent?.includes("theme draft")) return false;
          // The answer's words are its label's, which names the radio by its id.
          const label = [...asking.querySelectorAll<HTMLLabelElement>("label")].find(
            (one) => one.textContent?.trim() === "Revert",
          );
          const revert = label === undefined ? null : document.getElementById(label.htmlFor);
          revert?.click();
          const close = [...asking.querySelectorAll<HTMLButtonElement>("button")].find(
            (one) => one.textContent?.trim() === "Close",
          );
          // Off until the draft is answered: unavailable, and kept in reach for its reason.
          if (
            close === undefined ||
            close.disabled ||
            close.getAttribute("aria-disabled") === "true"
          )
            return false;
          close.click();
          return true;
        }),
      {
        timeout: 20_000,
        interval: 100,
        timeoutMsg: "closing the chat never asked about its draft",
      },
    );
    await browser.waitUntil(() => !existsSync(join(drafts, FOLDER)), {
      timeout: DRAWN_WITHIN,
      interval: 100,
      timeoutMsg: "Revert on close left the draft",
    });
    chat = undefined;
  });
});
