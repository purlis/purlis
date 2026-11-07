// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { realpathSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  anEmptyRecord,
  copyFixturePlane,
  THE_RUNS_GIT_CONFIG,
  THE_RUNS_GIT_CONFIG_TEXT,
} from "../harness.js";
import { bandNotice } from "../reading.js";
import { asksTheDoctorAgain, openedWithTheDoctorHeard } from "../switching.js";

/**
 * **A machine with no git identity is told so by a Notice** (#1250), in the built app against
 * the real core and the real doctor.
 *
 * `app/src/DoctorNotice.window.test.tsx` drives the Notice against a mocked core. What only a
 * real window can show is the whole path: git asked through charter's hardened runner, the
 * doctor the window runs, and the finding standing in the doctor's one Notice line (#5645).
 *
 * **Every other spec runs with an identity** (`THE_RUNS_HOME`, D-1250-8), so this one takes it
 * away: it opens a project of its own and lets its doctor answer, so the first-open baseline
 * (#5648) holds nothing to find, then empties the run's own `.gitconfig` — never the runner's —
 * asks the doctor again, which finds no identity, and puts the file back before anything else
 * is asked. One app process serves the whole run, so it also lets its project go again.
 */

const PROJECTS = '[data-strip="Projects"]';

const NAME = "no-identity";

/** The doctor's one Notice line (#5645, `DOCTOR_LINE`), whose list holds the finding. */
const CAUSE = "doctor-finding:*";
/** The finding's row in the line's list, by the doctor row's name. */
const LINE = `[role="tabpanel"][data-view="notices"] [data-cause="${CAUSE}"]`;
const ROW = `${LINE} li.doctor-row[data-row="git identity"]`;

/** This spec's own project: the fixture plane, with nothing to put back on open. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), NAME);
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  anEmptyRecord(root);
  return root;
})();

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

/** The run's identity, as every other spec has it. */
const putTheIdentityBack = () => writeFileSync(THE_RUNS_GIT_CONFIG, THE_RUNS_GIT_CONFIG_TEXT);

describe("a machine with no git identity", function () {
  this.timeout(180_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
  });

  after(async () => {
    putTheIdentityBack();
    const selector = `${PROJECTS} [role="tab"][aria-label="${NAME}"]`;
    if ((await ask<string[]>("open_planes")).includes(mine))
      await ask("close_plane", { plane: mine });
    await browser.waitUntil(async () => !(await $(selector).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "this spec's project's tab stayed on the strip",
    });
    expect((await ask<string[]>("open_planes"))[0]).toBe(first);
  });

  it("is shown the doctor's finding in its Notice line, with its Fix", async () => {
    // Opened with the identity in place and its doctor heard: what a project holds the first
    // time it opens on a machine counts as seen (#5648), and this spec's finding has to be new.
    await openedWithTheDoctorHeard(mine);
    writeFileSync(THE_RUNS_GIT_CONFIG, "");
    try {
      // The doctor asked again, as the person does from its status item: the read this spec is
      // about.
      await asksTheDoctorAgain();

      let said = "";
      await browser.waitUntil(
        async () => {
          const now = await bandNotice(CAUSE);
          if (now.state === "shown") said = now.text;
          return now.state === "shown";
        },
        { timeout: 60_000, timeoutMsg: "the doctor's Notice line never stood" },
      );
      expect(said).toMatch(/Doctor: \d+ findings?/);
      // Its list, opened from the line: the finding, as the Doctor dialog's row says it.
      await $(LINE).$("button=Show findings").click();
      const row = await $(ROW);
      await row.waitForDisplayed({ timeout: 20_000 });
      await expect(row).toHaveText(expect.stringContaining("not set: user.name, user.email"));
      await expect(row.$("button=Fix")).toBeExisting();
    } finally {
      putTheIdentityBack();
    }
  });
});
