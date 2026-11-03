// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import type { IdentitiesShown, IdentityRow, IdentityStand } from "./bindings";
import { AgentIdentityTab } from "./AgentIdentityTab";

const NONE: IdentitiesShown = {
  rows: [],
  pending: [],
  owners: [{ name: "acme", personal: false }],
  personas: ["steward", "reviewer"],
};

const ROW: IdentityRow = {
  host: "github.com",
  app: "Iv23li",
  personas: ["steward"],
  acts_as: "acme-purlis-agent",
  repos: 12,
  settings_page: "https://github.com/organizations/acme/settings/apps/acme-purlis-agent",
  trouble: null,
  by_hand: false,
};

const STAND: IdentityStand = {
  acts_as: "acme-purlis-agent",
  owner: "acme",
  installed: true,
  workflows: false,
  install_page: "https://github.com/apps/acme-purlis-agent/installations/new",
  settings_page: "https://github.com/organizations/acme/settings/apps/acme-purlis-agent",
};

/** The commands, each answered from `answers`, with what each was asked. */
function fake(answers: Record<string, unknown>) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: args as Record<string, unknown> });
    if (cmd in answers) {
      const said = answers[cmd];
      return typeof said === "function" ? (said as () => unknown)() : said;
    }
    throw new Error(`nothing fakes ${cmd}`);
  });
  return asked;
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Agent identity", () => {
  it("starts with the one action, and what it will do said first", async () => {
    fake({ agent_identities: NONE });
    render(<AgentIdentityTab plane="p" />);
    expect(await screen.findByRole("button", { name: "Create the GitHub App" })).toBeTruthy();
    expect(screen.getByText(/Your own sign-in never reaches a chat/)).toBeTruthy();
    expect(screen.getByText(/create an App for acme/)).toBeTruthy();
  });

  it("makes the App for the owner chosen and then waits on GitHub", async () => {
    const asked = fake({ agent_identities: NONE, agent_identity_create: new Promise(() => {}) });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Create the GitHub App" }));
    expect(await screen.findByRole("heading", { name: "Finish on GitHub" })).toBeTruthy();
    const made = asked.find((one) => one.cmd === "agent_identity_create");
    expect(made?.args).toMatchObject({ owner: "acme", personal: false, workflows: false });
  });

  it("says what stopped it, on the first screen", async () => {
    fake({
      agent_identities: NONE,
      agent_identity_create: () => Promise.reject("GitHub's page wasn't finished in time."),
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Create the GitHub App" }));
    expect(await screen.findByText(/wasn't finished in time/)).toBeTruthy();
  });

  it("resumes at Finish with the exact lines, and adds them on that one press", async () => {
    const asked = fake({
      agent_identities: { ...NONE, pending: [{ app: "Iv23li", stand: STAND, trouble: null }] },
      agent_identity_stand: STAND,
      agent_identity_lines: '[sandbox.personas.steward.tools]\ngh = { mint = "github-app" }',
      agent_identity_finish: { row: ROW, read: "acme/web", to_allow: [] },
    });
    render(<AgentIdentityTab plane="p" />);
    expect(await screen.findByRole("heading", { name: "One last look" })).toBeTruthy();
    expect(await screen.findByText(/github-app/)).toBeTruthy();
    expect(asked.some((one) => one.cmd === "agent_identity_finish")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Finish" }));
    await waitFor(() =>
      expect(asked.some((one) => one.cmd === "agent_identity_finish")).toBe(true),
    );
    const sent = asked.find((one) => one.cmd === "agent_identity_finish");
    expect(sent?.args.shown).toContain("github-app");
    expect(await screen.findByText(/and read acme\/web with it/)).toBeTruthy();
  });

  it("says on the last look when the App may change workflows", async () => {
    const stand = { ...STAND, workflows: true };
    fake({
      agent_identities: { ...NONE, pending: [{ app: "Iv23li", stand, trouble: null }] },
      agent_identity_stand: stand,
      agent_identity_lines: "lines",
    });
    render(<AgentIdentityTab plane="p" />);
    expect(await screen.findByText(/and CI workflows/)).toBeTruthy();
  });

  it("names a persona Finish left for its own Allow", async () => {
    fake({
      agent_identities: { ...NONE, pending: [{ app: "Iv23li", stand: STAND, trouble: null }] },
      agent_identity_stand: STAND,
      agent_identity_lines: "lines",
      agent_identity_finish: { row: ROW, read: null, to_allow: ["steward"] },
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Finish" }));
    expect(
      await screen.findByText(/Give your Allow in Settings › Sandbox for steward/),
    ).toBeTruthy();
  });

  it("shows what a row acts as and on how many repos", async () => {
    fake({ agent_identities: { ...NONE, rows: [ROW] } });
    render(<AgentIdentityTab plane="p" />);
    const row = await screen.findByRole("region", { name: "github.com" });
    expect(within(row).getByText("acme-purlis-agent on 12 repos")).toBeTruthy();
  });

  it("asks before removing, then says what remains on GitHub", async () => {
    const asked = fake({
      agent_identities: { ...NONE, rows: [ROW] },
      agent_identity_remove: { settings_page: ROW.settings_page },
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove" }));
    expect(asked.some((one) => one.cmd === "agent_identity_remove")).toBe(false);
    const dialog = await screen.findByRole("alertdialog");
    await userEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    expect(await screen.findByText(/The App itself stays on GitHub/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Delete it on GitHub" })).toBeTruthy();
  });

  it("starts over at once from GitHub's page, and a late answer changes nothing", async () => {
    let refuse: (why: string) => void = () => undefined;
    const asked = fake({
      agent_identities: NONE,
      agent_identity_create: () => new Promise((_, no) => (refuse = no)),
      agent_identity_create_cancel: null,
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Create the GitHub App" }));
    await userEvent.click(await screen.findByRole("button", { name: "Start over" }));
    expect(await screen.findByRole("button", { name: "Create the GitHub App" })).toBeTruthy();
    expect(asked.some((one) => one.cmd === "agent_identity_create_cancel")).toBe(true);
    refuse("Stopped. No App was kept.");
    await new Promise((later) => setTimeout(later, 0));
    expect(screen.queryByText(/Stopped/)).toBeNull();
  });

  it("keeps the box ticked on a try that failed", async () => {
    const asked = fake({
      agent_identities: NONE,
      agent_identity_create: () => Promise.reject("It stopped."),
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Options" }));
    await userEvent.click(await screen.findByRole("checkbox", { name: /CI workflows/ }));
    await userEvent.click(screen.getByRole("button", { name: "Create the GitHub App" }));
    expect(await screen.findByText("It stopped.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Create the GitHub App" }));
    const tries = asked.filter((one) => one.cmd === "agent_identity_create");
    expect(tries[1]?.args.workflows).toBe(true);
  });

  it("says why Finish didn't finish, and lets it be pressed again", async () => {
    fake({
      agent_identities: { ...NONE, pending: [{ app: "Iv23li", stand: STAND, trouble: null }] },
      agent_identity_stand: STAND,
      agent_identity_lines: "lines",
      agent_identity_finish: () => Promise.reject("GitHub refused a token."),
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Finish" }));
    expect(await screen.findByText("GitHub refused a token.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Finish" }).getAttribute("aria-disabled")).toBeNull();
  });

  it("gives up on an App it waits for, once asked, and says what remains", async () => {
    const stand = { ...STAND, installed: false };
    fake({
      agent_identities: { ...NONE, pending: [{ app: "Iv23li", stand, trouble: null }] },
      agent_identity_stand: stand,
      agent_identity_lines: "lines",
      agent_identity_remove: { settings_page: ROW.settings_page },
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Start over" }));
    const dialog = await screen.findByRole("alertdialog");
    await userEvent.click(within(dialog).getByRole("button", { name: "Start over" }));
    expect(await screen.findByText(/The App itself stays on GitHub/)).toBeTruthy();
  });

  it("finishes an App waiting beside a row", async () => {
    fake({
      agent_identities: {
        ...NONE,
        rows: [ROW],
        pending: [{ app: "Other", stand: STAND, trouble: null }],
      },
      agent_identity_stand: STAND,
      agent_identity_lines: "lines",
    });
    render(<AgentIdentityTab plane="p" />);
    expect(await screen.findByRole("heading", { name: "One last look" })).toBeTruthy();
  });

  it("adds a persona with one tick and no second App", async () => {
    const asked = fake({
      agent_identities: { ...NONE, rows: [ROW] },
      agent_identity_lines: "the line",
      agent_identity_finish: { row: ROW, read: null, to_allow: [] },
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "reviewer" }));
    await waitFor(() => {
      const sent = asked.find((one) => one.cmd === "agent_identity_finish");
      expect(sent?.args).toMatchObject({ personas: ["reviewer"], shown: "the line" });
    });
  });

  it("reads again from where a read failed", async () => {
    let tries = 0;
    fake({
      agent_identities: () => (++tries === 1 ? Promise.reject("It didn't read.") : NONE),
    });
    render(<AgentIdentityTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Read again" }));
    expect(await screen.findByRole("button", { name: "Create the GitHub App" })).toBeTruthy();
  });

  it("names why Create can't be pressed with no personas", async () => {
    fake({ agent_identities: { ...NONE, personas: [] } });
    render(<AgentIdentityTab plane="p" />);
    expect(await screen.findByText(/no personas yet\. Add one/)).toBeTruthy();
  });

  it("offers no Remove on a line written by hand", async () => {
    fake({
      agent_identities: {
        ...NONE,
        rows: [{ ...ROW, by_hand: true, acts_as: null, repos: null, settings_page: null }],
      },
    });
    render(<AgentIdentityTab plane="p" />);
    expect(await screen.findByText(/written by hand/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Remove" })).toBeNull();
    expect(screen.queryByText("Set up")).toBeNull();
    expect(screen.getByRole("button", { name: "Create the GitHub App" })).toBeTruthy();
  });
});
