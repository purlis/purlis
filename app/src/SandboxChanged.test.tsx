// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { OlderSandbox, PlaneId } from "./bindings";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import {
  CLUSTER_POOL_ADDED,
  TLS_POOL_ADDED,
  POLICY_CHANGED,
  PolicyRequiresNotice,
  SandboxChangedNotice,
  useOlderSandbox,
} from "./SandboxChanged";

/**
 * **Where this machine's policy is the cause** (#1431): the Notice after a sandbox change says
 * so, and who set the policy, and a chat running without the sandbox that policy now requires
 * says so on its own tab, with Restart. A policy that changes is asked about again with nothing
 * else moving. The window's whole Restart flow is in `RestartChat.window.test.tsx`.
 */

const PLANE = "/projects/acme" as PlaneId;
const POLICY =
  "The sandbox is now required by this machine's policy, set by IT in /etc/purlis/policy.json.";

afterEach(() => {
  cleanup();
  clearMocks();
});

function theNotice(older: OlderSandbox) {
  render(
    <ChatsHere.Provider value={fixedChats({ ...nothingKnown, bySession: { 4: "waiting" } })}>
      <SandboxChangedNotice
        older={older}
        dismissed={new Set()}
        dismiss={vi.fn()}
        settle={vi.fn()}
        onRestart={vi.fn()}
      />
    </ChatsHere.Provider>,
  );
  return screen.getByRole("status", { name: "Sandbox changed" });
}

describe("the Notice after a sandbox change, where policy is the cause", () => {
  it("leads with the policy and who set it, and says the chat runs without the sandbox", () => {
    const notice = theNotice({ chats: [{ session: 4, change: "c1", policy: POLICY }] });
    expect(notice).toHaveTextContent(
      `${POLICY} 1 chat runs without it until it restarts. A restart keeps the conversation`,
    );
    expect(notice).not.toHaveTextContent("This project's sandbox changed.");
  });

  it("says the project's change and the policy both, where only some chats are policy's", () => {
    const notice = theNotice({
      chats: [
        { session: 4, change: "c1", policy: POLICY },
        { session: 5, change: "c1" },
      ],
    });
    expect(notice).toHaveTextContent(
      `This project's sandbox changed. ${POLICY} 2 chats keep the old sandbox until they restart.`,
    );
  });

  it("names no policy where none is the cause", () => {
    const notice = theNotice({ chats: [{ session: 4, change: "c1", policy: null }] });
    expect(notice).toHaveTextContent(
      "This project's sandbox changed. 1 chat keeps the old sandbox until it restarts.",
    );
    expect(notice).not.toHaveTextContent("policy");
  });
});

/** The project's hook, as `PlaneView` holds it, and chat 4's tab beside it. */
function Window({ onRestart }: { onRestart: () => void }) {
  useOlderSandbox(PLANE, "asked");
  return <PolicyRequiresNotice plane={PLANE} session={4} onRestart={onRestart} />;
}

function answering(answers: (OlderSandbox | null)[]) {
  const asked: number[] = [];
  mockIPC(
    (cmd) => {
      if (cmd !== "chats_on_older_sandbox") return null;
      asked.push(asked.length);
      return answers[Math.min(asked.length - 1, answers.length - 1)] ?? null;
    },
    { shouldMockEvents: true },
  );
  return asked;
}

describe("a chat's tab, where policy now requires the sandbox it runs without", () => {
  it("says so with who set it, and Restart restarts this chat", async () => {
    answering([{ chats: [{ session: 4, change: "c1", policy: POLICY }] }]);
    const onRestart = vi.fn();
    render(<Window onRestart={onRestart} />);

    const notice = await screen.findByRole("status", { name: "Sandbox required" });
    expect(notice).toHaveTextContent(
      `${POLICY} This chat runs without it until it restarts. A restart keeps the conversation.`,
    );
    await userEvent.click(within(notice).getByRole("button", { name: "Restart this chat" }));
    expect(onRestart).toHaveBeenCalledOnce();
  });

  it("says nothing for a chat behind for the project's own settings", async () => {
    const asked = answering([{ chats: [{ session: 4, change: "c1" }] }]);
    render(<Window onRestart={vi.fn()} />);
    await waitFor(() => expect(asked.length).toBeGreaterThan(0));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(screen.queryByRole("status", { name: "Sandbox required" })).toBeNull();
  });

  it("is asked again when this machine's policy changes, and comes up with nothing else moving", async () => {
    const asked = answering([null, { chats: [{ session: 4, change: "c1", policy: POLICY }] }]);
    render(<Window onRestart={vi.fn()} />);
    await waitFor(() => expect(asked.length).toBe(1));
    expect(screen.queryByRole("status", { name: "Sandbox required" })).toBeNull();

    await act(() => emit(POLICY_CHANGED, null));

    expect(await screen.findByRole("status", { name: "Sandbox required" })).toBeInTheDocument();
    expect(asked.length).toBe(2);
  });
});

/**
 * **A cluster pool added while chats ran** (#1750): a chat started before it holds none of the
 * programs the pool runs until it restarts. The Notice says that is why, in the person's terms,
 * and the window asks again as the pool is added, with nothing else moving.
 */
describe("the Notice after a cluster pool was added", () => {
  const POOL =
    "A cluster pool was added, so purlis now keeps chats from changing the programs it runs.";

  it("leads with the pool, and says the chats keep the old sandbox until they restart", () => {
    const notice = theNotice({
      chats: [
        { session: 4, change: "c1", pool: POOL },
        { session: 5, change: "c1", pool: POOL },
      ],
    });
    expect(notice).toHaveTextContent(
      `${POOL} 2 chats keep the old sandbox until they restart. A restart keeps the conversation`,
    );
    expect(notice).not.toHaveTextContent("This project's sandbox changed.");
    expect(within(notice).getByRole("button", { name: "Restart them" })).toBeInTheDocument();
  });

  it("says the project's change where the pool is not the only cause for every chat", () => {
    const notice = theNotice({
      chats: [
        { session: 4, change: "c1", pool: POOL },
        { session: 5, change: "c2" },
      ],
    });
    expect(notice).toHaveTextContent("This project's sandbox changed.");
    expect(notice).not.toHaveTextContent(POOL);
  });

  it("is asked again when a cluster pool is added", async () => {
    const asked = answering([null]);
    render(<Window onRestart={vi.fn()} />);
    await waitFor(() => expect(asked.length).toBe(1));

    await act(() => emit(CLUSTER_POOL_ADDED, null));

    await waitFor(() => expect(asked.length).toBe(2));
  });
});

/**
 * **A pool over Docker's TLS API added while chats ran** (#6472): a chat started before it reads
 * the pool's certificate and key until it restarts. The Notice says so in the person's terms, and
 * the window asks again as the pool is added.
 */
describe("the Notice after a pool over Docker's TLS API was added", () => {
  const TLS =
    "A pool over Docker's TLS API was added, so purlis now keeps chats from reading its certificate and key.";
  const CLUSTER =
    "A cluster pool was added, so purlis now keeps chats from changing the programs it runs.";

  it("leads with the TLS pool, and says the chat keeps the old sandbox until it restarts", () => {
    const notice = theNotice({ chats: [{ session: 4, change: "c1", pool: TLS }] });
    expect(notice).toHaveTextContent(
      `${TLS} 1 chat keeps the old sandbox until it restarts. A restart keeps the conversation`,
    );
    expect(within(notice).getByRole("button", { name: "Restart it" })).toBeInTheDocument();
  });

  it("says the project's change where the chats are behind different pools", () => {
    const notice = theNotice({
      chats: [
        { session: 4, change: "c1", pool: CLUSTER },
        { session: 5, change: "c2", pool: TLS },
      ],
    });
    expect(notice).toHaveTextContent("This project's sandbox changed.");
    expect(notice).not.toHaveTextContent(TLS);
    expect(notice).not.toHaveTextContent(CLUSTER);
  });

  it("is asked again when a pool over Docker's TLS API is added", async () => {
    const asked = answering([null]);
    render(<Window onRestart={vi.fn()} />);
    await waitFor(() => expect(asked.length).toBe(1));

    await act(() => emit(TLS_POOL_ADDED, null));

    await waitFor(() => expect(asked.length).toBe(2));
  });
});
