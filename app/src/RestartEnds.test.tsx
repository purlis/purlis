// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { RestartEndsSaid } from "./RestartEnds";

afterEach(cleanup);

const said = (ends: Parameters<typeof RestartEndsSaid>[0]["ends"]) =>
  render(<RestartEndsSaid ends={ends} />).container.textContent;

describe("what an Allow that restarts the chat says beforehand (#1986)", () => {
  it("names the program listening on a port that will stop", () => {
    expect(said({ listening: [{ program: "node", port: 5173 }], others: 0 })).toBe(
      "Allowing restarts this chat on its conversation at its next pause between turns; its node on port 5173 will stop.",
    );
  });

  it("names each listening program and counts the other programs its commands started", () => {
    expect(
      said({
        listening: [
          { program: "node", port: 5173 },
          { program: "python3", port: 8000 },
        ],
        others: 2,
      }),
    ).toBe(
      "Allowing restarts this chat on its conversation at its next pause between turns; its node on port 5173, python3 on port 8000 and 2 other programs it started will stop.",
    );
  });

  it("counts the programs its commands started where none listens", () => {
    expect(said({ listening: [], others: 1 })).toBe(
      "Allowing restarts this chat on its conversation at its next pause between turns; 1 program it started will stop.",
    );
  });

  it("says that nothing it started is running", () => {
    expect(said({ listening: [], others: 0 })).toBe(
      "Allowing restarts this chat on its conversation at its next pause between turns; nothing it started is running now.",
    );
  });

  it("says plainly when purlis cannot tell what will end", () => {
    expect(said(null)).toBe(
      "Allowing restarts this chat on its conversation at its next pause between turns. purlis cannot tell what it has running, so anything it started may stop.",
    );
  });

  it("says only when it restarts while what will end is still being read", () => {
    expect(said(undefined)).toBe(
      "Allowing restarts this chat on its conversation at its next pause between turns.",
    );
  });

  it("draws a program's name inertly, apart from purlis's words", () => {
    const { container } = render(
      <RestartEndsSaid
        ends={{ listening: [{ program: "dev\u202eserver", port: 3000 }], others: 0 }}
      />,
    );
    const name = container.querySelector("code");
    expect(name?.textContent).toBe("dev⟦U+202E⟧server");
  });

  it("names the chat where it is read away from the chat's own pane", () => {
    expect(
      render(
        <RestartEndsSaid
          ends={{ listening: [{ program: "node", port: 5173 }], others: 0 }}
          chat="steward 3"
        />,
      ).container.textContent,
    ).toBe(
      "Allowing restarts steward 3 on its conversation at its next pause between turns; its node on port 5173 will stop.",
    );
  });

  it("says it restarts now where the chat is between turns already", () => {
    expect(
      render(<RestartEndsSaid ends={{ listening: [], others: 1 }} now />).container.textContent,
    ).toBe(
      "Allowing restarts this chat on its conversation now, as it is between turns; 1 program it started will stop.",
    );
  });
});
