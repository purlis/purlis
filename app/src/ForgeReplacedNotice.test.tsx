// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { FORGE_REPLACED, ForgeReplacedNotice } from "./ForgeReplacedNotice";
import type { ForgeReplaced } from "./bindings";

/**
 * A project's old forge token lines, which the person's sign-in replaces: said once for the lines
 * it has now, dismissed until they change, and never at all where the core says none.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const LABEL = "Forge token lines replaced by your sign-in";

const TOLD: ForgeReplaced = {
  cause: `${FORGE_REPLACED}:00aa11bb22cc33dd`,
  text: "This project has 2 tool lines that hand a chat a forge token, which purlis no longer does: chats use your sign-in.",
};

/** A core whose sandbox state carries `forge_replaced`. */
function core(forgeReplaced: ForgeReplaced | null) {
  mockIPC((cmd) => (cmd === "sandbox_state" ? { forge_replaced: forgeReplaced } : null));
}

function shown(dismissed = new Set<string>()) {
  const settle = vi.fn();
  const dismiss = vi.fn();
  const onSignIn = vi.fn();
  render(
    <ForgeReplacedNotice
      plane={PLANE}
      dismissed={dismissed}
      dismiss={dismiss}
      settle={settle}
      onSignIn={onSignIn}
    />,
  );
  return { settle, dismiss, onSignIn };
}

describe("a project's old forge token lines", () => {
  it("says what the core says, once, with a way to the sign-in", async () => {
    core(TOLD);
    const { onSignIn } = shown();

    const notice = await screen.findByRole("status", { name: LABEL });
    expect(notice).toHaveTextContent("chats use your sign-in");
    await userEvent.setup().click(screen.getByRole("button", { name: "Open sign-in" }));
    expect(onSignIn).toHaveBeenCalledTimes(1);
  });

  it("is dismissed by its cause, and the dismissal is settled against what stands now", async () => {
    core(TOLD);
    const { settle, dismiss } = shown();

    await screen.findByRole("status", { name: LABEL });
    expect(settle).toHaveBeenCalledWith(FORGE_REPLACED, [TOLD.cause]);
    await userEvent.setup().click(screen.getByRole("button", { name: /dismiss/i }));
    expect(dismiss).toHaveBeenCalledWith(TOLD.cause);
  });

  it("draws nothing for a cause already dismissed", async () => {
    core(TOLD);
    const { settle } = shown(new Set([TOLD.cause]));

    await vi.waitFor(() => expect(settle).toHaveBeenCalled());
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
  });

  it("draws nothing where the core says there are no old lines, and lets the dismissals go", async () => {
    core(null);
    const { settle } = shown();

    await vi.waitFor(() => expect(settle).toHaveBeenCalledWith(FORGE_REPLACED, []));
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
  });
});
