// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press, pressGrant } from "../acting.js";
import { focusWorkspace } from "../reach.js";

/**
 * **A link in an artifact's page** (#5956, ADR 0120 §5), in the built app against the real
 * core: an https link written in the page opens the browser only once the person answers a
 * question naming its host and its whole address, Always open keeps the host so the next one
 * opens without asking, and `file:`, `smb:`, another app's scheme and a link the page's script
 * made are refused with a sentence.
 *
 * **What it cannot reach**: the page's own webview, where the click is, and the system browser.
 * The click's report is made by the `e2e` build's `artifact_link_follow` as the page's script
 * makes it, attributed to the page's webview by its label; the click's own capture is proved
 * below the window (`artifact_links.rs`). What the browser was asked to open is read back with
 * `artifact_links_opened`, which in that build notes a link in place of opening it.
 *
 * **It leaves the window as it found it**: the tab it opens it closes, and the files it writes
 * it removes.
 */

const TABS = '[data-strip="Tabs"]';
const GROUP = '[role="toolbar"][aria-label="Artifacts"]';
const DIALOG = '[role="dialog"]';
const NAME = "Release links";
const ASKED = "https://github.com/purlis/purlis/issues?q=is%3Aopen&sort=new";
const AGAIN = "https://github.com/purlis/purlis/pulls";
const PAGE = `<!doctype html><title>${NAME}</title>
<a href="https://github.com/purlis/purlis/issues?q=is%3Aopen&amp;sort=new">Open issues</a>
<a href="${AGAIN}">Pull requests</a>
<a href="file:///etc/passwd">A file</a>
<a href="smb://server/share">A share</a>
<a href="vscode://file/tmp/x">An editor</a>`;
const PINS = `version = 1
pin = [
  { id = "e2e000000002", folder = "release-links", name = "${NAME}", icon = "chart", order = 1 },
]
`;

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

/** The labels of the artifact webviews the app holds now. */
async function artifactWebviews(): Promise<string[]> {
  const all = await ask<{ label: string }[]>("plugin:webview|get_all_webviews");
  return all.map((one) => one.label).filter((label) => /^artifact-\d+$/.test(label));
}

/** What the app opened in place of the browser, so far. */
const opened = () => ask<string[]>("artifact_links_opened");

/** The dialog that says `title`, once it is up. */
async function dialogSaying(title: string) {
  const dialog = $('[role="dialog"]');
  await browser.waitUntil(
    async () => (await dialog.isExisting()) && (await dialog.getText()).includes(title),
    { timeout: 20_000, timeoutMsg: `no dialog saying "${title}" came up` },
  );
  return dialog;
}

/** Waits until no dialog is up. */
async function noDialog(): Promise<void> {
  await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
    timeout: 20_000,
    timeoutMsg: "the dialog did not close",
  });
}

describe("a link in an artifact's page", function () {
  this.timeout(180_000);

  let artifacts = "";
  let label = "";

  before(async () => {
    const plane = (await ask<string[]>("open_planes"))[0];
    artifacts = join(plane, "workspaces", "alpha", "artifacts");
    mkdirSync(join(artifacts, "release-links"), { recursive: true });
    writeFileSync(join(artifacts, "release-links", "index.html"), PAGE);
    writeFileSync(join(artifacts, "pins.toml"), PINS);
    await focusWorkspace("alpha");
    const icon = await $(`${GROUP} button[aria-label="${NAME}"]`);
    await icon.waitForExist({ timeout: 30_000 });
    await icon.click();
    await browser.waitUntil(async () => (await artifactWebviews()).length === 1, {
      timeout: 20_000,
      timeoutMsg: "the artifact's page did not open",
    });
    label = (await artifactWebviews())[0];
  });

  after(async () => {
    if (await $(`${TABS} button[aria-label="Close ${NAME}"]`).isExisting()) {
      await $(`${TABS} button[aria-label="Close ${NAME}"]`).click();
    }
    rmSync(artifacts, { recursive: true, force: true });
  });

  it("asks first, naming its host and its whole address, and opens it on Open", async () => {
    const before = await opened();
    await ask("artifact_link_follow", {
      label,
      href: "https://github.com/purlis/purlis/issues?q=is%3Aopen&sort=new",
    });
    const dialog = await dialogSaying("Open a link to github.com?");
    expect(await dialog.getText()).toContain(ASKED);
    expect(await opened()).toEqual(before);

    await press(DIALOG, "Open");
    await noDialog();
    expect(await opened()).toEqual([...before, ASKED]);
  });

  it("opens nothing on Cancel", async () => {
    const before = await opened();
    await ask("artifact_link_follow", { label, href: AGAIN });
    await dialogSaying("Open a link to github.com?");
    await press(DIALOG, "Cancel");
    await noDialog();
    expect(await opened()).toEqual(before);
  });

  for (const [href, scheme] of [
    ["file:///etc/passwd", "file"],
    ["smb://server/share", "smb"],
    ["vscode://file/tmp/x", "vscode"],
  ] as const) {
    it(`refuses a ${scheme}: link with a sentence, and opens nothing`, async () => {
      const before = await opened();
      await ask("artifact_link_follow", { label, href });
      const dialog = await dialogSaying("Link not opened");
      expect(await dialog.getText()).toContain(
        `Purlis opens only https links from an artifact, so it will not open this ${scheme}: link.`,
      );
      await press(DIALOG, "Close");
      await noDialog();
      expect(await opened()).toEqual(before);
    });
  }

  it("refuses a link the page's script made, though it is https", async () => {
    const before = await opened();
    await ask("artifact_link_follow", { label, href: "https://github.com/elsewhere" });
    const dialog = await dialogSaying("Link not opened");
    expect(await dialog.getText()).toContain("its script made this one");
    await press(DIALOG, "Close");
    await noDialog();
    expect(await opened()).toEqual(before);
  });

  it("keeps the host on Always open, and opens its next link without asking", async () => {
    const before = await opened();
    await ask("artifact_link_follow", { label, href: AGAIN });
    await dialogSaying("Open a link to github.com?");
    // The one grant in the dialog: Always open github.com.
    await pressGrant($(`${DIALOG} [data-variant="grants"]`));
    await noDialog();
    expect(await opened()).toEqual([...before, AGAIN]);

    await ask("artifact_link_follow", { label, href: AGAIN });
    await browser.waitUntil(async () => (await opened()).length === before.length + 2, {
      timeout: 20_000,
      timeoutMsg: "a link to a host kept as always open was not opened",
    });
    expect(await $('[role="dialog"]').isExisting()).toBe(false);
  });
});
