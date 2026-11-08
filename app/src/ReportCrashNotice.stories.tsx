// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The crash Notice for someone who chose Not now** (#5700, spec #5683, the prototype's
 * InboxCrashNotice): after a crash purlis caught, one line asks again about Crash reports, with
 * Turn on crash reports, Open Reports settings and a Dismiss that says it will not ask again.
 * The core is faked at its command boundary, keeping what it is told, as Reports leaves it after
 * refusing a crash (`crash_notice: "due"`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReportConsent } from "./bindings";
import { ReportCrashNotice } from "./ReportCrashNotice";
import { forgetReportConsent, UNANSWERED } from "./reportAnswers";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

const CRASHED: ReportConsent = { ...UNANSWERED, asked: true, crash_notice: "due" };

let kept: ReportConsent = CRASHED;

const meta = {
  component: ReportCrashNotice,
  args: { onOpenSettingsAt: fn() },
  parameters: { screenshot: "page" },
  beforeEach: () => {
    kept = CRASHED;
    forgetReportConsent();
    mockIPC((cmd, args) => {
      if (cmd === "report_consent") return kept;
      if (cmd === "keep_report_consent") kept = (args as { consent: ReportConsent }).consent;
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof ReportCrashNotice>;

export default meta;
type Story = StoryObj<typeof meta>;

const NAME = "purlis crashed when it last ran";

/**
 * **After a crash**: the line, its fix, its link and its Dismiss. The keyboard walks all three
 * ways out; Dismiss puts it away for good and turns nothing on.
 */
export const AfterACrash: Story = {
  play: async ({ canvasElement }) => {
    const notice = await within(canvasElement).findByRole("status", { name: NAME });
    await expect(notice).toHaveTextContent("Crash reports are off");

    await userEvent.tab();
    await untilFocused(
      () => within(canvasElement).getByRole("button", { name: "Turn on crash reports" }),
      "the keyboard reaches the fix",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => within(canvasElement).getByRole("button", { name: "Open Reports settings" }),
      "Tab goes on to the link",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => within(canvasElement).getByRole("button", { name: "Dismiss" }),
      "Tab goes on to Dismiss",
    );
    expectFocusInSight();
    await pictureNow();

    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(within(canvasElement).queryByRole("status", { name: NAME })).toBeNull(),
    );
    await expect(kept).toEqual({ ...CRASHED, crash_notice: "settled" });
  },
};

/** **Turned on from the line**: Crash reports are on, and the line is gone for good. Nothing
 *  is left standing to picture. */
export const TurnedOn: Story = {
  parameters: { screenshot: false },
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByRole("status", { name: NAME });

    await userEvent.tab();
    await untilFocused(
      () => within(canvasElement).getByRole("button", { name: "Turn on crash reports" }),
      "the keyboard reaches the fix",
    );
    await userEvent.keyboard("{Enter}");

    await waitFor(() =>
      expect(within(canvasElement).queryByRole("status", { name: NAME })).toBeNull(),
    );
    await expect(kept).toEqual({ ...CRASHED, crash_reports: true, crash_notice: "settled" });
  },
};
