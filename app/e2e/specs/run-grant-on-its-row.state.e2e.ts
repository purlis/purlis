// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pressGrant } from "../acting.js";
import { endEveryChat, pressAndStart } from "../opening.js";
import {
  RUNS_A_SECRET,
  SECRET_KEY,
  SECRET_RAN,
  SECRET_VAULT,
  WAITING_ON_THE_PERSON,
} from "../harness.js";

/**
 * **The operator's case, in the built app** (#5998, ADR 0121 amended 2026-11-09): a sandboxed
 * persona chat runs `secret exec steward --env P=some-page -- curl …`, where `curl` is no tool the
 * persona declares. Its vault holds the key, so the run asks one thing alone: whether `curl` may
 * use the vault. Before #5998 its options stood behind an Answer text link, and the person saw a
 * row with no way to refuse.
 *
 * Here the person **starts from the ✋ alone** and opens nothing else: the run grant's row says
 * its question and offers **Don't allow** on the row itself. Pressing it **tells the command**:
 * `secret exec` ends refused, and nothing the vault holds reached `curl`.
 *
 * Nothing is faked past the harness: the reporting harness runs the real `secret exec`
 * (`RUNS_A_SECRET`), the app brokers it and holds it on the ask, and the window's answer goes to
 * the real core. A run whose vault lacks its key keeps its value's form behind Answer
 * (`secret-run.state.e2e.ts`).
 *
 * **It leaves the project as it found it**: the vault it registered, the vault's file and the page
 * are removed, and the chat is ended.
 */

/** The title bar's hand, by its test id: it opens the Inbox. */
const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';

/** The Inbox view, wherever the right side's bar draws it. */
const INBOX = '[role="tabpanel"][data-view="asks"]';

/** The run grant's row. */
const ROW = `${INBOX} .inbox-ask[data-source="run-grant"]`;

/** The value the vault holds already. A page's name, not a credential. */
const VALUE = "page-e2e-5998-3b9e";

/** What the page says, which only a `curl` handed {@link VALUE} can print. */
const PAGE = "the page some-page names was fetched";

/** What the chat's pane shows. */
async function paneSaid(): Promise<string> {
  return (await (await $('[data-testid="pane"]')).$(".xterm-rows")).getText();
}

/** The project's directory, from the status line's item (qw357). */
async function planeRoot(): Promise<string> {
  const said = await $('[data-testid="status-project"]');
  await said.waitForDisplayed({ timeout: 30_000 });
  return ((await said.getAttribute("data-directory")) ?? "").trim();
}

describe("a run grant answered on its row (#5998)", function () {
  // The chat's start, and the settle before a press.
  this.timeout(180_000);

  let plane = "";
  /** The local half of the vault registry, as it was, or `null` where there was none. */
  let registry: string | null = null;

  before(async () => {
    // One app for the whole run: a chat an earlier spec left open would ask too.
    await endEveryChat();
    plane = await planeRoot();
    const local = join(plane, ".charter", "vaults.json");
    registry = existsSync(local) ? readFileSync(local, "utf8") : null;
    const vaults = registry ? (JSON.parse(registry) as { vaults?: Record<string, unknown> }) : {};
    // The persona's vault, on this machine alone, holding the key the command is handed: so
    // the run asks only whether curl may use it.
    mkdirSync(join(plane, ".charter", "vaults"), { recursive: true });
    writeFileSync(
      local,
      JSON.stringify({
        ...vaults,
        vaults: {
          ...(vaults.vaults ?? {}),
          [SECRET_VAULT]: {
            provider: "plain-file",
            persona: SECRET_VAULT,
            config: { file: `.charter/vaults/${SECRET_VAULT}-e2e.json` },
          },
        },
      }),
      { mode: 0o600 },
    );
    writeFileSync(
      join(plane, ".charter", "vaults", `${SECRET_VAULT}-e2e.json`),
      JSON.stringify({ [SECRET_KEY]: VALUE }),
      { mode: 0o600 },
    );
    mkdirSync(join(plane, "e2e-pages"), { recursive: true });
    writeFileSync(join(plane, "e2e-pages", VALUE), `${PAGE}\n`);
    writeFileSync(RUNS_A_SECRET, "");
  });

  after(async () => {
    rmSync(RUNS_A_SECRET, { force: true });
    await endEveryChat();
    if (plane === "") return;
    const local = join(plane, ".charter", "vaults.json");
    if (registry === null) rmSync(local, { force: true });
    else writeFileSync(local, registry, { mode: 0o600 });
    const kept = join(plane, ".charter", "vaults");
    if (existsSync(kept))
      for (const name of readdirSync(kept))
        if (name.startsWith(`${SECRET_VAULT}-e2e.json`)) rmSync(join(kept, name), { force: true });
    rmSync(join(plane, "e2e-pages"), { recursive: true, force: true });
  });

  it("shows its question and Don't allow from the ✋ alone, and Don't allow tells the command", async () => {
    await pressAndStart("New tab");
    // The harness holds its output until a line is typed; then it runs the command, which waits.
    const pane = await $('[data-testid="pane"]');
    await pane.click();
    await $(".xterm-helper-textarea").addValue("\n");

    // **From the ✋ alone**: no view is opened by the test.
    await browser.waitUntil(
      async () => {
        const hand = await $(HAND);
        if (!(await hand.isExisting())) return false;
        return /^\d+ things? waits? on you$/.test((await hand.getAttribute("aria-label")) ?? "");
      },
      { timeout: 30_000, interval: 250, timeoutMsg: "the hand never counted the run grant" },
    );
    await $(HAND).click();
    const row = await $(ROW);
    await row.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: "the ✋ did not open Asks on the run grant",
    });

    // **Its question is its title, and its answers stand on the row**: nothing behind Answer.
    await expect(row.$(".inbox-question")).toHaveText(`Allow curl to use vault ${SECRET_VAULT}?`);
    const refuse = await row.$("button=Don't allow");
    await expect(refuse).toBeDisplayed();
    await expect(row.$("button=Allow once")).toBeDisplayed();
    await expect(row.$("button=Answer")).not.toBeExisting();

    await pressGrant(refuse);

    // **The command is told**: `secret exec` ends refused, neither run nor waiting, and the page
    // only the value could name was never fetched.
    await browser.waitUntil(async () => (await paneSaid()).includes(SECRET_RAN), {
      timeout: 60_000,
      interval: 250,
      timeoutMsg: "the command never heard Don't allow",
    });
    const said = await paneSaid();
    const ended = Number(new RegExp(`${SECRET_RAN} (\\d+)`).exec(said)?.[1]);
    expect(ended).not.toBe(0);
    expect(ended).not.toBe(WAITING_ON_THE_PERSON);
    expect(said).not.toContain(PAGE);
    expect(said).not.toContain(VALUE);
    await expect(row).not.toBeExisting();
  });
});
