// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProjectBrowserSitesNotice } from "./ProjectBrowserSitesNotice";
import type { SandboxState } from "./bindings";

/**
 * The one-time Notice of the project's browser sites (#2443): a site a teammate committed is
 * shown once, before it reaches this machine's browsers, and Got it sends back exactly the
 * sites it named.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const QUIET: SandboxState = {
  on: true,
  offer: false,
  said: null,
  never: [],
  hosts_changed: null,
  presets_changed: null,
  presets: [],
  persona_hosts: [],
  besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
  policy: null,
};

const ADDED: SandboxState = {
  ...QUIET,
  browser_sites_changed: { added: ["news.example.org:443"], removed: ["old.example:443"] },
};

function core(state: SandboxState) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "sandbox_state") return state;
    if (cmd === "acknowledge_project_browser_sites") return QUIET;
    return null;
  });
  return asked;
}

describe("the project's browser sites Notice", () => {
  it("says a teammate's site reaches this machine's browsers only once it was read, and nothing for commands", async () => {
    core(ADDED);
    render(<ProjectBrowserSitesNotice plane={PLANE} onReview={() => {}} />);
    const notice = await screen.findByText(/news\.example\.org:443/);
    expect(notice).toHaveTextContent(/Your browsers reach it once you press Got it/);
    expect(notice).toHaveTextContent(/chats' commands still ask/);
    expect(notice).toHaveTextContent(/No longer for browsers here: old\.example:443/);
  });

  it("sends back exactly the sites it named as added, then goes", async () => {
    const asked = core(ADDED);
    render(<ProjectBrowserSitesNotice plane={PLANE} onReview={() => {}} />);
    await userEvent.click(await screen.findByRole("button", { name: "Got it" }));
    await waitFor(() => expect(screen.queryByText(/news\.example\.org/)).toBeNull());
    expect(asked.filter((one) => one.cmd === "acknowledge_project_browser_sites")).toEqual([
      {
        cmd: "acknowledge_project_browser_sites",
        args: { plane: PLANE, shown: ["news.example.org:443"] },
      },
    ]);
  });

  it("draws nothing when no browser site changed", async () => {
    const asked = core(QUIET);
    const { container } = render(<ProjectBrowserSitesNotice plane={PLANE} onReview={() => {}} />);
    await waitFor(() => expect(asked.some((one) => one.cmd === "sandbox_state")).toBe(true));
    expect(container).toBeEmptyDOMElement();
  });
});
