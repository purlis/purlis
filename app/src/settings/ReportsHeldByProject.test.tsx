// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReportConsent } from "../bindings";
import { forgetReportConsent, UNANSWERED } from "../reportAnswers";
import { SettingGroup } from "./components";
import { LiveRow, ShownWhile } from "./SettingsTab";
import type { LiveSetting } from "./groups";
import { reportsGroup } from "./reports";

/**
 * **Always send, held by the project** (#5696, spec #5683, Q12): a project whose shared file
 * turns off sending without asking holds a member's Always send off. Their You level shows the
 * row read-only, badged with the level that holds it and saying why; the core is asked for the
 * rule of the project the window is in (`members_may_always_send`).
 */

const PLANE = "p1";
let mayAlwaysSend = true;
let askedFor: unknown;

beforeEach(() => {
  mayAlwaysSend = true;
  askedFor = undefined;
  const kept: ReportConsent = {
    ...UNANSWERED,
    crash_reports: true,
    agent_drafts: true,
    always_send: true,
    asked: true,
  };
  forgetReportConsent();
  mockIPC((cmd, args) => {
    if (cmd === "report_consent") return kept;
    if (cmd === "reports_sent") return [];
    if (cmd === "members_may_always_send") {
      askedFor = (args as { plane: string }).plane;
      return mayAlwaysSend;
    }
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

function Reports({ plane }: { plane?: string }) {
  const group = reportsGroup(plane);
  return (
    <SettingGroup label={group.label} help={group.help}>
      {group.settings.map((one) => (
        <ShownWhile key={one.id} useShown={(one as LiveSetting).useShown}>
          <LiveRow setting={one as LiveSetting} />
        </ShownWhile>
      ))}
    </SettingGroup>
  );
}

const alwaysSend = () => screen.getByRole("radiogroup", { name: "Always send drafts" });

it("a project that turns it off holds Always send off, read-only, badged by the Project level", async () => {
  mayAlwaysSend = false;
  render(<Reports plane={PLANE} />);

  await waitFor(() =>
    expect(within(alwaysSend()).getByRole("radio", { name: "Ask me first" })).toBeChecked(),
  );
  expect(askedFor).toBe(PLANE);
  for (const radio of within(alwaysSend()).getAllByRole("radio")) expect(radio).toBeDisabled();
  const badge = screen.getByText("Project");
  expect(badge).toBeTruthy();
  expect(alwaysSend().getAttribute("aria-describedby") ?? "").not.toBe("");
  const described = (alwaysSend().getAttribute("aria-describedby") ?? "")
    .split(" ")
    .map((id) => document.getElementById(id)?.textContent ?? "")
    .join(" ");
  expect(described).toContain("Your project turns this off");
});

it("a project that lets its members leaves Always send as the person chose it", async () => {
  render(<Reports plane={PLANE} />);
  await waitFor(() =>
    expect(within(alwaysSend()).getByRole("radio", { name: "Send without asking" })).toBeChecked(),
  );
  await waitFor(() => expect(askedFor).toBe(PLANE));
  for (const radio of within(alwaysSend()).getAllByRole("radio")) expect(radio).toBeEnabled();
  expect(screen.queryByText("Project")).toBeNull();
});

it("with no project in front, nothing holds Always send", async () => {
  render(<Reports />);
  await waitFor(() =>
    expect(within(alwaysSend()).getByRole("radio", { name: "Send without asking" })).toBeChecked(),
  );
  expect(askedFor).toBeUndefined();
  expect(screen.queryByText("Project")).toBeNull();
});
