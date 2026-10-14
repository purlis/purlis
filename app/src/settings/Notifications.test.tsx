// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { notificationPrefs } from "../notificationPrefs";
import { forgetThisLaunch } from "../regions";
import type { LiveSetting } from "./groups";
import { notificationsGroup } from "./notifications";

/**
 * **Settings › You › Notifications** (#1989): the quiet hours, off until turned on, each time
 * chosen while they are on; and the system's permission, asked for only on the person's Allow.
 */

let sent: string[] = [];
let permission = "granted";

beforeEach(() => {
  forgetThisLaunch();
  sent = [];
  permission = "granted";
  mockIPC((cmd) => {
    sent.push(cmd);
    if (cmd === "notification_permission") return permission;
    if (cmd === "ask_notification_permission") return "granted";
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

function Row({ id }: { id: string }) {
  const setting = notificationsGroup().settings.find((one) => one.id === id) as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id, label: setting.label, describedBy: undefined })}</>;
}

describe("quiet hours", () => {
  it("are off, and their times out of reach, until turned on", async () => {
    render(
      <>
        <Row id="you.notifications.quiet" />
        <Row id="you.notifications.quiet.from" />
      </>,
    );
    const on = screen.getByRole("switch");
    expect(on).not.toBeChecked();
    expect(screen.getByRole("combobox", { name: "Quiet from" })).toBeDisabled();

    await userEvent.click(on);

    expect(notificationPrefs().quiet.on).toBe(true);
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Quiet from" })).toBeEnabled());
  });
});

describe("the system's permission", () => {
  it("is never asked for by drawing the row", async () => {
    render(<Row id="you.notifications.permission" />);
    expect(await screen.findByText(/^Allowed\./)).toBeInTheDocument();
    expect(sent).not.toContain("ask_notification_permission");
  });

  it("is asked for when the person presses Allow, and says what the system answered", async () => {
    permission = "prompt";
    render(<Row id="you.notifications.permission" />);
    const allow = await screen.findByRole("button", { name: "Allow" });
    expect(sent).not.toContain("ask_notification_permission");

    await userEvent.click(allow);

    await vi.waitFor(() => expect(sent).toContain("ask_notification_permission"));
    expect(await screen.findByText(/^Allowed\./)).toBeInTheDocument();
  });

  it("says where to turn it on once the system said no", async () => {
    permission = "denied";
    render(<Row id="you.notifications.permission" />);
    expect(await screen.findByText(/system's notification settings/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Allow" })).toBeNull();
  });
});
