// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { beforeEach, describe, expect, it } from "vitest";
import type { LiveSetting } from "./groups";
import { projectRunsOnGroup, runsOnGroup } from "./runsOn";

/** Where each level's setting is, as the store answers (#4536). */
const PLANE = "plane-1" as never;
let asked: [string, Record<string, unknown>][];
let pool: string | null;
let here: boolean;
let project: string | null;
let pools: string[];

beforeEach(() => {
  asked = [];
  pool = null;
  here = false;
  project = null;
  pools = ["builds"];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "chats_run_on") return { pool, here, project, pools };
    if (cmd === "project_chats_run_on") return { pool: project, pools };
    asked.push([cmd, given]);
    if (cmd === "set_project_chats_run_on") {
      project = (given.pool as string | null) ?? null;
      return { pool: project, pools };
    }
    if (cmd === "set_chats_run_on") {
      pool = (given.pool as string | null) ?? null;
      here = pool === null;
      return { pool, here, project, pools };
    }
    if (cmd === "follow_project_chats_run_on") {
      pool = null;
      here = false;
      return { pool, here, project, pools };
    }
    return undefined;
  });
});

function Control({ setting }: { setting: LiveSetting }) {
  const shown = setting.useControl();
  return shown.control({ id: "control", describedBy: undefined } as never);
}

describe("Runs on at the project (#4536)", () => {
  const setting = () => projectRunsOnGroup(PLANE).settings[0] as LiveSetting;

  it("sets the project's pool on this machine, and back", async () => {
    render(<Control setting={setting()} />);
    const machine = await screen.findByRole("radio", { name: /This machine/ });
    await waitFor(() => expect(machine).toBeChecked());
    await userEvent.click(screen.getByRole("radio", { name: /Pool builds/ }));
    await waitFor(() =>
      expect(asked).toEqual([["set_project_chats_run_on", { plane: PLANE, pool: "builds" }]]),
    );
    await waitFor(() => expect(screen.getByRole("radio", { name: /Pool builds/ })).toBeChecked());
    await waitFor(() => expect(machine).not.toBeDisabled());
    await userEvent.click(machine);
    await waitFor(() => expect(project).toBeNull());
  });

  it("offers a way to add a pool while there is none", async () => {
    pools = [];
    render(<Control setting={setting()} />);
    expect(await screen.findByRole("button", { name: "Add a pool…" })).toBeInTheDocument();
  });
});

describe("Runs on at a workspace (#4536)", () => {
  const setting = () => runsOnGroup(PLANE, "alpha", true)?.settings[0] as LiveSetting;

  it("follows the project until the person chooses, and says what the project names", async () => {
    project = "builds";
    render(<Control setting={setting()} />);
    const same = await screen.findByRole("radio", { name: /Same as the project/ });
    await waitFor(() => expect(same).toBeChecked());
    expect(same.closest("label") ?? document.body).toHaveTextContent(/Now pool builds/);
  });

  it("keeps This machine apart from following the project", async () => {
    project = "builds";
    render(<Control setting={setting()} />);
    const machine = await screen.findByRole("radio", { name: /^This machine/ });
    await waitFor(() => expect(machine).not.toBeDisabled());
    await userEvent.click(machine);
    await waitFor(() =>
      expect(asked).toEqual([
        ["set_chats_run_on", { plane: PLANE, workspace: "alpha", pool: null }],
      ]),
    );
    await waitFor(() => expect(machine).toBeChecked());
    const same = screen.getByRole("radio", { name: /Same as the project/ });
    await waitFor(() => expect(same).not.toBeDisabled());
    await userEvent.click(same);
    await waitFor(() => expect(asked.at(-1)?.[0]).toBe("follow_project_chats_run_on"));
    await waitFor(() => expect(same).toBeChecked());
  });
});
