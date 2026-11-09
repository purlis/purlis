// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactRegenerate as About, ArtifactRegenerated } from "./bindings";
import { ArtifactRegenerate } from "./ArtifactRegenerate";

const APPROVED: About = {
  has_command: true,
  refused: null,
  standing: "approved",
  code: "/plane/skills/report",
  fingerprint: "ab".repeat(32),
  last_good: "2026-11-08 09:00 UTC",
};

type Call = { cmd: string; args: Record<string, unknown> };

/** The core, answering Regenerate's commands, with what each was asked. */
function core(about: About | null, ran: ArtifactRegenerated | string = written()) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "artifact_regenerate_about") return about;
    if (cmd === "artifact_regenerate_approve") return null;
    if (cmd === "artifact_regenerate") {
      if (typeof ran === "string") throw ran;
      return ran;
    }
    return null;
  });
  return calls;
}

function written(): ArtifactRegenerated {
  return { written: ["data.json"], failed: null, output: null };
}

function draw() {
  render(<ArtifactRegenerate plane="p" workspace="alpha" folder="board" name="Board" />);
}

const asked = (calls: Call[], cmd: string) => calls.filter((call) => call.cmd === cmd);

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

describe("Regenerate in an artifact's bar", () => {
  it("draws nothing for a pin that names no command", async () => {
    const calls = core({ ...APPROVED, has_command: false });
    draw();
    await waitFor(() => expect(asked(calls, "artifact_regenerate_about")).toHaveLength(1));
    expect(screen.queryByRole("button", { name: "Regenerate" })).toBeNull();
  });

  it("runs an approved command at once and says what it wrote", async () => {
    const calls = core(APPROVED);
    draw();
    const button = await screen.findByRole("button", { name: "Regenerate" });
    await userEvent.click(button);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-regenerate")).toHaveTextContent("Regenerated data.json"),
    );
    expect(asked(calls, "artifact_regenerate_approve")).toHaveLength(0);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("asks first, naming the code it approves, and approves the fingerprint shown", async () => {
    const calls = core({ ...APPROVED, standing: "changed" });
    draw();
    await userEvent.click(await screen.findByRole("button", { name: "Regenerate" }));
    const ask = await screen.findByRole("alertdialog");
    expect(ask).toHaveTextContent("/plane/skills/report");
    expect(ask).toHaveTextContent("The code changed since you approved it.");
    expect(asked(calls, "artifact_regenerate")).toHaveLength(0);

    await userEvent.click(screen.getByRole("button", { name: /Approve and run/ }));
    await waitFor(() => expect(asked(calls, "artifact_regenerate")).toHaveLength(1));
    expect(asked(calls, "artifact_regenerate_approve")[0]?.args).toMatchObject({
      folder: "board",
      fingerprint: APPROVED.fingerprint,
    });
  });

  it("runs nothing when the person cancels the prompt", async () => {
    const calls = core({ ...APPROVED, standing: "new" });
    draw();
    await userEvent.click(await screen.findByRole("button", { name: "Regenerate" }));
    await screen.findByRole("alertdialog");
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(asked(calls, "artifact_regenerate_approve")).toHaveLength(0);
    expect(asked(calls, "artifact_regenerate")).toHaveLength(0);
  });

  it("says a failed run's reason, its last good date and its output", async () => {
    core(APPROVED, {
      written: [],
      failed:
        "Regenerate failed: its command exited with code 1. The data shown is unchanged; last good: 2026-11-08 09:00 UTC",
      output: "TypeError: rows is undefined",
    });
    draw();
    await userEvent.click(await screen.findByRole("button", { name: "Regenerate" }));
    const failed = await waitFor(() => {
      const found = document.querySelector('[data-cause="artifact-regenerate-failed:board"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(failed).toHaveTextContent("last good: 2026-11-08 09:00 UTC");
    expect(within(failed).getByRole("button", { name: "Regenerate again" })).toBeInTheDocument();
    expect(screen.getByTestId("artifact-regenerate-output")).toHaveTextContent(
      "TypeError: rows is undefined",
    );
  });

  it("is off, with the reason, where its command cannot run", async () => {
    core({
      ...APPROVED,
      standing: null,
      refused: "its command names no script",
      fingerprint: null,
    });
    draw();
    const button = await screen.findByRole("button", { name: "Regenerate" });
    expect(button).toBeDisabled();
    expect(screen.getByTestId("artifact-regenerate")).toHaveTextContent(
      "Cannot regenerate: its command names no script",
    );
  });
});
