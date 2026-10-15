// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { MachineProfileRemoval, MachineProfileRow } from "../bindings";
import { profilesGroup } from "./profiles";
import type { LiveSetting } from "./groups";
import { PROFILE_SIGN_IN, type ProfileSignInAsk } from "./links";

/**
 * **Settings › You › Profiles** (#2034): this machine's profiles are listed with when their
 * settings were imported; Add profile hands the name and harness to the core; Import again
 * imports again; Remove… asks first with what goes, and removes only what it showed; an adopted
 * profile's question says its folder stays. Each row says how its profile signs in, where, and
 * what its harness says (#2035); Sign in asks the project's window to sign it in.
 */

let profiles: MachineProfileRow[];
let sent: { cmd: string; args: Record<string, unknown> }[];

function row(name: string, more: Partial<MachineProfileRow> = {}): MachineProfileRow {
  return {
    name,
    kind: "claude",
    harness: "Claude Code",
    origin: "made",
    folder: `/data/profiles/${name}/config`,
    imported_at: 1_760_000_000,
    imported_from: "/home/dev/.claude",
    signs_in: "through Claude Code's own sign-in, on this Mac",
    standing: "signed-in",
    plans: [],
    ...more,
  };
}

function removal(of: MachineProfileRow): MachineProfileRemoval {
  return of.origin === "adopted"
    ? {
        name: of.name,
        adopted: true,
        folder: of.folder,
        conversations: [],
        says: `purlis forgets the profile '${of.name}'. Its folder, ${of.folder}, is the harness's own and stays.`,
      }
    : {
        name: of.name,
        adopted: false,
        folder: `/data/profiles/${of.name}`,
        conversations: ["config/projects/-a/one.jsonl"],
        says: `Removing the profile '${of.name}' deletes its folder, with its one conversation.`,
      };
}

beforeEach(() => {
  profiles = [];
  sent = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    sent.push({ cmd, args: given });
    switch (cmd) {
      case "machine_profiles":
        return structuredClone(profiles);
      case "machine_profile_create": {
        const made = row(String(given.name), { kind: String(given.kind) });
        profiles.push(made);
        return { profile: made, copied: ["settings.json", "CLAUDE.md"], skipped: [] };
      }
      case "machine_profile_import": {
        const again = profiles.find((one) => one.name === given.name);
        return { profile: again, copied: ["settings.json"], skipped: [] };
      }
      case "machine_profile_removal": {
        const of = profiles.find((one) => one.name === given.name);
        if (of === undefined) throw "this machine has no profile named that.";
        return removal(of);
      }
      case "machine_profile_remove":
        profiles = profiles.filter((one) => one.name !== given.name);
        return null;
      default:
        return null;
    }
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

/** One of the group's rows, drawn as its control. */
function Row({ at, plane = "/p" }: { at: number; plane?: string | null }) {
  const setting = profilesGroup(plane ?? undefined).settings[at] as LiveSetting;
  const { control, error } = setting.useControl();
  return (
    <>
      {control({ id: `row-${at}`, labelledBy: `row-${at}-label` })}
      {error?.map((one) => (
        <p key={one}>{one}</p>
      ))}
    </>
  );
}

describe("Settings › You › Profiles", () => {
  it("lists each profile with when its settings were imported", async () => {
    profiles = [row("work"), row("mine", { origin: "adopted", imported_at: null })];
    render(<Row at={0} />);
    expect(await screen.findByText("work")).toBeTruthy();
    expect(screen.getByText(/^Claude Code, settings imported /)).toBeTruthy();
    expect(screen.getByText("Claude Code, the harness's own folder")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Import the settings of work again" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Import the settings of mine again" })).toBeNull();
  });

  it("adds a profile of the harness picked, under the name typed", async () => {
    render(
      <>
        <Row at={0} />
        <Row at={1} />
      </>,
    );
    expect(await screen.findByText("This machine keeps no profile of its own yet.")).toBeTruthy();
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "claude-work");
    await userEvent.click(screen.getByRole("button", { name: "Add profile" }));
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "machine_profile_create")?.args).toEqual({
        name: "claude-work",
        kind: "claude",
      }),
    );
    expect(await screen.findByText("claude-work")).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("imported 2 files of settings");
  });

  it("imports a profile's settings again", async () => {
    profiles = [row("work")];
    render(<Row at={0} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Import the settings of work again" }),
    );
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "machine_profile_import")?.args).toEqual({
        name: "work",
      }),
    );
  });

  it("asks before removing, naming what goes, and removes only what it showed", async () => {
    profiles = [row("work")];
    render(<Row at={0} />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove work" }));
    const asked = await screen.findByRole("alertdialog");
    expect(within(asked).getByText(/its one conversation/)).toBeTruthy();
    expect(within(asked).getByText("config/projects/-a/one.jsonl")).toBeTruthy();
    expect(sent.some((one) => one.cmd === "machine_profile_remove")).toBe(false);

    await userEvent.click(within(asked).getByRole("button", { name: "Remove profile" }));

    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "machine_profile_remove")?.args).toEqual({
        name: "work",
        shown: removal(row("work")),
      }),
    );
    await waitFor(() => expect(screen.queryByText("work")).toBeNull());
  });

  it("removes nothing when the question is cancelled", async () => {
    profiles = [row("work")];
    render(<Row at={0} />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove work" }));
    const asked = await screen.findByRole("alertdialog");
    await userEvent.click(within(asked).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(sent.some((one) => one.cmd === "machine_profile_remove")).toBe(false);
    expect(screen.getByText("work")).toBeTruthy();
  });

  it("says an adopted profile's folder stays", async () => {
    profiles = [row("mine", { origin: "adopted", folder: "/home/dev/.claude", imported_at: null })];
    render(<Row at={0} />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove mine" }));
    const asked = await screen.findByRole("alertdialog");
    expect(within(asked).getByText(/is the harness's own and stays/)).toBeTruthy();
    expect(within(asked).getByRole("button", { name: "Forget profile" })).toBeTruthy();
  });

  it("says how each profile signs in, where, and whether its harness says it is signed in", async () => {
    profiles = [
      row("work", { standing: "signed-out" }),
      row("mine", {
        origin: "adopted",
        imported_at: null,
        signs_in: "Claude Code's own sign-in on this Mac, as it was before purlis",
        standing: null,
      }),
    ];
    render(<Row at={0} />);
    expect(
      await screen.findByText(
        "Signs in through Claude Code's own sign-in, on this Mac. Not signed in: a chat on it does not start until it is.",
      ),
    ).toBeTruthy();
    expect(
      screen.getByText(
        "Signs in with Claude Code's own sign-in on this Mac, as it was before purlis.",
      ),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Sign mine in" })).toBeNull();
  });

  it("asks the project's window to sign a made profile in", async () => {
    profiles = [row("work", { standing: "signed-out" })];
    const asked: ProfileSignInAsk[] = [];
    const hear = (event: Event) => asked.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    try {
      render(<Row at={0} plane="/projects/p" />);
      await userEvent.click(await screen.findByRole("button", { name: "Sign work in" }));
      expect(asked).toEqual([{ plane: "/projects/p", profile: "work" }]);
      expect(sent.some((one) => one.cmd === "type_profile_sign_in")).toBe(false);
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });

  it("names the command that signs a profile in where no project is open", async () => {
    profiles = [row("work", { standing: "signed-out" })];
    render(<Row at={0} plane={null} />);
    expect(await screen.findByText("purlis profile sign-in work")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Sign work in" })).toBeNull();
  });

  it("signs an opencode profile in to the plan picked, and offers no Claude plan", async () => {
    profiles = [
      row("oc", {
        kind: "opencode",
        harness: "opencode",
        standing: "cannot-tell",
        plans: [
          { id: "openai", title: "ChatGPT" },
          { id: "github-copilot", title: "GitHub Copilot" },
        ],
      }),
    ];
    const asked: ProfileSignInAsk[] = [];
    const hear = (event: Event) => asked.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    try {
      render(<Row at={0} plane="/projects/p" />);
      await userEvent.click(
        await screen.findByRole("button", { name: "Sign oc in to GitHub Copilot" }),
      );
      expect(asked).toEqual([{ plane: "/projects/p", profile: "oc", provider: "github-copilot" }]);
      expect(screen.queryByRole("button", { name: "Sign oc in" })).toBeNull();
      expect(screen.queryByRole("button", { name: /Claude/ })).toBeNull();
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });
});
