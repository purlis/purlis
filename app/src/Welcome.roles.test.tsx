// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { declarations } from "./guards.testkit";
import { wears } from "./textRoles.testkit";
import { Welcome } from "./Welcome";
import { Launcher } from "./DoorPage";

/**
 * **Every word of the Welcome wears a text role** (#1920, the seventh surface to move;
 * `docs/design-system.md` "Text roles"). The Welcome drew its heading and the wordmark at
 * `--font-size-xl`, above the title, and its lede, its steps and its section headings in rules of
 * its own; each is now one of the roles, held here through the window's own cascade.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

function core() {
  mockIPC(
    (cmd) => {
      if (cmd === "recent_planes")
        return {
          planes: [{ path: "/home/dev/elsewhere", name: "elsewhere", approved: false }],
          gone: [],
          dropped: [],
          forgetful: null,
        };
      return null;
    },
    { shouldMockEvents: true },
  );
}

function draw() {
  core();
  return render(
    <Launcher>
      <Welcome
        here={false}
        reason=""
        onOpen={() => {}}
        onOpenRepo={() => {}}
        onMakeProject={() => {}}
        onSignInToForge={() => {}}
      />
    </Launcher>,
  ).container;
}

describe("the Welcome's words, each on its text role", () => {
  it("titles the page and draws the wordmark with the title, and says its lede as the body", async () => {
    const drawn = draw();
    wears(
      screen.getByRole("heading", { level: 1, name: "Welcome to purlis" }),
      "title",
      "the Welcome's heading",
    );
    wears(drawn.querySelector(".welcome-brand"), "title", "the wordmark");
    wears(drawn.querySelector(".welcome-lede"), "body", "the lede");
    await screen.findByText("elsewhere");
  });

  it("labels the recent projects with the section label, and says a path and a note as the meta", async () => {
    const drawn = draw();
    await screen.findByText("elsewhere");
    wears(
      screen.getByRole("heading", { level: 2, name: "Recent projects" }),
      "label",
      "the recents' heading",
    );
    const where = drawn.querySelector(".recents code");
    wears(where, "meta", "a project's path");
    expect(where).toHaveClass("ui-text-code");
    wears(screen.getByText("purlis will ask about this one"), "meta", "a project's note");
  });

  it("writes no size, weight or colour of words of its own for what now wears a role", () => {
    const worn =
      /^\.welcome-(?:brand|lede|next|elsewhere)$|^\.welcome-pane (?:h1|h2)$|^\.recents (?:\.where|\.value|code)$/;
    const typed = declarations(readFileSync(join(process.cwd(), "src", "App.css"), "utf8"))
      .filter(({ context }) => worn.test(context[context.length - 1] ?? ""))
      .filter(({ property }) => ["font-size", "font-weight", "color"].includes(property))
      .map(({ context, property, value }) => `${context.join(" ")} | ${property}: ${value}`);
    expect(typed).toEqual([]);
  });
});
