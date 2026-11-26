// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { HIBERNATED_SCREEN_LABEL, HibernatedOr, NONE_KEPT } from "./HibernatedScreen";
import type { ChatHibernation } from "./bindings";

/**
 * Opening a hibernated chat shows what it showed as it was hibernated, read-only, in place of
 * its terminal (SC-4b, #7631); where none was kept it says so.
 */

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const BY_YOU: ChatHibernation = { by: "operator" };

/** A core where chat 7 is `held`, and its kept screen is `kept`. */
function core(held: ChatHibernation | null, kept: string | null) {
  mockIPC((cmd) => {
    if (cmd === "chat_hibernated") return held;
    if (cmd === "hibernated_chat_screen") return kept;
    return null;
  });
}

function pane() {
  return render(
    <HibernatedOr plane={PLANE} session={7}>
      <div data-testid="live-terminal" />
    </HibernatedOr>,
  );
}

describe("a hibernated chat's pane", () => {
  it("shows the screen kept for it, read-only, in place of its terminal", async () => {
    core(BY_YOU, "Deployed to staging.\r\n");
    pane();
    const region = await screen.findByRole("region", { name: HIBERNATED_SCREEN_LABEL });
    await waitFor(() =>
      expect(within(region).getAllByText(/Deployed to staging/).length).toBeGreaterThan(0),
    );
    expect(screen.queryByTestId("live-terminal")).toBeNull();
  });

  it("says no copy was kept where none was", async () => {
    core(BY_YOU, null);
    pane();
    expect(await screen.findByText(NONE_KEPT)).toBeVisible();
    expect(screen.queryByTestId("live-terminal")).toBeNull();
  });

  it("is the live terminal for a chat that is not hibernated", async () => {
    core(null, "never read");
    pane();
    expect(await screen.findByTestId("live-terminal")).toBeInTheDocument();
  });
});
