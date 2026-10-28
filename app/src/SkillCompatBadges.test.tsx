// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { SkillCompatBadges, fitSaid } from "./SkillCompatBadges";

afterEach(cleanup);

describe("a skill's compatibility badges (SK-8)", () => {
  it("marks a skill from outside the project that widens the chat, with its lint", () => {
    render(
      <SkillCompatBadges
        name="wide"
        compat={[
          { harness: "claude", title: "Claude Code", fit: "widens", says: ["Widens the chat: …"] },
        ]}
        lint={["it names skills:gone, and this project has no skill gone"]}
      />,
    );
    expect(screen.getByTestId("widens-wide").textContent).toBe("Widens the chat");
    expect(screen.getByText(/no skill gone/)).toBeInTheDocument();
  });

  it("draws one per harness with how well it fits, and the core's lines under it", () => {
    render(
      <SkillCompatBadges
        name="deploy"
        compat={[
          {
            harness: "claude",
            title: "Claude Code",
            fit: "partial",
            says: ["Left out, because a skill may only narrow a chat: the tools it pre-approves."],
          },
          {
            harness: "codex",
            title: "Codex",
            fit: "full",
            says: ["Carried over by purlis's equivalent: running only when asked for."],
          },
        ]}
      />,
    );
    const list = screen.getByRole("list", { name: "Where deploy works" });
    expect(within(list).getByText("Claude Code: partial")).toBeInTheDocument();
    expect(within(list).getByText("Codex: full")).toBeInTheDocument();
    expect(screen.getByTestId("compat-deploy-claude").textContent).toContain(
      "a skill may only narrow a chat",
    );
  });

  it("draws nothing for a skill no harness would load, and names a widening skill plainly", () => {
    const { container } = render(<SkillCompatBadges name="half" compat={[]} />);
    expect(container).toBeEmptyDOMElement();
    expect(fitSaid("widens")).toBe("widens the chat");
    expect(fitSaid("refused")).toBe("handed to no chat");
  });

  it("says skills are not supported on a declared harness that takes none (SK-30)", () => {
    const said =
      "Skills are not supported on Goose: it has no skill format purlis can hand skills by, and takes no MCP servers, so its chats get no skills.";
    render(
      <SkillCompatBadges
        name="deploy"
        compat={[]}
        notSupported={[{ name: "goose", title: "Goose", said }]}
      />,
    );
    const badge = screen.getByTestId("compat-deploy-goose");
    expect(within(badge).getByText("Goose: skills not supported")).toBeInTheDocument();
    expect(within(badge).getByText(said)).toBeInTheDocument();
  });
});
