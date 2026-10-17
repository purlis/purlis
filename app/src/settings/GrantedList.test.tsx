// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, onTestFinished } from "vitest";
import { cleanup, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { commands, type SandboxGrant } from "../bindings";
import { networkGroup, grantSaid } from "./GrantedList";
import type { LiveSetting } from "./groups";
import { useSandboxCommands } from "../sandboxAsked";
import { SETTINGS_ACTION, type SettingsActionAsk } from "./links";

/**
 * **Settings › Sandbox › Network › Allowed** (#1348, renamed by #1662): every Allow at every
 * scope, who allowed it and when, each with Remove, which the core takes out of every later
 * start and audits.
 */

const PLANE = "/home/dev/plane";

const CHAT: SandboxGrant = {
  id: "chat\u001fc1\u001fhost\u001fapi.example.com",
  what: "host",
  target: "api.example.com",
  persona: null,
  level: "chat",
  by: null,
  at: 1_790_000_000,
  chat: "claude 4",
  locked: null,
  waiting: null,
  for_no_persona: false,
  expected: false,
};
const MINE: SandboxGrant = {
  id: "you\u001fwrite\u001f/opt/cache",
  what: "write",
  target: "/opt/cache",
  persona: null,
  level: "you",
  by: null,
  at: null,
  chat: null,
  locked: null,
  waiting: null,
  for_no_persona: false,
  expected: false,
};
const PROJECT: SandboxGrant = {
  id: "project\u001fhost\u001f10.0.0.5:6443",
  what: "host",
  target: "10.0.0.5:6443",
  persona: null,
  level: "project",
  by: "Dana",
  at: 1_790_000_000,
  chat: null,
  locked:
    "10.0.0.5:6443 is not a host policy allows. Locked by policy, set by IT in /etc/purlis/policy.json.",
  waiting: null,
  for_no_persona: false,
  expected: false,
};

afterEach(() => {
  cleanup();
  clearMocks();
});

function Setting({ at }: { at: number }) {
  const setting = networkGroup(PLANE, "charter.toml").settings[at] as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id: `g${at}`, labelledBy: `g${at}-label` })}</>;
}

/** The Allowed list: the Network page's second row. */
const Granted = () => <Setting at={1} />;
/** The folders a block's Allow may name: its fourth. */
const FOLDERS = 3;

describe("the Allowed list's links (#1388)", () => {
  it("links a vault grant to the vault's tab and its persona's", async () => {
    const heard: SettingsActionAsk[] = [];
    const on = (event: Event) => heard.push((event as CustomEvent<SettingsActionAsk>).detail);
    window.addEventListener(SETTINGS_ACTION, on);
    onTestFinished(() => window.removeEventListener(SETTINGS_ACTION, on));
    const VAULT: SandboxGrant = {
      ...MINE,
      id: "you\u001fvault\u001fforge\u001fsteward",
      what: "vault",
      target: "forge",
      persona: "steward",
    };
    mockIPC((cmd) => (cmd === "sandbox_grants" ? [VAULT, CHAT] : null));
    render(<Granted />);

    const list = await screen.findByRole("list", { name: "Allowed" });
    const [vault, chat] = within(list).getAllByRole("listitem");
    await userEvent.click(within(vault).getByRole("button", { name: "Open vault forge" }));
    await userEvent.click(within(vault).getByRole("button", { name: "Show steward" }));

    expect(heard).toEqual([
      { plane: PLANE, action: "vault.open:forge" },
      { plane: PLANE, action: "persona.show:steward" },
    ]);
    // A host names neither.
    expect(within(chat).queryByRole("button", { name: /^(Open vault|Show) / })).toBeNull();
  });
});

describe("the Allowed list off the window (#1927)", () => {
  it("lists what is allowed and offers no Remove where the press could not work", async () => {
    // Remove is the window's alone: a page shown anywhere else cannot make that call, so it
    // draws none and says where the person takes a grant back.
    const held = commands as Record<string, unknown>;
    const was = held.revokeSandboxGrant;
    delete held.revokeSandboxGrant;
    onTestFinished(() => {
      held.revokeSandboxGrant = was;
    });
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      return cmd === "sandbox_grants" ? [CHAT, MINE] : null;
    });
    render(<Granted />);

    const list = await screen.findByRole("list", { name: "Allowed" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    expect(within(list).queryAllByRole("button", { name: /^Remove / })).toHaveLength(0);
    expect(screen.getByText(/Remove one from purlis's own window on this machine\./)).toBeVisible();
    expect(asked).not.toContain("revoke_sandbox_grant");
  });
});

describe("the Allowed list", () => {
  it("lists every scope with who allowed it, and Remove takes one out", async () => {
    const asked: Record<string, unknown>[] = [];
    let held = [CHAT, MINE, PROJECT];
    mockIPC((cmd, args) => {
      if (cmd === "sandbox_grants") return held;
      if (cmd === "revoke_sandbox_grant") {
        asked.push(args as Record<string, unknown>);
        held = held.filter((one) => one.id !== (args as { id: string }).id);
        return held;
      }
      return null;
    });
    render(<Granted />);

    const list = await screen.findByRole("list", { name: "Allowed" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("Reach api.example.com · This chat · allowed by you");
    expect(rows[0]).toHaveTextContent("from claude 4");
    expect(rows[1]).toHaveTextContent(
      "Write /opt/cache and everything in it · This project on this machine · allowed by you",
    );
    expect(rows[2]).toHaveTextContent(
      "Reach 10.0.0.5:6443 · Everyone in the project · committed by Dana",
    );
    // Locked by policy: said, and still yours to take away (#1431); a project host's Remove
    // edits the committed file.
    expect(rows[2]).toHaveTextContent(
      "Not in force. 10.0.0.5:6443 is not a host policy allows. Locked by policy, set by IT in /etc/purlis/policy.json.",
    );
    expect(rows[2]).toHaveTextContent("Removing it edits the committed charter.toml.");
    expect(
      within(rows[2]).getByRole("button", { name: "Remove reaching 10.0.0.5:6443" }),
    ).toBeVisible();

    // What reads the chats left on an older sandbox hears that a sandbox command returned
    // (#1428): the removed folder is kept where no watcher reports a write.
    const heard = renderHook(() => useSandboxCommands());
    const before = heard.result.current;
    await userEvent.click(screen.getByRole("button", { name: "Remove writing /opt/cache" }));
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(2));
    expect(asked).toEqual([{ plane: PLANE, id: MINE.id }]);
    await waitFor(() => expect(heard.result.current).toBeGreaterThan(before));

    // The locked one goes the same way.
    await userEvent.click(screen.getByRole("button", { name: "Remove reaching 10.0.0.5:6443" }));
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(1));
    expect(asked).toEqual([
      { plane: PLANE, id: MINE.id },
      { plane: PLANE, id: PROJECT.id },
    ]);
  });

  it("says when nothing is allowed", async () => {
    mockIPC((cmd) => (cmd === "sandbox_grants" ? [] : null));
    render(<Granted />);
    expect(
      await screen.findByText("Nothing is allowed beyond the Open hosts."),
    ).toBeInTheDocument();
  });

  it("says a refused Remove through a Notice", async () => {
    mockIPC((cmd) => {
      if (cmd === "sandbox_grants") return [MINE];
      if (cmd === "revoke_sandbox_grant")
        throw new Error("purlis did not remove it: it is no longer there.");
      return null;
    });
    render(<Granted />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove writing /opt/cache" }));
    expect(await screen.findByRole("status")).toHaveTextContent("it is no longer there");
  });

  it("lists the folders a block's Allow may name, with Add and Remove", async () => {
    const asked: { cmd: string; args: unknown }[] = [];
    let held = ["/opt/tools"];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "grantable_folders") return { folders: held, dropped: [] };
      if (cmd === "list_grantable_folder") {
        held = [...held, (args as { folder: string }).folder];
        return { folders: held, holds: ["/Users/dev/Library/LaunchAgents"] };
      }
      if (cmd === "unlist_grantable_folder") {
        held = held.filter((one) => one !== (args as { folder: string }).folder);
        return held;
      }
      return null;
    });
    render(<Setting at={FOLDERS} />);
    const list = await screen.findByRole("list", { name: "Folders a block's Allow may name" });
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(1));
    // The set's Field (#626): the row's control, which the row's label names by its id.
    const box = screen.getByRole("textbox");
    expect(box).toHaveAttribute("id", `g${FOLDERS}`);
    expect(box).toHaveClass("ui-field");
    expect(box).toHaveAttribute("placeholder", "/Users/you/tools/cache");
    await userEvent.type(box, "/opt/more");
    await userEvent.click(screen.getByRole("button", { name: "Add folder" }));
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(2));
    // Listed, and warned of what it holds that other programs load code from.
    expect(await screen.findByRole("status")).toHaveTextContent(
      "holds folders other programs load code from",
    );
    await userEvent.click(screen.getByRole("button", { name: "Remove /opt/tools" }));
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(1));
    expect(asked.map((one) => one.cmd)).toContain("list_grantable_folder");
  });

  it("says a project host not committed yet as that, and that removing edits the file", async () => {
    mockIPC((cmd) => (cmd === "sandbox_grants" ? [{ ...PROJECT, by: null, locked: null }] : null));
    render(<Granted />);
    const row = (await screen.findAllByRole("listitem"))[0];
    expect(row).toHaveTextContent("Everyone in the project · not committed yet");
    expect(row).toHaveTextContent("Removing it edits the committed charter.toml.");
  });

  it("says a listed folder the core dropped because it leads elsewhere now", async () => {
    mockIPC((cmd) =>
      cmd === "grantable_folders" ? { folders: [], dropped: ["/Users/dev/tools"] } : null,
    );
    render(<Setting at={FOLDERS} />);
    expect(await screen.findByRole("status")).toHaveTextContent(
      "off this list: it now leads somewhere else",
    );
  });

  it("lists a vault you let a persona's chats use, and removes it by both names (#1430)", async () => {
    const VAULT: SandboxGrant = {
      id: "you\u001fvault\u001fdevops\u001fsteward",
      what: "vault",
      target: "devops",
      persona: "steward",
      level: "you",
      by: null,
      at: null,
      chat: "steward 1",
      locked: null,
      waiting: null,
      for_no_persona: false,
      expected: false,
    };
    expect(grantSaid(VAULT)).toBe(
      "Use vault devops as steward · This project on this machine · allowed by you, from steward 1",
    );
    const asked: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "sandbox_grants") return [VAULT];
      if (cmd === "revoke_sandbox_grant") return [];
      return null;
    });
    render(<Granted />);
    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: "Remove using vault devops as steward" }));
    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "revoke_sandbox_grant",
        args: { plane: PLANE, id: VAULT.id },
      }),
    );
    expect(await screen.findByText("Nothing is allowed beyond the Open hosts.")).toBeVisible();
  });

  it("says beside a general-purpose tool's Allow that it can show its values (T-4)", async () => {
    const SAID =
      "curl can show the values it is handed in the chat's output; allow it only for a persona you trust with them.";
    const row = (persona: string, target: string, cautions: string[] | null): SandboxGrant => ({
      id: `you\u001fpersona-hosts\u001f${persona}`,
      what: "persona-hosts",
      target,
      persona,
      level: "you",
      by: null,
      at: null,
      chat: null,
      locked: null,
      waiting: null,
      for_no_persona: false,
      expected: false,
      cautions,
    });
    mockIPC((cmd) =>
      cmd === "sandbox_grants"
        ? [
            row("logs", "curl with api-token from vault api as API_TOKEN env", [SAID]),
            row("cloud", "aws with aws-token from vault cloud as AWS_TOKEN env", null),
          ]
        : null,
    );
    render(<Granted />);

    const list = await screen.findByRole("list", { name: "Allowed" });
    const [logs, cloud] = within(list).getAllByRole("listitem");
    expect(logs).toHaveTextContent(SAID);
    expect(cloud).not.toHaveTextContent("can show the values");
  });

  it("lists a persona's hosts you allowed, credited to the persona, and removes them (#1362)", async () => {
    const DEVOPS: SandboxGrant = {
      id: "you\u001fpersona-hosts\u001fdevops",
      what: "persona-hosts",
      target: "10.100.39.145:6443, *.internal.example",
      persona: "devops",
      level: "you",
      by: null,
      at: null,
      chat: null,
      locked: null,
      waiting: null,
      for_no_persona: false,
      expected: false,
    };
    expect(grantSaid({ ...DEVOPS, for_no_persona: true })).toBe(
      "Chats as devops and every chat that names no persona reach 10.100.39.145:6443, *.internal.example · This project on this machine · allowed by you",
    );
    expect(grantSaid(DEVOPS)).toBe(
      "Chats as devops reach 10.100.39.145:6443, *.internal.example · This project on this machine · allowed by you",
    );
    const asked: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "sandbox_grants") return [DEVOPS];
      if (cmd === "revoke_sandbox_grant") return [];
      return null;
    });
    render(<Granted />);
    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: "Remove devops's hosts" }));
    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "revoke_sandbox_grant",
        args: { plane: PLANE, id: DEVOPS.id },
      }),
    );
  });

  it("says a browser site is the browsers' alone, apart from a host the chats' commands reach (#2018)", () => {
    const site: SandboxGrant = {
      id: "you\u001fbrowser-site\u001fnews.example.org:443",
      what: "browser-site",
      target: "news.example.org:443",
      persona: null,
      level: "you",
      by: null,
      at: null,
      chat: "qa 8",
      locked: null,
      waiting: null,
      for_no_persona: false,
      expected: false,
    };
    expect(grantSaid(site)).toBe(
      "Browsers open, send and type on news.example.org:443 (browsers only; chats' commands still ask) · This project on this machine · allowed by you, from qa 8",
    );
    expect(grantSaid({ ...site, what: "host", id: "you\u001fhost\u001fnews.example.org:443" })).toBe(
      "Reach news.example.org:443 · This project on this machine · allowed by you, from qa 8",
    );
  });

  it("lists Always for a persona's browser, credited to the persona, held by policy, and removes it (#2241)", async () => {
    const QA: SandboxGrant = {
      id: "you\u001fbrowser\u001fqa",
      what: "browser",
      target: "this project's sites, its dev servers and the open web",
      persona: "qa",
      level: "you",
      by: null,
      at: null,
      chat: "qa 8",
      locked: null,
      waiting: null,
      for_no_persona: false,
      expected: false,
    };
    expect(grantSaid(QA)).toBe(
      "Chats as qa open a browser without asking, reaching this project's sites, its dev servers and the open web · This project on this machine · allowed by you, from qa 8",
    );
    const asked: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "sandbox_grants")
        return [
          QA,
          {
            ...QA,
            id: "you\u001fbrowser\u001fops",
            persona: "ops",
            locked: "Policy removed Allow for you.",
          },
        ];
      if (cmd === "revoke_sandbox_grant") return [];
      return null;
    });
    render(<Granted />);
    expect(await screen.findByText(/Not in force\. Policy removed Allow for you\./)).toBeVisible();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Remove Always for qa's browser" }));
    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "revoke_sandbox_grant",
        args: { plane: PLANE, id: QA.id },
      }),
    );
  });

  it("lists an Allow whose list changed as not in force, with Remove (#1362)", async () => {
    const WAITING: SandboxGrant = {
      id: "you\u001fpersona-hosts\u001fdevops",
      what: "persona-hosts",
      target: "10.100.39.145:6443",
      persona: "devops",
      level: "you",
      by: null,
      at: null,
      chat: null,
      locked: null,
      waiting:
        "waiting: the list changed since you allowed it, so it reaches nothing until you allow it again",
      for_no_persona: false,
      expected: false,
    };
    mockIPC((cmd) => (cmd === "sandbox_grants" ? [WAITING] : null));
    render(<Granted />);
    expect(
      await screen.findByText(/Not in force: waiting: the list changed since you allowed it/),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Remove devops's hosts" })).toBeVisible();
  });

  it("names an Allow with no time as it is", () => {
    expect(grantSaid(MINE)).toBe(
      "Write /opt/cache and everything in it · This project on this machine · allowed by you",
    );
  });
});

describe("the Allowed list's answers (#1543)", () => {
  it("draws the newest answer, though an older one lands after it", async () => {
    // Both commands answer off the window's thread, so a first revoke's answer can land after
    // a second's: the grant the second took away must not come back.
    let landFirst: (list: SandboxGrant[]) => void = () => {};
    mockIPC((cmd, args) => {
      if (cmd === "sandbox_grants") return [CHAT, MINE];
      if (cmd === "revoke_sandbox_grant") {
        if ((args as { id: string }).id === MINE.id)
          return new Promise<SandboxGrant[]>((land) => (landFirst = land));
        return [];
      }
      return null;
    });
    render(<Granted />);
    const list = await screen.findByRole("list", { name: "Allowed" });
    await userEvent.click(within(list).getByRole("button", { name: "Remove writing /opt/cache" }));
    await userEvent.click(
      within(list).getByRole("button", { name: "Remove reaching api.example.com" }),
    );
    expect(
      await screen.findByText("Nothing is allowed beyond the Open hosts."),
    ).toBeInTheDocument();
    landFirst([CHAT]);
    await new Promise((settle) => setTimeout(settle, 20));
    expect(screen.getByText("Nothing is allowed beyond the Open hosts.")).toBeInTheDocument();
  });
});

describe("a service on this Mac in the Granted list (qw384)", () => {
  it("says the port it lets every chat of this project connect to", () => {
    const PORT: SandboxGrant = {
      id: "you\u001fport\u001f50795",
      what: "port",
      target: "50795",
      persona: null,
      level: "you",
      by: null,
      at: null,
      chat: null,
      locked: null,
      waiting: null,
      for_no_persona: false,
      expected: false,
    };
    expect(grantSaid(PORT)).toBe(
      "Connect to the service on this Mac at port 50795 (Codex and opencode chats; Claude Code chats not yet) · This project on this machine · allowed by you",
    );
  });
});
