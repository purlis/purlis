// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { OutsideFileTab } from "./PieceFiles";

/** What the core was asked, by command. */
let asked: { cmd: string; args: Record<string, unknown> }[] = [];
/** What the core answers `pane_link_runner_file`. */
let answer: unknown;

beforeEach(() => {
  asked = [];
  answer = { read: "file", file: { kind: "text", text: "fn main() {}\n" } };
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "pane_link_runner_file") {
      if (typeof answer === "string") throw answer;
      return answer;
    }
    if (cmd === "pane_link_file") throw "never asked for a runner's file";
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a file on a runner chat's runner, in its tab (#6387)", () => {
  it("is read on its runner for its chat, and says it is there, read-only", async () => {
    render(
      <OutsideFileTab plane="/planes/one" session={7} runner="docker/runner-1" path="/w/main.rs" />,
    );

    await screen.findByText("fn main() {}");
    expect(screen.getByText("In runner docker/runner-1, read-only")).toBeInTheDocument();
    expect(asked.filter((one) => one.cmd === "pane_link_runner_file")[0]?.args).toEqual({
      from: { plane: "/planes/one", session: 7 },
      runner: "docker/runner-1",
      path: "/w/main.rs",
    });
    expect(asked.some((one) => one.cmd === "pane_link_file")).toBe(false);
  });

  it("says what the runner said where it could not be read", async () => {
    answer =
      "This file is now outside the chat's workspace on runner docker/runner-1. Open it from its link in the chat again.";
    render(
      <OutsideFileTab plane="/planes/one" session={7} runner="docker/runner-1" path="/w/x.rs" />,
    );

    await waitFor(() =>
      expect(
        screen.getByText(/now outside the chat's workspace on runner docker\/runner-1/u),
      ).toBeInTheDocument(),
    );
  });
});
