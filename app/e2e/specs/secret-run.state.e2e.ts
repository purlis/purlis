// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, relative } from "node:path";
import { pressGrant } from "../acting.js";
import { endEveryChat, pressAndStart } from "../opening.js";
import { RUNS_A_SECRET, SECRET_KEY, SECRET_RAN, SECRET_VAULT } from "../harness.js";

/**
 * **#5227's steps, in the built app** (#5246, spec #5235, ADR 0112): a sandboxed persona chat
 * runs `secret exec steward --env P=some-page -- curl …`, where `curl` is no tool the persona
 * declares and the vault lacks `some-page`. Before ADR 0112 that took two Inbox items, the Fill
 * and a proposed `secret-need` line in the persona's committed file from "chat 0", and the run
 * was still refused.
 *
 * Here the run raises **one notice**, the person types the value and presses **Fill and allow
 * once**, and **the command runs**: `curl` prints the page the value names, so the value
 * reached it, and it is nowhere in the pane. **No second item** follows, and **nothing the
 * project commits changes**: no persona line, no tool line, no key name. The value is stored in
 * the vault, on this machine.
 *
 * Nothing is faked past the harness: the reporting harness runs the real `secret exec`
 * (`RUNS_A_SECRET`), the app brokers it and holds it on the notice, and the window's answer goes
 * to the real core. A Codex chat's command waits 7.5 s, so the harness runs it again while it
 * says it waits, as a chat the answer nudged would: that is also what shows a retry joins the
 * one notice, and that Allow once outlives the wait it was given after (spec #5235, story 25).
 *
 * **It leaves the project as it found it**: the vault it registered, the vault's file and the
 * page are removed, and the chat is ended.
 */

/** The title bar's hand, by its test id: it opens the Inbox. */
const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';

/** The Inbox view, wherever the right side's bar draws it. */
const INBOX = '[role="tabpanel"][data-view="asks"]';

/** Every item the Inbox lists. */
const ITEMS = `${INBOX} .inbox-ask`;

/** The run's notice. */
const NOTICE = `${INBOX} .inbox-ask[data-source="run-grant"]`;

/** The value the person types. A page's name, not a credential, so the command may say it. */
const VALUE = "page-e2e-5246-7d1c";

/** What the page says, which only a `curl` handed {@link VALUE} can print. */
const PAGE = "the page some-page names was fetched";

/**
 * The Inbox's kinds #5227 is about: a secret's asks, a vault's, and a proposed change to a
 * committed file, where the second approval came from.
 */
const SECRETS_KINDS = ["secret", "secret-fill", "run-grant", "vault", "proposed-change"];

/** The items of {@link SECRETS_KINDS} the Inbox lists, by kind and the start of what they say. */
async function aboutTheSecret(): Promise<string[]> {
  return browser.execute(
    (items: string, kinds: string[]) =>
      [...document.querySelectorAll<HTMLElement>(items)]
        .filter((item) => kinds.includes(item.dataset.source ?? ""))
        .map((item) => `${item.dataset.source}: ${(item.textContent ?? "").slice(0, 160)}`),
    ITEMS,
    SECRETS_KINDS,
  );
}

/** The project's directory, from the status line's item (qw357). */
async function planeRoot(): Promise<string> {
  const said = await $('[data-testid="status-project"]');
  await said.waitForDisplayed({ timeout: 30_000 });
  return ((await said.getAttribute("data-directory")) ?? "").trim();
}

/**
 * Every file of `plane` a commit could carry, by its path and a hash of its bytes: all but the
 * clone's own state (`.charter`, `.purlis`) and git's folder.
 */
function committable(plane: string): Map<string, string> {
  const files = new Map<string, string>();
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (dir === plane && [".charter", ".purlis", ".git"].includes(name)) continue;
      if (statSync(path).isDirectory()) walk(path);
      else
        files.set(
          relative(plane, path),
          createHash("sha256").update(readFileSync(path)).digest("hex"),
        );
    }
  };
  walk(plane);
  return files;
}

/** What changed between two {@link committable} reads: paths made, gone or rewritten. */
function changed(before: Map<string, string>, after: Map<string, string>): string[] {
  const paths = new Set([...before.keys(), ...after.keys()]);
  return [...paths].filter((path) => before.get(path) !== after.get(path)).sort();
}

/** What the chat's pane shows. */
async function paneSaid(): Promise<string> {
  return (await (await $('[data-testid="pane"]')).$(".xterm-rows")).getText();
}

describe("a run that needs a secret (#5227)", function () {
  // The chat's start, a wait the harness sits out once (7.5 s), and the settle before a press.
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
    // The persona's vault, on this machine alone (the local half, which no commit carries), as a
    // plain file in the clone's state, holding nothing yet.
    mkdirSync(join(plane, ".charter"), { recursive: true });
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
    // The vault's file and the rotation record beside it.
    const kept = join(plane, ".charter", "vaults");
    if (existsSync(kept))
      for (const name of readdirSync(kept))
        if (name.startsWith(`${SECRET_VAULT}-e2e.json`)) rmSync(join(kept, name), { force: true });
    rmSync(join(plane, "e2e-pages"), { recursive: true, force: true });
  });

  it("is one notice, Fill and allow once runs it, and nothing reaches what the project commits", async () => {
    const before = committable(plane);

    await pressAndStart("New tab");
    // The harness holds its output until a line is typed; then it runs the command, which waits.
    const pane = await $('[data-testid="pane"]');
    await pane.click();
    await $(".xterm-helper-textarea").addValue("\n");

    await browser.waitUntil(
      async () => {
        const hand = await $(HAND);
        if (!(await hand.isExisting())) return false;
        return /^\d+ things? waits? on you$/.test((await hand.getAttribute("aria-label")) ?? "");
      },
      { timeout: 30_000, interval: 250, timeoutMsg: "the hand never counted the run's notice" },
    );
    await $(HAND).click();
    const notice = await $(NOTICE);
    await notice.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: "the ✋ did not open Asks on the run's notice",
    });

    // **One notice**, through the harness's retry: its first wait ends at 7.5 s and it runs the
    // command again, which joins the notice that is up rather than raising another.
    await browser.pause(10_000);
    const listed = await browser.execute(
      (items: string) =>
        [...document.querySelectorAll<HTMLElement>(items)].map((item) => item.dataset.source ?? ""),
      ITEMS,
    );
    expect(listed).toEqual(["run-grant"]);
    // The program, the key and the vault, as the notice names them.
    await expect(notice).toHaveText(expect.stringContaining("curl"));
    await expect(notice).toHaveText(expect.stringContaining(SECRET_KEY));

    // Its form stands behind its Answer button, as a value is typed there (#5998; a decision
    // alone is answered on its row, `run-grant-on-its-row.state.e2e.ts`): the value, typed in its
    // masked box, and the narrowest scope, which is chosen already.
    await expect(notice).toHaveText(expect.stringContaining(`Needs a value for ${SECRET_KEY}`));
    await notice.$("button=Answer").click();
    await notice.$(`aria/Value of ${SECRET_KEY}`).setValue(VALUE);
    await pressGrant(await notice.$("button=Fill and allow once"));

    // **It asks once more** (ADR 0121 decision 8, #6178): a program Allow hands the vault's value
    // to curl, a vault use, so the first press only asks, under the form, and nothing runs yet.
    // The person reads the question and presses its own Fill and allow once.
    const again = await notice.$(`[role="group"][aria-label^="Allow handing ${SECRET_KEY} to "]`);
    await again.waitForDisplayed({
      timeout: 10_000,
      timeoutMsg: "Fill and allow once did not ask once more before handing the value to curl",
    });
    await expect(again).toHaveText(expect.stringContaining("curl"));
    expect(await paneSaid()).not.toContain(`${SECRET_RAN} 0`);
    await pressGrant(await again.$("button=Fill and allow once"));

    // **The command runs**: curl printed the page the value names, and ended well.
    await browser.waitUntil(async () => (await paneSaid()).includes(`${SECRET_RAN} 0`), {
      timeout: 60_000,
      interval: 250,
      timeoutMsg: "the command never ran after Fill and allow once",
    });
    const said = await paneSaid();
    expect(said).toContain(PAGE);
    expect(said).not.toContain(VALUE);

    // **No second item**: in #5227 a proposal followed the Fill at once. Nothing about a secret,
    // a vault or a proposed change is left, and nothing comes while the person looks a moment
    // longer. The chat's own turn ending once its command ran may list it as waiting on the
    // person (`question`, `terminal`), which is the chat's and not the secret's.
    await browser
      .waitUntil(async () => (await aboutTheSecret()).length === 0, {
        timeout: 20_000,
        interval: 250,
      })
      .catch(async () => {
        throw new Error(
          `an item stayed in the Inbox after the run's notice was answered: ${JSON.stringify(await aboutTheSecret())}`,
        );
      });
    await browser.pause(3_000);
    expect(await aboutTheSecret()).toEqual([]);

    // **Nothing in what the project commits**: no persona line, no tool line, nothing new.
    expect(changed(before, committable(plane))).toEqual([]);
    // The value is stored, in the vault on this machine: its key is listed there.
    const stored = JSON.parse(
      readFileSync(join(plane, ".charter", "vaults", `${SECRET_VAULT}-e2e.json`), "utf8"),
    ) as Record<string, unknown>;
    expect(Object.keys(stored)).toContain(SECRET_KEY);
  });
});
