// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { LiveSetting } from "./groups";
import { previewsGroup } from "./previews";

/**
 * **Settings › You › Previews** (#2765): "Skill studio" writes this machine's flag, and
 * the switch shows what the core then says is in force.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

function Row() {
  const setting = previewsGroup().settings[0] as LiveSetting;
  const { control, error } = setting.useControl();
  return (
    <>
      {control({ id: setting.id, label: setting.label, describedBy: undefined })}
      {error?.map((said) => (
        <p key={said}>{said}</p>
      ))}
    </>
  );
}

describe("the Skill studio preview", () => {
  it("turns the machine's flag on and shows it on", async () => {
    let stored = false;
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "skill_studio_on") return stored;
      if (cmd === "set_skill_studio_on") {
        asked.push(args);
        stored = (args as { on: boolean }).on;
        return stored;
      }
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Row />);
    const toggle = screen.getByRole("switch", { name: "Skill studio" });
    expect(toggle).not.toBeChecked();

    await userEvent.click(toggle);

    await waitFor(() => expect(toggle).toBeChecked());
    expect(asked).toEqual([{ on: true }]);
  });

  it("says the core's refusal beside it", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio_on") return true;
      if (cmd === "set_skill_studio_on")
        throw new Error("purlis could not keep it, so nothing was changed: disk full");
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Row />);

    await userEvent.click(screen.getByRole("switch", { name: "Skill studio" }));

    expect(await screen.findByText(/nothing was changed: disk full/)).toBeInTheDocument();
  });
});
