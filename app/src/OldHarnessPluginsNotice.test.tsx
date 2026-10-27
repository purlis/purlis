// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { OldHarnessPluginsNotice } from "./OldHarnessPluginsNotice";
import type { OldHarnessPlugins } from "./bindings";
import { OPEN_PROPOSAL_ASK, type OpenProposalAsk } from "./proposedChangeOpen";

/**
 * **The old `[harness_plugins]` keys, said once** (#3467): the project starts as any other, the
 * Notice says the keys are ignored and the harness decides, Remove holds the change for the
 * person to apply and opens it, and Dismiss keeps the Notice away.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const LABEL = "Old harness plugin settings";

const OLD: OldHarnessPlugins = {
  said: "purlis no longer turns other harnesses' plugins on or off per project, so [harness_plugins] in charter.toml is ignored: each harness decides from its own settings.",
  files: ["charter.toml"],
  removable: true,
};

function core(old: OldHarnessPlugins | null) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "old_harness_plugins") return old;
    if (cmd === "remove_old_harness_plugins") return [4];
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("the old harness plugin keys (#3467)", () => {
  it("says nothing for a project without them", async () => {
    const sent = core(null);
    render(<OldHarnessPluginsNotice plane={PLANE} canAnswer />);
    await waitFor(() => expect(sent("old_harness_plugins")).toHaveLength(1));
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
  });

  it("says they are ignored, and Remove holds the change and opens it to apply", async () => {
    const sent = core(OLD);
    const opened: OpenProposalAsk[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent<OpenProposalAsk>).detail);
    window.addEventListener(OPEN_PROPOSAL_ASK, heard);
    render(<OldHarnessPluginsNotice plane={PLANE} canAnswer />);
    const notice = await screen.findByRole("status", { name: LABEL });
    expect(notice).toHaveTextContent(/is ignored: each harness decides/);
    await userEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(sent("remove_old_harness_plugins")).toEqual([{ plane: PLANE }]));
    await waitFor(() => expect(opened).toEqual([{ plane: PLANE, id: 4, path: "charter.toml" }]));
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
    window.removeEventListener(OPEN_PROPOSAL_ASK, heard);
  });

  it("Dismiss keeps the answer, so it is said once", async () => {
    const sent = core(OLD);
    render(<OldHarnessPluginsNotice plane={PLANE} canAnswer />);
    await screen.findByRole("status", { name: LABEL });
    await userEvent.click(screen.getByRole("button", { name: /Dismiss/ }));
    await waitFor(() => expect(sent("dismiss_old_harness_plugins")).toEqual([{ plane: PLANE }]));
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
  });

  it("offers no Remove where only a workspace's manifest holds them", async () => {
    core({ ...OLD, files: ["workspaces/a/workspace.json"], removable: false });
    render(<OldHarnessPluginsNotice plane={PLANE} canAnswer />);
    await screen.findByRole("status", { name: LABEL });
    expect(screen.queryByRole("button", { name: "Remove" })).toBeNull();
  });
});
