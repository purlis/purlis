// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The first-run question about reports** (#5688, spec #5683, the prototype's variant D):
 * asked with nothing chosen, Save held until both streams are answered, and "See exactly what's
 * sent…" opening sample bytes. Drawn, held to axe, walked by the keyboard and pictured in every
 * pictured theme cell (`docs/component-stories.md`). The core is faked at its command boundary,
 * keeping what it is told.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReportConsent } from "./bindings";
import { ReportConsentAsk } from "./ReportConsent";
import { forgetReportConsent, UNANSWERED } from "./reportAnswers";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

let kept: ReportConsent = UNANSWERED;

const meta = {
  component: ReportConsentAsk,
  parameters: { layout: "fullscreen" },
  beforeEach: () => {
    kept = UNANSWERED;
    forgetReportConsent();
    mockIPC((cmd, args) => {
      if (cmd === "report_consent") return kept;
      if (cmd === "keep_report_consent") kept = (args as { consent: ReportConsent }).consent;
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof ReportConsentAsk>;

export default meta;
type Story = StoryObj<typeof meta>;

const asked = () => within(document.body).findByRole("dialog", { name: "Help fix purlis" });

/** **Unanswered**: nothing chosen, Save held with its reason, Not now as plain as Save. Tab
 *  walks each choice and reaches Not now; Return answers neither. */
export const Unanswered: Story = {
  play: async ({ userEvent }) => {
    const dialog = await asked();
    const save = within(dialog).getByRole("button", { name: "Save" });
    await expect(save).toHaveAttribute("aria-disabled", "true");
    for (const radio of within(dialog).getAllByRole("radio")) await expect(radio).not.toBeChecked();

    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("radio", { name: "Send" }),
      "Tab reaches the first stream's choice",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(kept.asked).toBe(false);
    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("radio", { name: "Allow" }),
      "Tab goes on to the second stream's choice",
    );
    expectFocusInSight();
  },
};

/** **Both answered**: Save is on, and pressing it keeps both answers and closes the question. */
export const BothAnswered: Story = {
  play: async ({ userEvent }) => {
    const dialog = await asked();
    await userEvent.click(within(dialog).getByRole("radio", { name: "Send" }));
    await userEvent.click(within(dialog).getByRole("radio", { name: "Don't allow" }));
    const save = within(dialog).getByRole("button", { name: "Save" });
    await expect(save).not.toHaveAttribute("aria-disabled");
    save.focus();
    await untilFocused(() => save, "Save takes the keyboard");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() =>
      expect(kept).toEqual({ crash_reports: true, agent_drafts: false, asked: true }),
    );
  },
};

/** **Not now, by Esc**: neither stream answered, and the question is not asked again. */
export const NotNowByEsc: Story = {
  play: async ({ userEvent }) => {
    await asked();
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(kept).toEqual({ crash_reports: null, agent_drafts: null, asked: true }),
    );
    await waitFor(() => expect(within(document.body).queryByRole("dialog")).toBeNull());
  },
};

/** **What is sent**: a sample of each kind of report, its bytes, before anything is answered. */
export const WhatIsSent: Story = {
  play: async ({ userEvent }) => {
    const dialog = await asked();
    const see = within(dialog).getByRole("button", { name: "See exactly what's sent…" });
    see.focus();
    await untilFocused(() => see, "See exactly what's sent… takes the keyboard");
    await userEvent.keyboard("{Enter}");
    const samples = await within(document.body).findByRole("dialog", {
      name: "What a report sends",
    });
    await within(samples).findByText(/"where": "purlis_core::plane::read_layout"/);
    await within(samples).findByText(/Title: purlis worktree update refuses a clean branch/);
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(
        within(document.body).queryByRole("dialog", { name: "What a report sends" }),
      ).toBeNull(),
    );
    await expect(kept.asked).toBe(false);
  },
};
