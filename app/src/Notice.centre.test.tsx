// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Notice, NoticeInCentre, NoticeList, NoticeOf } from "./Notice";

/**
 * **A chat in no pane has no band** (#4759, spec #4750): a Notice its pane would draw is said in
 * the Notices centre instead, as one of the list's lines, saying whose it is with Go to it, and
 * its chat is told while it stands, so its row can be marked.
 */

afterEach(cleanup);

function draw(held: (cause: string, held: boolean) => void, onGo = () => {}, shown = true) {
  return render(
    <NoticeList>
      <NoticeInCentre.Provider value={held}>
        <NoticeOf.Provider value={{ whose: "steward 4", onGo }}>
          {shown && (
            <Notice cause="mcp:4" at="pane" onDismiss={() => {}}>
              Its MCP server did not start.
            </Notice>
          )}
        </NoticeOf.Provider>
      </NoticeInCentre.Provider>
    </NoticeList>,
  );
}

describe("a pane's Notice of a chat in no pane (#4759)", () => {
  it("is a line of the Notices centre's list, saying whose it is, with Go to it", async () => {
    const go = vi.fn();
    draw(() => {}, go);

    const line = await screen.findByText(/Its MCP server did not start\./);
    const listed = line.closest('[data-placement="inbox"]');
    expect(listed).not.toBeNull();
    expect(document.querySelector('[data-placement="pane"]')).toBeNull();
    expect(listed).toHaveTextContent("steward 4");
    await userEvent.click(within(listed as HTMLElement).getByRole("button", { name: /Go to/ }));
    expect(go).toHaveBeenCalledOnce();
  });

  it("tells its chat while it stands, and when it goes", () => {
    const held = vi.fn();
    const { rerender } = draw(held);
    expect(held).toHaveBeenLastCalledWith("mcp:4", true);

    rerender(
      <NoticeList>
        <NoticeInCentre.Provider value={held}>
          <NoticeOf.Provider value={{ whose: "steward 4", onGo: () => {} }}>
            {null}
          </NoticeOf.Provider>
        </NoticeInCentre.Provider>
      </NoticeList>,
    );
    expect(held).toHaveBeenLastCalledWith("mcp:4", false);
  });
});
