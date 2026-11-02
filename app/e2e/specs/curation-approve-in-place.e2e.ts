// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import {
  closeTheViewTabsOpenedSince,
  endTheSessionsStartedSince,
  focusWorkspace,
  runFromThePalette,
  tabNames,
} from "../reach.js";
import { projectInFront, runningIn } from "../running.js";

/**
 * **Approve and start, in place, for a curation action** (#3968), in the built app against the
 * real core: a curation action starts its chat in the core on the project's default profile,
 * without the new-chat picker, and the core refuses a profile nobody approved on this machine.
 * The refusal's notice offers Approve and start…, whose question shows the profile's command in
 * the picker's sentence; its answer approves that line and starts the action's chat.
 *
 * The default is a profile this spec declares, named for it alone, so nobody has approved it
 * whichever spec ran first. It runs the `scenario` profile's program. **It leaves the window, the
 * project and its Local file as it found them**; the approval it records names a profile no
 * other spec declares.
 */

const PROFILE = "e2e-curation-approval";
const ACTION = "curate:workspace:alpha/charter/safe-remove";

/** `local`, with a profile `PROFILE` that runs what its `scenario` profile runs, as the default. */
function withTheProfileAsDefault(local: string): string {
  const scenario = /\[harness\.scenario\]\n((?:[^[\n][^\n]*\n|\n)*)/.exec(local);
  if (scenario === null) throw new Error("the project's Local file declares no scenario profile");
  if (!/^default = "scenario"$/m.test(local))
    throw new Error("the project's Local file does not make scenario the default");
  const made = local.replace(/^default = "scenario"$/m, `default = "${PROFILE}"`);
  return `${made.endsWith("\n") ? made : `${made}\n`}\n[harness.${PROFILE}]\n${scenario[1].trimEnd()}\n`;
}

describe("a curation action refused for its profile's approval", function () {
  this.timeout(180_000);

  let root = "";
  let plane = "";
  /** The Local file as it was, or `null` where there was none. */
  let local: string | null = null;
  let wereOpen: string[] = [];
  let wereRunning: number[] = [];

  before(async () => {
    root = await projectRoot();
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
    plane = await projectInFront();
    wereRunning = await runningIn(plane);
    const file = join(root, "charter.local.toml");
    local = existsSync(file) ? readFileSync(file, "utf8") : null;
    writeFileSync(file, withTheProfileAsDefault(local ?? ""));
  });

  after(async () => {
    await endTheSessionsStartedSince(wereRunning);
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    if (root === "") return;
    const file = join(root, "charter.local.toml");
    if (local === null) rmSync(file, { force: true });
    else writeFileSync(file, local);
  });

  it("offers Approve and start, showing what it runs, and starts the action's chat", async () => {
    await runFromThePalette("Safe remove", ACTION);

    // Refused, with the approval offered beside the refusal; nothing started.
    // By its accessible name, as the person and the screen reader get it.
    const offer = await $("aria/Approve and start…");
    await offer
      .waitForClickable({
        timeout: 30_000,
        timeoutMsg: "the refusal offered no Approve and start…",
      })
      .catch(async (why: unknown) => {
        const seen = await browser.execute(() => ({
          text: document.body.innerText.slice(0, 2500),
          said: [...document.querySelectorAll(".said, .refusal, [data-scope='notice']")].map((e) =>
            (e.textContent ?? "").slice(0, 300),
          ),
          approve: [...document.querySelectorAll("button")]
            .filter((b) => (b.textContent ?? "").includes("Approve"))
            .map((b) => {
              const r = b.getBoundingClientRect();
              const at = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
              return {
                text: b.textContent,
                rect: [r.x, r.y, r.width, r.height],
                top: at === null ? null : at.outerHTML.slice(0, 200),
                path: (() => {
                  const chain: string[] = [];
                  for (let e: Element | null = b; e; e = e.parentElement)
                    chain.push(`${e.tagName}.${String(e.className).slice(0, 40)}`);
                  return chain.join(" < ").slice(0, 600);
                })(),
              };
            }),
          blocked: [...document.querySelectorAll("[aria-hidden='true'], [inert]")]
            .filter((e) => e.querySelector("button") !== null)
            .map((e) => `${e.tagName}.${String(e.className).slice(0, 60)}`)
            .slice(0, 20),
          dialogs: [...document.querySelectorAll("[role='dialog'], [role='alertdialog']")].map((e) =>
            (e.textContent ?? "").slice(0, 300),
          ),
          welcome: document.querySelector(".welcome") !== null,
          panes: document.querySelectorAll(".window-notices-at").length,
        }));
        console.log(`DIAG-4604 ${JSON.stringify(seen)}`);
        throw why;
      });
    expect(await runningIn(plane)).toEqual(wereRunning);
    await offer.click();

    // The question shows the command it approves, as the picker does.
    const ask = await $('[role="alertdialog"]');
    await ask.waitForDisplayed({ timeout: 20_000 });
    await expect(ask).toHaveText(new RegExp(`Approve ${PROFILE}\\?`));
    await expect(ask).toHaveText(/The curation action Safe remove starts a chat/);
    await expect(ask.$("p.approve code")).toHaveText(/claude-stand-in/);
    await ask.$("aria/Approve and start").click();

    await browser.waitUntil(
      async () => (await runningIn(plane)).some((one) => !wereRunning.includes(one)),
      { timeout: 30_000, timeoutMsg: "Approve and start started no chat" },
    );
  });
});
