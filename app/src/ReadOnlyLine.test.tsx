import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

import type { PlaneId, ReadOnlyProject } from "./bindings";
import { ReadOnlyLine, useReadOnly } from "./ReadOnlyLine";

/**
 * **A project this purlis may not write says so once, in its tab** (#826, FR-24): which feature
 * is missing and which version has it. The core refuses the writes; the window says why.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane" as PlaneId;
const MISSING: ReadOnlyProject = {
  sentence:
    "this project requires the feature memory-proposals, which purlis 0.8.0 does not have; " +
    "purlis 0.9.0 or later has it. purlis opens the project read-only and writes nothing to " +
    "it. Upgrade purlis: update the app.",
  feature: "memory-proposals",
  since: "0.9.0",
  mendable: false,
};

function Tab({ plane }: { plane: PlaneId }) {
  return <ReadOnlyLine readOnly={useReadOnly(plane)} />;
}

describe("the read-only project line", () => {
  it("says which feature is missing and which version has it", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "project_read_only") {
        asked.push(args);
        return MISSING;
      }
      return null;
    });
    render(<Tab plane={PLANE} />);
    const line = await screen.findByRole("note", { name: "Read-only project" });
    expect(line.textContent).toContain("memory-proposals");
    expect(line.textContent).toContain("purlis 0.9.0 or later has it");
    expect(asked).toEqual([{ plane: PLANE }]);
  });

  it("draws nothing for a project this purlis may write", async () => {
    let asked = false;
    mockIPC((cmd) => {
      if (cmd === "project_read_only") asked = true;
      return null;
    });
    render(<Tab plane={PLANE} />);
    await waitFor(() => expect(asked).toBe(true));
    expect(screen.queryByRole("note")).toBeNull();
  });

  it("draws nothing when the window could not ask", async () => {
    mockIPC(() => {
      throw new Error("no core");
    });
    const { container } = render(<Tab plane={PLANE} />);
    await Promise.resolve();
    expect(container.textContent).toBe("");
  });
});
