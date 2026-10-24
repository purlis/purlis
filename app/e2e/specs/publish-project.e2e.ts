// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, realpathSync, renameSync } from "node:fs";
import { dirname, join } from "node:path";
import { execFileSync } from "node:child_process";
import { $, browser, expect } from "@wdio/globals";
import { press, untilSays } from "../acting.js";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { aForgeThatPublishes, letThePushGo, pushedCommits } from "../standInForge.js";
import { ask, openFromTheStrip } from "../switching.js";

/**
 * **Publish a project that lives only on this machine, in the built window** (#1820, spec
 * #1810's first seam).
 *
 * A person opens a project that is no repository yet. Its tab says plainly that it lives only on
 * this machine, with Publish to GitHub or GitLab… beside it. They press it, keep #1819's form as
 * it comes (GitHub, the folder's name, their own account, private), and press Publish: what was
 * not committed is committed, the repo is made on the forge, the project's history is pushed,
 * and the repo becomes the project's remote. The line and its Publish are then gone.
 *
 * **The forge is a stand-in** (`standInForge.ts`): the e2e build answers the owner picker and
 * the publish from recordings in the forge contract's shape, and pushes to a bare repository in
 * the run's tree, so no spec makes a repo anywhere real. Each stage's failure and Cancel on both
 * forges is the core's own test against the contract's recordings (`purlis-core`'s
 * `publishproject`).
 *
 * **Its own project, copied into the run's tree**, and let go of at the end: one app process
 * serves the whole run.
 */

const LINE = '[role="note"][aria-label="Only on this machine"]';
const DIALOG = '[role="dialog"]';
const OWNER = '[data-setting="create-project-owner"] [role="combobox"]';
const NAME_BOX = '[data-setting="publish-project-name"] input';

/** This spec's own project: the fixture, renamed, and no repository. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "notebook");
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  anEmptyRecord(root);
  return root;
})();

/** Whether a line that says the project lives only here is on screen, in the project in front. */
async function theLineIsShown(): Promise<boolean> {
  return browser.execute(
    (selector: string) =>
      [...document.querySelectorAll<HTMLElement>(selector)].some((one) => one.checkVisibility()),
    LINE,
  );
}

/** The project's `origin`, as git says it, or "" with none. */
function originOf(root: string): string {
  try {
    return execFileSync("git", ["-C", root, "remote", "get-url", "origin"], {
      env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" },
      encoding: "utf8",
    }).trim();
  } catch {
    return "";
  }
}

describe("publishing a project that lives only on this machine", function () {
  this.timeout(240_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    aForgeThatPublishes("notebook");
    await openFromTheStrip(mine);
  });

  after(async () => {
    letThePushGo();
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
  });

  it("says on the project's tab that it lives only on this machine, with Publish beside it", async () => {
    await browser.waitUntil(theLineIsShown, {
      timeout: 30_000,
      timeoutMsg: "the project's tab never said it lives only on this machine",
    });
    const said = await browser.execute(
      (selector: string) =>
        [...document.querySelectorAll<HTMLElement>(selector)].find((one) => one.checkVisibility())
          ?.textContent ?? "",
      LINE,
    );
    expect(said).toContain("This project lives only on this machine.");
    expect(said).toContain("Publish to GitHub or GitLab…");
  });

  it("publishes it, pushes its history, makes the repo its remote, and the line goes", async () => {
    // The visible line's own Publish: the launch's project, behind, may say the same.
    const pressed = await browser.execute((selector: string) => {
      const line = [...document.querySelectorAll<HTMLElement>(selector)].find((one) =>
        one.checkVisibility(),
      );
      const button = line?.querySelector<HTMLButtonElement>("button");
      button?.click();
      return button !== undefined && button !== null;
    }, LINE);
    expect(pressed).toBe(true);

    await $(DIALOG).waitForExist({ timeout: 20_000, timeoutMsg: "Publish opened no dialog" });
    // #1819's form as it comes: GitHub, the folder's name, the account itself, private.
    await browser.waitUntil(
      async () =>
        (await $(OWNER).isExisting()) && (await $(OWNER).getAttribute("data-value")) === "octocat",
      { timeout: 20_000, timeoutMsg: "the owner picker never offered the account" },
    );
    expect(await $(NAME_BOX).getValue()).toBe("notebook");
    expect(await $(`${DIALOG} input[type="radio"][value="private"]`).isSelected()).toBe(true);

    await press(DIALOG, "Publish");
    await untilSays(DIALOG, "Published to https://github.com/octocat/notebook", true, 90_000);

    // The project's history reached the forge's repository, and the repo is its remote.
    expect(pushedCommits()).toEqual(["Publish notebook with purlis"]);
    expect(existsSync(join(mine, ".git"))).toBe(true);
    expect(originOf(mine)).not.toBe("");

    await press(DIALOG, "Done");
    await browser.waitUntil(async () => !(await theLineIsShown()), {
      timeout: 30_000,
      timeoutMsg: "the line stayed after the project was published",
    });
  });
});
