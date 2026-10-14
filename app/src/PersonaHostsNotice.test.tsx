// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { PersonaHostsNotice } from "./PersonaHostsNotice";
import { ChatsHere } from "./chatState";
import { startingChats } from "./chatStarts.testkit";
import { inTheWindow } from "./personaHostsAllow";
import type { PersonaHosts, SandboxState } from "./bindings";

/**
 * A persona's committed hosts wait for the person on each machine (#1362, D-1362-7): the
 * Notice shows the list, and Allow sends back the digest of exactly what it showed.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const DEVOPS: PersonaHosts = {
  persona: "devops",
  hosts: ["10.100.39.145:6443", "*.internal.example"],
  reached: ["10.100.39.145:6443", "*.internal.example"],
  tools: [],
  digest: "d1",
  allowed: false,
  default: false,
  waiting: false,
};

const QUIET: SandboxState = {
  on: true,
  offer: false,
  said: null,
  never: [],
  hosts_changed: null,
  presets_changed: null,
  presets: [],
  persona_hosts: [],
  besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
  policy: null,
};

const WAITING: SandboxState = { ...QUIET, persona_hosts: [DEVOPS] };
const ALLOWED: SandboxState = { ...QUIET, persona_hosts: [{ ...DEVOPS, allowed: true }] };

const NAME = "devops's hosts wait for you";

function core(state: SandboxState, allowed: SandboxState | string = ALLOWED) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "sandbox_state") return state;
    if (cmd === "allow_persona_hosts") {
      if (typeof allowed === "string") throw allowed;
      return allowed;
    }
    return null;
  });
  return asked;
}

describe("a persona's hosts Notice", () => {
  it("shows the hosts a chat as the persona would reach, and that none reach it until allowed", async () => {
    core(WAITING);
    render(<PersonaHostsNotice plane={PLANE} />);

    const notice = await screen.findByRole("status", { name: NAME });
    expect(notice).toHaveTextContent(
      "The project's settings let chats as devops reach 10.100.39.145:6443 and *.internal.example.",
    );
    expect(notice).toHaveTextContent("no chat here reaches these hosts until you allow them");
    expect(notice).toHaveTextContent(
      "Chats started after you allow them reach them; a chat already running takes them when it restarts.",
    );
  });

  it("says the default persona's hosts reach every chat that names no persona, and allows that", async () => {
    const asked = core({ ...QUIET, persona_hosts: [{ ...DEVOPS, default: true }] });
    render(<PersonaHostsNotice plane={PLANE} />);

    const notice = await screen.findByRole("status", { name: NAME });
    expect(notice).toHaveTextContent(
      "let chats as devops, and every chat that names no persona (devops is this project's default persona), reach",
    );
    await userEvent.setup().click(
      await screen.findByRole("button", {
        name: "Allow for devops chats and chats that name no persona",
      }),
    );
    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "allow_persona_hosts",
        args: { plane: PLANE, persona: "devops", digest: "d1" },
      }),
    );
  });

  it("says an Allow whose list changed since reaches nothing until it is allowed again", async () => {
    core({ ...QUIET, persona_hosts: [{ ...DEVOPS, waiting: true }] });
    render(<PersonaHostsNotice plane={PLANE} />);

    const notice = await screen.findByRole("status", { name: NAME });
    expect(notice).toHaveTextContent(
      "You allowed devops's hosts on this machine, and they have changed since, so no chat reaches any of them until you allow them again.",
    );
  });

  it("offers no Allow on a link, whose client has no window-only call", async () => {
    core(WAITING);
    render(<PersonaHostsNotice plane={PLANE} canAllow={false} />);

    const notice = await screen.findByRole("status", { name: NAME });
    expect(notice).toHaveTextContent("Allow them from purlis's own window on this machine.");
    expect(screen.queryByRole("button", { name: /Allow/ })).toBeNull();
    expect(inTheWindow({ allowPersonaHosts: () => null }, "allowPersonaHosts")).toBe(true);
    expect(inTheWindow({}, "allowPersonaHosts")).toBe(false);
  });

  it("sends back the digest of the list it showed, and is gone once allowed", async () => {
    const asked = core(WAITING);
    render(<PersonaHostsNotice plane={PLANE} />);

    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: "Allow for devops chats" }));

    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "allow_persona_hosts",
        args: { plane: PLANE, persona: "devops", digest: "d1" },
      }),
    );
    await waitFor(() => expect(screen.queryByRole("status", { name: NAME })).toBeNull());
  });

  it("says why when the list changed since it was shown, and stays", async () => {
    core(WAITING, "purlis allowed nothing: devops's hosts changed after they were shown.");
    render(<PersonaHostsNotice plane={PLANE} />);

    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: "Allow for devops chats" }));

    const notice = await screen.findByRole("status", { name: NAME });
    await waitFor(() => expect(notice).toHaveTextContent("changed after they were shown"));
  });

  it("asks nothing for hosts already allowed, none a chat would reach, or under a policy lock", async () => {
    const asked = core({
      ...QUIET,
      persona_hosts: [
        { ...DEVOPS, allowed: true },
        { ...DEVOPS, persona: "qa", reached: [] },
      ],
    });
    render(<PersonaHostsNotice plane={PLANE} />);
    await waitFor(() => expect(asked.map((one) => one.cmd)).toContain("sandbox_state"));
    expect(screen.queryByRole("status")).toBeNull();

    cleanup();
    clearMocks();
    const locked = core({
      ...WAITING,
      policy: {
        presets: null,
        hosts: null,
        persona_hosts: true,
        personal_hosts: false,
        write_grants: false,
        required: false,
        locked_by: "Locked by policy.",
      } as unknown as SandboxState["policy"],
    });
    render(<PersonaHostsNotice plane={PLANE} />);
    await waitFor(() => expect(locked.map((one) => one.cmd)).toContain("sandbox_state"));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("asks for a persona's tools with its hosts, and for its tools alone, under one Allow", async () => {
    const KUBECTL = "kubectl with kubeconfig from vault devops as KUBECONFIG file";
    const asked = core({ ...QUIET, persona_hosts: [{ ...DEVOPS, tools: [KUBECTL] }] });
    render(<PersonaHostsNotice plane={PLANE} />);

    const notice = await screen.findByRole("status", {
      name: "devops's hosts and tools wait for you",
    });
    expect(notice).toHaveTextContent(
      `let chats as devops reach 10.100.39.145:6443 and *.internal.example, and run ${KUBECTL}, through purlis with a credential from the vault.`,
    );
    expect(notice).toHaveTextContent(
      "no chat here gets these hosts and tools until you allow them",
    );
    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: "Allow for devops chats" }));
    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "allow_persona_hosts",
        args: { plane: PLANE, persona: "devops", digest: "d1" },
      }),
    );

    cleanup();
    clearMocks();
    core({
      ...QUIET,
      persona_hosts: [{ ...DEVOPS, hosts: [], reached: [], tools: [KUBECTL] }],
      // A policy that forbids persona hosts leaves its tools to ask about.
      policy: {
        presets: null,
        hosts: null,
        persona_hosts: true,
        personal_hosts: false,
        write_grants: false,
        required: false,
        locked_by: "Locked by policy.",
      } as unknown as SandboxState["policy"],
    });
    render(<PersonaHostsNotice plane={PLANE} />);
    const alone = await screen.findByRole("status", { name: "devops's tools wait for you" });
    expect(alone).toHaveTextContent(`let chats as devops run ${KUBECTL}, through purlis`);
  });

  it("shows every value a tool is handed, and keeps two tools apart", async () => {
    // T-1: a tool handed several values names each on its line, which an Allow is bound to.
    const KUBECTL = "kubectl with kubeconfig from vault devops as KUBECONFIG file";
    const CURL =
      'curl with "kibana-password" as KIBANA_PASS file and "kibana-username" as KIBANA_USER env from vault elastic-logs-master';
    core({
      ...QUIET,
      persona_hosts: [{ ...DEVOPS, hosts: [], reached: [], tools: [KUBECTL, CURL] }],
    });
    render(<PersonaHostsNotice plane={PLANE} />);

    const notice = await screen.findByRole("status", { name: "devops's tools wait for you" });
    expect(notice).toHaveTextContent(
      `let chats as devops run ${KUBECTL}; and ${CURL}, through purlis`,
    );
    expect(notice).toHaveTextContent('"kibana-password" as KIBANA_PASS file');
    expect(notice).toHaveTextContent('"kibana-username" as KIBANA_USER env');
  });

  it("says a general-purpose tool can show its values in the chat's output, and an unlisted one says nothing", async () => {
    // T-4: one sentence beside the Allow, from the core; never part of the line allowed.
    const CURL = "curl with api-token from vault api as API_TOKEN env";
    const AWS = "aws with aws-token from vault cloud as AWS_TOKEN env";
    const SAID =
      "curl can show the values it is handed in the chat's output; allow it only for a persona you trust with them.";
    core({
      ...QUIET,
      persona_hosts: [
        { ...DEVOPS, persona: "logs", hosts: [], reached: [], tools: [CURL], cautions: [SAID] },
        { ...DEVOPS, persona: "cloud", hosts: [], reached: [], tools: [AWS], cautions: null },
      ],
    });
    render(<PersonaHostsNotice plane={PLANE} />);

    const logs = await screen.findByRole("status", { name: "logs's tools wait for you" });
    expect(logs).toHaveTextContent(`let chats as logs run ${CURL}, through purlis`);
    expect(logs.textContent).toContain(`from the vault. ${SAID} Anyone who can change`);
    const cloud = await screen.findByRole("status", { name: "cloud's tools wait for you" });
    expect(cloud).toHaveTextContent(`let chats as cloud run ${AWS}, through purlis`);
    expect(cloud).not.toHaveTextContent("can show the values");
    expect(cloud.textContent).toContain("from the vault. Anyone who can change");
  });

  it("hides on Not now and keeps nothing", async () => {
    const asked = core(WAITING);
    render(<PersonaHostsNotice plane={PLANE} />);

    await userEvent.setup().click(await screen.findByRole("button", { name: "Not now" }));

    await waitFor(() => expect(screen.queryByRole("status", { name: NAME })).toBeNull());
    expect(asked.map((one) => one.cmd)).not.toContain("allow_persona_hosts");
  });
});

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

/**
 * A core whose state is `states[0]` until it is told the project moved, then the next, and a
 * way to say so: `plane-changed` for the project, as the plane watcher sends it.
 */
function changing(...states: SandboxState[]) {
  const listeners = new Map<string, number[]>();
  let at = 0;
  mockIPC((cmd, args) => {
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    if (cmd === "sandbox_state") return states[Math.min(at, states.length - 1)];
    return null;
  });
  return {
    pulled: async (answers: { answer: string }[]) => {
      at += 1;
      await waitFor(() => expect(listeners.get("plane-changed") ?? []).not.toHaveLength(0));
      await act(async () => {
        for (const handler of listeners.get("plane-changed") ?? [])
          window.__TAURI_INTERNALS__.runCallback(handler, {
            event: "plane-changed",
            id: 1,
            payload: { plane: PLANE, changes: null, answers },
          });
        await Promise.resolve();
      });
    },
    focused: () => {
      at += 1;
      act(() => {
        window.dispatchEvent(new Event("focus"));
      });
    },
  };
}

/** The Notice follows the project's state while it is open, not only when it mounts (#1407). */
describe("a persona's hosts Notice, live", () => {
  const quiet = async () => {
    render(<PersonaHostsNotice plane={PLANE} />);
    await waitFor(() => expect(screen.queryByRole("status", { name: NAME })).toBeNull());
  };

  it("asks when a pull changes the project's settings while it is open", async () => {
    const { pulled } = changing(QUIET, WAITING);
    await quiet();

    await pulled([{ answer: "settings" }]);

    expect(await screen.findByRole("status", { name: NAME })).toBeInTheDocument();
  });

  it("is gone once the list is allowed elsewhere and the settings say so", async () => {
    const { pulled } = changing(WAITING, ALLOWED);
    render(<PersonaHostsNotice plane={PLANE} />);
    await screen.findByRole("status", { name: NAME });

    await pulled([{ answer: "settings" }]);

    await waitFor(() => expect(screen.queryByRole("status", { name: NAME })).toBeNull());
  });

  it("is not read again for a change that is not the settings'", async () => {
    const { pulled } = changing(QUIET, WAITING);
    await quiet();

    await pulled([{ answer: "sidebar" }]);

    expect(screen.queryByRole("status", { name: NAME })).not.toBeInTheDocument();
  });

  it("is read again when the window comes back into focus", async () => {
    const { focused } = changing(QUIET, WAITING);
    await quiet();

    focused();

    expect(await screen.findByRole("status", { name: NAME })).toBeInTheDocument();
  });
});

/**
 * The Notice reads again when a chat starts (#1407): a list allowed in another window, which no
 * watcher of this one reports, is not still asked about as the chat it would reach starts.
 */
describe("a persona's hosts Notice, as chats start", () => {
  /** A core answering `states` in turn, one per `sandbox_state`, and how often it was asked. */
  function inTurn(...states: SandboxState[]) {
    let asked = 0;
    mockIPC((cmd) => {
      if (cmd !== "sandbox_state") return null;
      asked += 1;
      return states[Math.min(asked - 1, states.length - 1)];
    });
    return () => asked;
  }

  it("is gone when a chat starts after the list was allowed elsewhere", async () => {
    const asked = inTurn(WAITING, ALLOWED);
    const { chats, start } = startingChats(PLANE);
    render(
      <ChatsHere.Provider value={chats}>
        <PersonaHostsNotice plane={PLANE} />
      </ChatsHere.Provider>,
    );
    await screen.findByRole("status", { name: NAME });
    expect(asked()).toBe(1);

    start(1);

    await waitFor(() => expect(screen.queryByRole("status", { name: NAME })).toBeNull());
    expect(asked()).toBe(2);
  });

  it("asks once per start, and not again when a chat closes or only moves", async () => {
    const asked = inTurn(QUIET);
    const { chats, start, close } = startingChats(PLANE);
    render(
      <ChatsHere.Provider value={chats}>
        <PersonaHostsNotice plane={PLANE} />
      </ChatsHere.Provider>,
    );
    await waitFor(() => expect(asked()).toBe(1));

    start(1);
    await waitFor(() => expect(asked()).toBe(2));
    start(1);
    close(1);
    start(2);
    await waitFor(() => expect(asked()).toBe(3));
    close(2);
    await act(async () => {
      await Promise.resolve();
    });
    expect(asked()).toBe(3);
  });
});
