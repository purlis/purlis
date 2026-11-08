// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReportConsent } from "./bindings";
import { ReportCrashNotice } from "./ReportCrashNotice";
import { forgetReportConsent, UNANSWERED } from "./reportAnswers";
import { reportsGroup } from "./settings/reports";

/**
 * **A crash after Not now asks again, once** (#5700): against a stood-in core that keeps what it
 * is told, as the core's Reports leaves it after refusing a crash for someone who chose Not now
 * (`crash_notice: "due"`). The Notice stands until it is dismissed or answered, and is never
 * raised again after either.
 */

const NOT_NOW: ReportConsent = { ...UNANSWERED, asked: true };
const CRASHED: ReportConsent = { ...NOT_NOW, crash_notice: "due" };

let kept: ReportConsent = UNANSWERED;
let reads = 0;
const keeps = vi.fn();

beforeEach(() => {
  keeps.mockReset();
  forgetReportConsent();
  mockIPC((cmd, args) => {
    if (cmd === "report_consent") {
      reads += 1;
      return kept;
    }
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

const onOpenSettingsAt = vi.fn();
const notice = () => screen.findByRole("status", { name: /purlis crashed when it last ran/ });
const noNotice = () =>
  expect(screen.queryByRole("status", { name: /purlis crashed/ })).not.toBeInTheDocument();

/** A launch with what the core keeps now: what the last one read is forgotten. It settles once
 *  the core has said what it keeps and the window has drawn that. */
async function launch(): Promise<void> {
  cleanup();
  forgetReportConsent();
  const before = reads;
  render(<ReportCrashNotice onOpenSettingsAt={onOpenSettingsAt} />);
  await waitFor(() => expect(reads).toBe(before + 1));
  await act(async () => {
    await Promise.resolve();
  });
}

describe("the crash Notice for someone who chose Not now", () => {
  it("is raised after a crash purlis caught, with a way to turn crash reports on", async () => {
    kept = CRASHED;
    await launch();

    const said = await notice();
    expect(said).toHaveTextContent("Crash reports are off");
    expect(screen.getByRole("button", { name: "Turn on crash reports" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Reports settings" })).toBeInTheDocument();
  });

  it("turns crash reports on, and is never raised again", async () => {
    kept = CRASHED;
    await launch();
    await notice();

    await userEvent.click(screen.getByRole("button", { name: "Turn on crash reports" }));

    await waitFor(() =>
      expect(kept).toEqual({ ...NOT_NOW, crash_reports: true, crash_notice: "settled" }),
    );
    noNotice();
    await launch();
    noNotice();
  });

  it("dismissed, says it will not ask again, and is never raised again", async () => {
    kept = CRASHED;
    await launch();
    await notice();
    const dismiss = screen.getByRole("button", { name: "Dismiss" });

    await userEvent.hover(dismiss);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("purlis will not ask again");
    await userEvent.click(dismiss);

    await waitFor(() => expect(kept).toEqual({ ...NOT_NOW, crash_notice: "settled" }));
    noNotice();
    await launch();
    noNotice();
  });

  it("opens Settings › You › Reports at the Crash reports row from its link", async () => {
    kept = CRASHED;
    onOpenSettingsAt.mockReset();
    await launch();
    await notice();

    await userEvent.click(screen.getByRole("button", { name: "Open Reports settings" }));

    expect(onOpenSettingsAt).toHaveBeenCalledExactlyOnceWith("you.reports", "you.reports.crash");
    const group = reportsGroup();
    expect(group.id).toBe("you.reports");
    expect(group.settings.find((row) => row.id === "you.reports.crash")?.label).toBe(
      "Crash reports",
    );
  });

  it("is not raised once crash reports were answered in Settings", async () => {
    kept = { ...CRASHED, crash_reports: false };
    await launch();

    noNotice();
  });

  it("is not raised while no crash was caught", async () => {
    kept = NOT_NOW;
    await launch();

    noNotice();
  });
});
