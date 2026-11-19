// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SETTINGS_NEAR_THE_LIMIT, SettingsNearTheLimitNotice } from "./ChatNotices";
import { withStartHanded } from "./tabWindows";

/**
 * **A chat whose harness settings near the limit of its auto mode's check says so** (T1.1-71,
 * #6961): past it, auto mode refuses what it would have checked, nobody asked. The line names
 * its cause, says what weighs them and what to do, and has a way out.
 */

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("settings near their harness's limit", () => {
  it("settings_near_the_limit_raise_a_notice_with_a_way_out", async () => {
    const dismissed = vi.fn();
    render(<SettingsNearTheLimitNotice bytes={300_000} onDismiss={dismissed} />);

    const line = document.querySelector(`[data-cause="${SETTINGS_NEAR_THE_LIMIT}"]`);
    expect(line).not.toBeNull();
    expect(SETTINGS_NEAR_THE_LIMIT).toBe("harness-settings-near-limit");
    expect(line?.textContent).toContain("300 KB");
    expect(line?.textContent).toContain("git state and history");
    expect(line?.textContent).toContain("Remove the branch folders you no longer need");

    await userEvent.click(screen.getByRole("button", { name: /dismiss/i }));
    expect(dismissed).toHaveBeenCalledTimes(1);
  });

  it("names allow rules, not git state, between the two tiers", () => {
    render(<SettingsNearTheLimitNotice bytes={200_000} onDismiss={vi.fn()} />);
    const text = document.querySelector(`[data-cause="${SETTINGS_NEAR_THE_LIMIT}"]`)?.textContent;
    expect(text).toContain("200 KB");
    expect(text).toContain("allow rules");
    expect(text).not.toContain("git state");
  });

  it("is kept when a tab window is handed its chat's start again", () => {
    const handed = { notes: [], agentsMd: [], heavySettings: 300 * 1024 };
    expect(withStartHanded(undefined, handed).heavySettings).toBe(300 * 1024);
    expect(withStartHanded({ notes: ["a line"], agentsMd: [] }, handed).heavySettings).toBe(
      300 * 1024,
    );
    expect(
      withStartHanded({ notes: ["a line"], agentsMd: [] }, { notes: [], agentsMd: [] }),
    ).toEqual({ notes: ["a line"], agentsMd: [] });
  });
});
