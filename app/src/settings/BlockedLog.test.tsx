// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SandboxNetwork } from "../bindings";
import { networkGroup } from "./GrantedList";
import type { LiveSetting } from "./groups";

/**
 * **"Blocked lately" is folded into the Blocked log** (R-222, T1-32; Q69, Q264): Settings'
 * Network page has one list of blocks, the Blocked log, and no Blocked lately beside it. Its rows
 * are the core's, read from the event log: here, a network-record block that T1-10 wrote as a
 * block, named by the host its target summary gives (R-220, Q206).
 */

const PLANE = "/home/dev/plane";

/** What the core answers for a fixture network record whose one refusal T1-10 wrote as a block:
 *  a connection purlis's proxy held, kept blocked. */
const NETWORK: SandboxNetwork = {
  on: true,
  open: [],
  blocked: [
    {
      target: "api.example.com:443",
      looked_up: null,
      said: "blocked by the sandbox",
      chat: "fix the build",
      at: 1_790_000_000,
      times: 1,
      reached: false,
      levels: ["you", "project"],
    },
  ],
};

afterEach(() => {
  cleanup();
  clearMocks();
});

/** The Network page's row `id`, drawn on its own. */
function Row({ id }: { id: string }) {
  const setting = networkGroup(PLANE, "charter.toml").settings.find(
    (one) => one.id === `project.sandbox.network.${id}`,
  ) as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id: `r-${id}`, labelledBy: `r-${id}-label` })}</>;
}

describe("the Blocked log", () => {
  it("Blocked lately is folded into the Blocked log", async () => {
    mockIPC((cmd) => {
      if (cmd === "sandbox_network") return NETWORK;
      if (cmd === "sandbox_grants") return [];
      return null;
    });
    const group = networkGroup(PLANE, "charter.toml");
    const labels = group.settings.map((one) => one.label);
    expect(labels).toContain("Blocked log");
    expect(labels).not.toContain("Blocked lately");
    for (const one of [group, ...group.settings])
      expect(`${one.label} ${one.help ?? ""}`).not.toMatch(/blocked lately/i);

    const { container } = render(
      <>
        {["open", "allowed", "blocked"].map((id) => (
          <Row key={id} id={id} />
        ))}
      </>,
    );
    const log = await screen.findByRole("list", { name: "Blocked log" });
    expect(screen.queryByRole("list", { name: "Blocked lately" })).toBeNull();
    expect(container.textContent ?? "").not.toMatch(/blocked lately/i);

    const [row] = within(log).getAllByRole("listitem");
    expect(within(row).getByText("api.example.com:443")).toBeVisible();
    expect(row).toHaveTextContent("Blocked by the sandbox · fix the build");
    expect(
      within(row).getByRole("button", {
        name: "Allow for me on this machine: api.example.com:443",
      }),
    ).toBeVisible();
  });
});
