// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { McpFailedNotice } from "./McpFailedNotice";
import type { McpFailed } from "./bindings";

/**
 * A persona's MCP server that could not start after purlis's second try says so on its chat's
 * tab (qw381, ruling T-3): the server, the persona, and the server's own error as data, with
 * Retry (the chat's restart on its conversation) and Dismiss.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const ELASTIC: McpFailed = {
  plane: PLANE,
  session: 7,
  server: "elasticsearch",
  persona: "elastic-logs-master",
  error: "Error: connect ECONNREFUSED 127.0.0.1:9200 | login as *** failed",
};

/** A core holding `held` for chat 7. */
function core(held: McpFailed[]) {
  const asked: { cmd: string; args: unknown }[] = [];
  let now = held;
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "mcp_failures") return now;
      if (cmd === "put_away_mcp_failure") {
        const server = (args as { server: string }).server;
        now = now.filter((one) => one.server !== server);
        return null;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    pressed: () => asked.filter((one) => one.cmd !== "mcp_failures"),
    fail: (one: McpFailed) => {
      now = [...now, one];
    },
  };
}

const notice = () => screen.findByRole("status", { name: "MCP server" });

describe("the failed MCP server Notice", () => {
  it("names the server and gives its own error as data, with Retry and Dismiss", async () => {
    const { pressed } = core([ELASTIC]);
    render(<McpFailedNotice plane={PLANE} session={7} onRestart={() => {}} />);
    expect(await notice()).toHaveTextContent(
      "MCP server elasticsearch could not start, twice, so its tools are not in this chat. Retry restarts this chat on its conversation once its turn has ended.",
    );
    expect(screen.getByText(/Its error/)).toHaveTextContent(
      "Its error: Error: connect ECONNREFUSED 127.0.0.1:9200 | login as *** failed",
    );
    expect(screen.getByText("elasticsearch").tagName).toBe("CODE");
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual(
      expect.arrayContaining(["Retry"]),
    );
    expect(pressed()).toEqual([]);
  });

  it("restarts the chat on Retry and puts the Notice away", async () => {
    const { pressed } = core([ELASTIC]);
    const restart = vi.fn();
    render(<McpFailedNotice plane={PLANE} session={7} onRestart={restart} />);
    await notice();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(restart).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(pressed()).toEqual([
        {
          cmd: "put_away_mcp_failure",
          args: { plane: PLANE, session: 7, server: "elasticsearch" },
        },
      ]),
    );
    await waitFor(() =>
      expect(screen.queryByRole("status", { name: "MCP server" })).not.toBeInTheDocument(),
    );
  });

  it("draws nothing until a server fails, and the failure when the core says so", async () => {
    const { fail } = core([]);
    render(<McpFailedNotice plane={PLANE} session={7} onRestart={() => {}} />);
    await act(async () => {});
    expect(screen.queryByRole("status", { name: "MCP server" })).not.toBeInTheDocument();
    fail(ELASTIC);
    await act(async () => {
      await emit("chat-mcp-failed", ELASTIC);
    });
    expect(await notice()).toHaveTextContent("MCP server elasticsearch could not start");
  });
});
