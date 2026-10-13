// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { PlaneId } from "./bindings";
import { REFUSED_MORE, moreRefusedSaid, useRefusedMore } from "./refusedMore";

/**
 * **"And N more" past the hosts a chat's proxy names** (#1353): the count the core sends as it
 * grows is kept for the chat it is about, and said in words that offer nothing for the hosts
 * it does not name.
 */

const PLANE = "/projects/more" as PlaneId;

afterEach(() => {
  cleanup();
  clearMocks();
});

function Count({ session }: { session: number }) {
  return <p>{useRefusedMore(PLANE, session)}</p>;
}

describe("the hosts a chat's proxy refused past the ones it names", () => {
  it("are counted for their chat alone, and a late older count is kept out", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    render(
      <>
        <Count session={4} />
        <Count session={5} />
      </>,
    );
    expect(screen.getAllByText("0")).toHaveLength(2);

    await act(() => emit(REFUSED_MORE, { plane: PLANE, session: 4, more: 3 }));
    await act(() => emit(REFUSED_MORE, { plane: PLANE, session: 4, more: 2 }));
    await act(() => emit(REFUSED_MORE, { plane: "/projects/other", session: 5, more: 9 }));

    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("0")).toBeInTheDocument();
  });

  it("are said as a count that no Allow on the Notice reaches", () => {
    expect(moreRefusedSaid(0)).toBeUndefined();
    expect(moreRefusedSaid(1)).toBe(
      "1 more host was refused after these, which purlis does not name, so no Allow here reaches it.",
    );
    expect(moreRefusedSaid(4)).toContain("4 more hosts were refused");
  });
});
