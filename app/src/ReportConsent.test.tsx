// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReportConsent } from "./bindings";
import { ReportConsentAsk } from "./ReportConsent";
import { forgetReportConsent, UNANSWERED } from "./reportAnswers";

/**
 * **Asked once** (#5688): the first-run question about reports, against a stood-in core that
 * keeps what it is told (`report_consent`, `keep_report_consent`). A fresh machine and a
 * machine updated from a purlis that kept no answer read alike, as nothing kept: both are asked.
 * An answer, or Not now, is kept, and the question is not asked again.
 */

let kept: ReportConsent = UNANSWERED;
const keeps = vi.fn();

beforeEach(() => {
  kept = UNANSWERED;
  keeps.mockReset();
  forgetReportConsent();
  mockIPC((cmd, args) => {
    if (cmd === "report_consent") return kept;
    if (cmd === "keep_report_consent") {
      kept = (args as { consent: ReportConsent }).consent;
      keeps(kept);
      return null;
    }
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const dialog = () => screen.findByRole("dialog", { name: "Help fix purlis" });
const save = () => screen.getByRole("button", { name: "Save" });

/** A new launch: what this one read is forgotten, and the core is asked again. */
function relaunch() {
  cleanup();
  forgetReportConsent();
  render(<ReportConsentAsk />);
}

describe("the first-run question about reports", () => {
  it("is asked on a machine that kept no answer, with nothing chosen", async () => {
    render(<ReportConsentAsk />);

    const asked = await dialog();
    for (const stream of ["Crash reports", "Agent drafts"]) {
      const group = within(asked).getByRole("radiogroup", { name: stream });
      for (const radio of within(group).getAllByRole("radio")) expect(radio).not.toBeChecked();
    }
    expect(save()).toHaveAttribute("aria-disabled", "true");
    expect(within(asked).getByRole("button", { name: "Not now" })).toBeEnabled();
    expect(
      within(asked).getByText(
        "Never sent: your code, your chats, file paths, project or workspace names, or secrets.",
      ),
    ).toBeInTheDocument();
  });

  it("holds Save until both are answered, and Return answers neither", async () => {
    render(<ReportConsentAsk />);
    const asked = await dialog();

    await userEvent.click(within(asked).getByRole("radio", { name: "Send" }));
    expect(save()).toHaveAttribute("aria-disabled", "true");
    await userEvent.keyboard("{Enter}");
    expect(keeps).not.toHaveBeenCalled();

    await userEvent.click(within(asked).getByRole("radio", { name: "Don't allow" }));
    expect(save()).not.toHaveAttribute("aria-disabled");
    await userEvent.keyboard("{Enter}");
    expect(keeps).not.toHaveBeenCalled();
  });

  it("keeps both answers on Save, and is not asked again", async () => {
    render(<ReportConsentAsk />);
    const asked = await dialog();

    await userEvent.click(within(asked).getByRole("radio", { name: "Send" }));
    await userEvent.click(within(asked).getByRole("radio", { name: "Don't allow" }));
    await userEvent.click(save());

    await waitFor(() =>
      expect(keeps).toHaveBeenCalledWith({
        crash_reports: true,
        agent_drafts: false,
        asked: true,
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    relaunch();
    await waitFor(() => expect(kept.asked).toBe(true));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  for (const how of ["Not now", "Esc"]) {
    it(`keeps no answer on ${how}, and is not asked again`, async () => {
      render(<ReportConsentAsk />);
      const asked = await dialog();

      if (how === "Esc") await userEvent.keyboard("{Escape}");
      else await userEvent.click(within(asked).getByRole("button", { name: "Not now" }));

      await waitFor(() =>
        expect(keeps).toHaveBeenCalledWith({
          crash_reports: null,
          agent_drafts: null,
          asked: true,
        }),
      );
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

      relaunch();
      // The core has answered once the first read settles: still no question.
      await new Promise((settle) => setTimeout(settle, 0));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  }

  it("opens sample bytes of each kind before the person answers", async () => {
    render(<ReportConsentAsk />);
    const asked = await dialog();

    await userEvent.click(within(asked).getByRole("button", { name: "See exactly what's sent…" }));

    const samples = await screen.findByRole("dialog", { name: "What a report sends" });
    expect(within(samples).getByText(/"where": "purlis_core::plane::read_layout"/)).toBeVisible();
    expect(within(samples).getByText(/Title: purlis worktree update/)).toBeVisible();
    expect(keeps).not.toHaveBeenCalled();
  });
});
