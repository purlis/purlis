// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";

/**
 * **TEMPORARY, for #5609: fails on purpose**, to show on Linux CI what a missed press records.
 * Removed before the pull request is ready.
 *
 * It draws a question like the close question with a veil over its answer, presses the answer
 * from an `after` hook, and puts the question away before it throws, as `closingByTheWindow`
 * does. The failure's record should still name the veil at the answer's centre.
 */
describe("a press that misses (temporary, #5609)", function () {
  this.timeout(60_000);

  it("draws a covered question", async () => {
    await browser.execute(() => {
      const question = document.createElement("div");
      question.id = "probe-5609";
      question.setAttribute("role", "alertdialog");
      question.setAttribute("aria-label", "Close this probe?");
      const answer = document.createElement("button");
      answer.textContent = "Close and end the probe";
      question.append(answer);
      const veil = document.createElement("div");
      veil.id = "veil-5609";
      veil.className = "probe-veil";
      veil.setAttribute("data-state", "open");
      veil.style.position = "fixed";
      veil.style.inset = "0";
      veil.style.zIndex = "2147483647";
      document.body.append(question, veil);
    });
  });

  after(async () => {
    try {
      const answer = await $('[role="alertdialog"]#probe-5609').$("button*=Close and end");
      await answer.waitForClickable({ timeout: 3_000 });
      await answer.click();
    } finally {
      await browser.execute(() => {
        document.getElementById("probe-5609")?.remove();
        document.getElementById("veil-5609")?.remove();
      });
    }
  });
});
