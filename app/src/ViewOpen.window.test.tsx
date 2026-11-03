// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ExtensionView, ViewAnswer } from "./bindings";
import { extensionsChanged } from "./extensionsOn";
import type { ViewRef } from "./tabs";
import { useExtensionViews, ViewPane } from "./Views";

/**
 * **An extension's view in its pane** (#4769): a button on a view opens another view beside
 * this pane; and a view whose extension is removed while it is open keeps its pane and says it
 * is not here any more, with Close pane (prototype `view-open-beside` Q4 A).
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const STATS: ExtensionView = {
  extension: "statistics",
  id: "stats",
  title: "Statistics",
  about: "personas",
};
const OPEN: ViewRef = { from: "statistics", view: "stats", key: "steward" };
const ANSWER: ViewAnswer = {
  kind: "answered",
  blocks: [{ kind: "note", text: "Turns this week: 214", tone: "default" }],
  took_ms: 1,
  overreach: null,
};

/** What the window's survey answers, which a test changes between surveys. */
let surveyed: ExtensionView[] = [STATS];

function core() {
  mockIPC((cmd) => {
    if (cmd === "extension_views") return surveyed;
    if (cmd === "open_view") return ANSWER;
    return undefined;
  });
}

/** The window's survey, beside a pane showing `view`. */
function Window({ view, onClosePane }: { view: ViewRef; onClosePane: () => void }) {
  const offered = useExtensionViews();
  return (
    <ViewPane
      plane={PLANE}
      view={view}
      title="Statistics · steward"
      waits={false}
      offered={offered}
      onOpenView={() => {}}
      onAsk={() => {}}
      onVaultChanged={() => {}}
      pane={4}
      onClosePane={onClosePane}
    />
  );
}

describe("a view whose extension goes while it is open (#4769)", () => {
  it("keeps its pane and says it is not here any more, with Close pane", async () => {
    surveyed = [STATS];
    core();
    const onClosePane = vi.fn();
    render(<Window view={OPEN} onClosePane={onClosePane} />);
    await screen.findByText("Turns this week: 214");

    surveyed = [];
    act(() => extensionsChanged());

    const gone = await screen.findByTestId("view-gone");
    expect(gone).toHaveTextContent("Statistics · steward is not here any more");
    await userEvent.click(screen.getByRole("button", { name: "Close pane" }));
    expect(onClosePane).toHaveBeenCalledTimes(1);
  });

  it("is itself again once the extension is back", async () => {
    surveyed = [STATS];
    core();
    render(<Window view={OPEN} onClosePane={() => {}} />);
    await screen.findByText("Turns this week: 214");
    surveyed = [];
    act(() => extensionsChanged());
    await screen.findByTestId("view-gone");
    surveyed = [STATS];
    act(() => extensionsChanged());
    await waitFor(() => expect(screen.queryByTestId("view-gone")).toBeNull());
  });
});

describe("a view's heading button opens another view beside its pane (#4769)", () => {
  it("hands the window this pane for Open beside", async () => {
    mockIPC((cmd) => (cmd === "open_view" ? { kind: "answered", blocks: [] } : undefined));
    const beside = vi.fn();
    render(
      <ViewPane
        plane={PLANE}
        view={{ from: null, view: "persona", key: "steward" }}
        title="steward"
        waits={false}
        offered={[STATS]}
        onOpenView={() => {}}
        onAsk={() => {}}
        onVaultChanged={() => {}}
        pane={4}
        onOpenViewBeside={beside}
      />,
    );
    await userEvent.click(
      await screen.findByRole("button", {
        name: "Open Statistics · steward beside this pane (⌥-press)",
      }),
    );
    expect(beside).toHaveBeenCalledWith(OPEN, "Statistics · steward", 4);
  });
});
