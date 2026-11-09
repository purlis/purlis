// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactCompared, ArtifactHistory as History, ArtifactOpenedAsOf } from "./bindings";
import { ArtifactHistory, asOfSaid, versionSaid, type AsOf } from "./ArtifactHistory";

const HISTORY: History = {
  kept_most: 20,
  versions: [
    {
      id: "data:001700004000-00",
      at: "2026-11-09 10:00 UTC",
      kind: "regenerated",
      by: null,
      said: null,
    },
    {
      id: "template:" + "a".repeat(40),
      at: "2026-11-08 09:00 UTC",
      kind: "template-changed",
      by: "Ada",
      said: "Show milestones",
    },
  ],
};

const COMPARED: ArtifactCompared = {
  template: [{ path: "index.html", how: "changed" }],
  data: [{ path: "data.json › open", before: "4", after: "3" }],
  cut: false,
};

type Call = { cmd: string; args: Record<string, unknown> };

/** The core, answering History's commands, with what each was asked. */
function core(history: History = HISTORY, compared: ArtifactCompared = COMPARED) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "artifact_history") return history;
    if (cmd === "artifact_compare") return compared;
    return null;
  });
  return calls;
}

function draw(onAsOf: (asOf: AsOf) => void = () => {}) {
  render(
    <ArtifactHistory plane="p" workspace="alpha" folder="board" name="Board" onAsOf={onAsOf} />,
  );
}

const asked = (calls: Call[], cmd: string) => calls.filter((call) => call.cmd === cmd);

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

describe("an artifact's History", () => {
  it("says how many Regenerates it keeps, and that they stay on this machine", async () => {
    core();
    draw();
    await userEvent.click(screen.getByRole("button", { name: "History…" }));
    const dialog = await screen.findByRole("dialog", { name: "History of Board" });
    await waitFor(() =>
      expect(dialog).toHaveTextContent(
        "purlis keeps the data of your last 20 Regenerates on this machine, never in the project.",
      ),
    );
  });

  it("shows the newest version as of its date when asked", async () => {
    core();
    const onAsOf = vi.fn();
    draw(onAsOf);
    await userEvent.click(screen.getByRole("button", { name: "History…" }));
    await waitFor(() =>
      expect(screen.getByTestId("artifact-history-version")).toHaveAttribute(
        "data-value",
        HISTORY.versions[0].id,
      ),
    );
    await userEvent.click(screen.getByRole("button", { name: "Show this version" }));
    expect(onAsOf).toHaveBeenCalledWith({ id: HISTORY.versions[0].id });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("compares the version picked with now and names what changed", async () => {
    const calls = core();
    draw();
    await userEvent.click(screen.getByRole("button", { name: "History…" }));
    const compare = await screen.findByRole("button", { name: "Compare" });
    await waitFor(() => expect(compare).toBeEnabled());
    await userEvent.click(compare);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-history-compared")).toHaveTextContent(
        "data.json › open: 4 → 3",
      ),
    );
    expect(screen.getByTestId("artifact-history-compared")).toHaveTextContent("index.html changed");
    expect(asked(calls, "artifact_compare")[0]?.args).toMatchObject({
      from: HISTORY.versions[0].id,
      to: "now",
    });
  });

  it("says when there are no versions yet, and offers neither showing nor comparing", async () => {
    core({ kept_most: 20, versions: [] });
    draw();
    await userEvent.click(screen.getByRole("button", { name: "History…" }));
    await screen.findByTestId("artifact-history-none");
    expect(screen.queryByRole("button", { name: "Show this version" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Compare" })).toBeNull();
  });
});

describe("a version, in words", () => {
  it("names a page change by who saved it and what they said", () => {
    expect(versionSaid(HISTORY.versions[1])).toBe(
      "2026-11-08 09:00 UTC · Page changed by Ada: Show milestones",
    );
    expect(versionSaid(HISTORY.versions[0])).toBe("2026-11-09 10:00 UTC · Regenerated");
  });

  it("says when the page or the data shown is not as it was then", () => {
    const then = (template: string | null, data_at: string | null): ArtifactOpenedAsOf => ({
      opened: {
        label: "artifact-1",
        standing: "as-of",
        changed_by: null,
        generated: true,
        queries: [],
        plane_granted: false,
      },
      at: "2026-11-09 10:00 UTC",
      template,
      data_at,
    });
    expect(asOfSaid(then("abc", "2026-11-09 10:00 UTC"))).toBe("As of 2026-11-09 10:00 UTC");
    expect(asOfSaid(then(null, null))).toBe(
      "As of 2026-11-09 10:00 UTC, with today's page, as none was saved by then; no data was kept by then",
    );
  });
});
