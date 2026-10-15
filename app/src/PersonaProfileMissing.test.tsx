// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ViewPane } from "./Views";
import type { PersonaProfile, ViewAnswer } from "./bindings";
import type { ViewRef } from "./tabs";

/**
 * **A teammate opening a persona whose profile they do not have** (#2039, spec #2030 story 21,
 * ADR 0095 §1): the persona's tab offers to make a profile of that name on this machine, by the
 * same command Settings › You › Profiles' Add profile runs. Nothing is made until the person
 * presses Create.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const QA: ViewRef = { from: null, view: "persona", key: "qa" };

const VIEW: ViewAnswer = {
  kind: "answered",
  blocks: [{ kind: "note", text: "Tests the release.", tone: "default" }],
  took_ms: 3,
  overreach: null,
};

/** The core: the persona's view, what its definition names, and every other call recorded. */
function core(profile: PersonaProfile, create?: (args: Record<string, unknown>) => unknown) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "open_view") return VIEW;
    if (cmd === "persona_profile") return profile;
    if (cmd === "machine_profile_create") return create?.(given);
    return undefined;
  });
  return asked;
}

function draw() {
  render(
    <ViewPane
      plane={PLANE}
      view={QA}
      title="qa"
      workspace={undefined}
      waits={false}
      offered={[]}
      onOpenView={() => {}}
      onAsk={() => {}}
      onVaultChanged={() => {}}
    />,
  );
}

const MISSING: PersonaProfile = {
  named: "qa-bot",
  inherited_from: null,
  offered: ["claude", "codex", "opencode"],
};

const made = (name: string, kind: string) => ({
  copied: [],
  profile: {
    name,
    kind,
    harness: "Claude Code",
    origin: "made",
    folder: `/home/dev/.local/share/purlis/profiles/${name}`,
    imported_at: 1,
    imported_from: "/home/dev/.claude",
    signs_in: "through Claude Code's own sign-in, on this Mac",
    standing: "signed-out",
    plans: [],
  },
});

describe("a persona whose profile this machine does not have", () => {
  it("offers to create a profile of that name, and makes nothing until Create is pressed", async () => {
    const asked = core(MISSING, (args) => made(String(args.name), String(args.kind)));
    draw();

    const notice = await screen.findByRole("status", { name: /qa-bot/ });
    expect(notice).toHaveTextContent(
      "qa names the profile qa-bot, which this machine does not have.",
    );
    await userEvent.click(within(notice).getByRole("button", { name: "Create profile qa-bot…" }));
    // Opening the form makes nothing.
    expect(asked.filter((one) => one.cmd === "machine_profile_create")).toEqual([]);

    await userEvent.click(screen.getByRole("button", { name: "Create profile" }));

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "machine_profile_create")).toEqual([
        {
          cmd: "machine_profile_create",
          args: { name: "qa-bot", kind: "claude", sandboxOnly: false },
        },
      ]),
    );
  });

  it("asks the persona's view again once the profile is made, so it says what this machine now has", async () => {
    let offered = MISSING.offered;
    const asked: string[] = [];
    mockIPC((cmd, args) => {
      asked.push(cmd);
      if (cmd === "open_view") return VIEW;
      if (cmd === "persona_profile") return { ...MISSING, offered };
      if (cmd === "machine_profile_create") {
        const given = args as { name: string; kind: string };
        offered = [...offered, given.name];
        return made(given.name, given.kind);
      }
      return undefined;
    });
    draw();

    const notice = await screen.findByRole("status", { name: /qa-bot/ });
    await userEvent.click(within(notice).getByRole("button", { name: "Create profile qa-bot…" }));
    await userEvent.click(screen.getByRole("button", { name: "Create profile" }));

    await waitFor(() => expect(screen.queryByRole("status", { name: /qa-bot/ })).toBeNull());
    expect(asked.filter((cmd) => cmd === "open_view").length).toBeGreaterThan(1);
    // What it made, as Add profile says it, until it is dismissed.
    expect(
      screen.getByText(
        "Made the profile qa-bot for this machine and found no settings to import in /home/dev/.claude. Its first chat asks you to sign it in.",
      ),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText(/Made the profile qa-bot/)).toBeNull();
  });

  it("says the core's refusal under the form, and keeps the offer", async () => {
    core(MISSING, () => {
      throw "a profile called qa-bot is declared by this project already";
    });
    draw();

    const notice = await screen.findByRole("status", { name: /qa-bot/ });
    await userEvent.click(within(notice).getByRole("button", { name: "Create profile qa-bot…" }));
    await userEvent.click(screen.getByRole("button", { name: "Create profile" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "a profile called qa-bot is declared by this project already",
    );
    expect(screen.getByRole("status", { name: /qa-bot/ })).toBeInTheDocument();
  });
});

describe("a persona whose profile this machine has, or that names none", () => {
  it.each<[string, PersonaProfile]>([
    ["names one the project offers", { named: "codex", inherited_from: null, offered: ["codex"] }],
    ["names none", { named: null, inherited_from: null, offered: ["claude"] }],
  ])("offers nothing to create when it %s", async (_, profile) => {
    const asked = core(profile);
    draw();

    await screen.findByText("Tests the release.");
    await waitFor(() => expect(asked.some((one) => one.cmd === "persona_profile")).toBe(true));
    expect(screen.queryByRole("button", { name: /Create profile/ })).toBeNull();
  });
});
