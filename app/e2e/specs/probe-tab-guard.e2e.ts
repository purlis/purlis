// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// PROBE (#5269, temporary): a spec made to leave a tab on a strip it only passes through. The
// tab guard should fail this file, naming Settings on alpha. Removed before the pull request is
// ready.

import { expect } from "@wdio/globals";
import { focusWorkspace, tabNames, workspaceTabs } from "../reach.js";
import { openProjectSettings } from "../settingsTab.js";

describe("PROBE (#5269): a tab left on a strip the file only passed through", () => {
  it("opens Settings on alpha, leaves it there and goes back", async () => {
    const began = (await workspaceTabs()).find((tab) => tab.selected)?.name ?? "Plane root";
    expect(began).not.toBe("alpha");
    await focusWorkspace("alpha");
    await openProjectSettings();
    expect((await tabNames()).some((name) => name.startsWith("Settings"))).toBe(true);
    await focusWorkspace(began);
  });
});
