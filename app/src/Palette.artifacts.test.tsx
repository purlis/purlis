// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Palette } from "./Palette";
import type { Offer, Ran } from "./actions";
import type { ArtifactFound, PlaneId } from "./bindings";

/**
 * **Artifacts in the palette's search** (#5968, ADR 0122): what is typed finds the pinned
 * artifacts of the project in front whose pages say it, in a group of their own after the
 * actions, and a found artifact opens its own tab. The core is mocked here; what it finds, and
 * what it never reads, is `an_artifacts_text_is_found_through_the_search_index.rs`'s.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/projects/alpha" as unknown as PlaneId;

const ready = (id: string, title: string) =>
  ({ id, title, available: true, reason: "", does: { verb: "nothing" } }) satisfies Offer;

const ok = (): Ran => ({ ok: true });

const BOARD: ArtifactFound = { workspace: "alpha", folder: "board", name: "Release board" };

/** The core, answering `artifact_search` with `answer(query)` and recording each ask. */
function core(answer: (query: string) => ArtifactFound[]) {
  const asked: { plane: unknown; query: string }[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "artifact_search") {
      const { plane, query } = args as { plane: unknown; query: string };
      asked.push({ plane, query });
      return answer(query);
    }
    return null;
  });
  return asked;
}

async function open() {
  await userEvent.keyboard("{F2}");
  return screen.findByRole("dialog", { name: "Command palette" });
}

describe("artifacts in the command palette", () => {
  it("finds a pinned artifact by what its page says, after the actions, and opens it", async () => {
    const asked = core((query) => (query === "release" ? [BOARD] : []));
    const onOpen = vi.fn();
    render(
      <Palette
        offers={[ready("chat.new", "New tab")]}
        artifacts={{ plane: PLANE, onOpen }}
        onRun={ok}
      />,
    );
    const palette = await open();
    expect(within(palette).queryByRole("listbox", { name: "Artifacts" })).toBeNull();

    await userEvent.keyboard("release");
    const group = await within(palette).findByRole("listbox", { name: "Artifacts" });
    expect(asked.at(-1)).toEqual({ plane: PLANE, query: "release" });
    const row = within(group).getByRole("option", { name: /Release board/ });
    expect(row).toHaveTextContent("alpha");

    // No action matches, so Enter is aimed at the artifact.
    await waitFor(() => expect(row).toHaveAttribute("aria-selected", "true"));
    await userEvent.keyboard("{Enter}");
    expect(onOpen).toHaveBeenCalledWith(BOARD);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("keeps the actions first, and the arrows reach the artifacts after them", async () => {
    core(() => [BOARD]);
    const onOpen = vi.fn();
    const onRun = vi.fn(ok);
    render(
      <Palette
        offers={[ready("chat.new", "New tab")]}
        artifacts={{ plane: PLANE, onOpen }}
        onRun={onRun}
      />,
    );
    const palette = await open();
    await userEvent.keyboard("new");
    const group = await within(palette).findByRole("listbox", { name: "Artifacts" });
    const action = within(palette).getByRole("option", { name: /New tab/ });
    expect(action).toHaveAttribute("aria-selected", "true");

    await userEvent.keyboard("{ArrowDown}");
    const row = within(group).getByRole("option", { name: /Release board/ });
    await waitFor(() => expect(row).toHaveAttribute("aria-selected", "true"));
    await userEvent.keyboard("{Enter}");
    expect(onRun).not.toHaveBeenCalled();
    expect(onOpen).toHaveBeenCalledWith(BOARD);
  });

  it("asks nothing without a project in front, and draws no group when nothing is found", async () => {
    const asked = core(() => []);
    const { rerender } = render(
      <Palette offers={[]} artifacts={{ plane: undefined, onOpen: vi.fn() }} onRun={ok} />,
    );
    const palette = await open();
    await userEvent.keyboard("release");
    expect(asked).toEqual([]);

    rerender(<Palette offers={[]} artifacts={{ plane: PLANE, onOpen: vi.fn() }} onRun={ok} />);
    await waitFor(() => expect(asked).toEqual([{ plane: PLANE, query: "release" }]));
    expect(within(palette).queryByRole("listbox", { name: "Artifacts" })).toBeNull();
  });
});
