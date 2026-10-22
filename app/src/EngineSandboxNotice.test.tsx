// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { ENGINE_SANDBOX_NEEDED, type EngineSandboxShown } from "./bindings";
import { EngineSandboxNotice } from "./EngineSandboxNotice";

/**
 * **Browsers that cannot start for the engine's sandbox say what to do** (#2835): the profile's
 * command to copy, and this machine's own Chrome where one is installed.
 */

const COMMAND = "printf '%s\\n' 'abi <abi/4.0>,' | sudo tee /etc/apparmor.d/purlis-engine-here";
const WITH_CHROME: EngineSandboxShown = {
  says: "Browsers cannot start here.",
  command: COMMAND,
  chrome: "/usr/bin/google-chrome",
};

function core(shown: () => EngineSandboxShown | null): { asked: string[] } {
  const asked: string[] = [];
  mockIPC(
    (cmd) => {
      asked.push(cmd);
      if (cmd === "engine_sandbox") return shown();
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked };
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("the notice about the browser engine's sandbox", () => {
  it("is not drawn until a browser was refused for it", async () => {
    const { asked } = core(() => null);

    render(<EngineSandboxNotice />);

    await waitFor(() => expect(asked).toContain("engine_sandbox"));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("comes once a browser is refused, offering this machine's Chrome and the command", async () => {
    let refused = false;
    const { asked } = core(() => (refused ? WITH_CHROME : null));
    render(<EngineSandboxNotice />);
    await waitFor(() => expect(asked).toContain("engine_sandbox"));

    refused = true;
    await act(() => emit(ENGINE_SANDBOX_NEEDED, null));

    const notice = await screen.findByRole("status");
    expect(notice.textContent).toContain("Browsers cannot start here.");
    expect(screen.getByRole("button", { name: "Copy command" })).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Use this machine's Chrome" }));
    await waitFor(() => expect(asked).toContain("use_this_machines_chrome"));
    await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
  });

  it("offers the command alone where this machine has no Chrome", async () => {
    core(() => ({ ...WITH_CHROME, chrome: null }));

    render(<EngineSandboxNotice />);

    await screen.findByRole("status");
    expect(screen.queryByRole("button", { name: "Use this machine's Chrome" })).toBeNull();
    expect(screen.getByRole("button", { name: "Copy command" })).toBeTruthy();
  });
});
