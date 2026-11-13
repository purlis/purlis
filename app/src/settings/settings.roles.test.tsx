// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { declarations } from "../guards.testkit";
import { drawnWith } from "../cascade.testkit";
import { typeOf, wears } from "../textRoles.testkit";
import { SettingGroup, SettingRow, SettingsLayout } from "./components";

/**
 * **Every word of Settings wears a text role** (#1920, the fifth surface to move;
 * `docs/design-system.md` "Text roles"). Settings ranked its words in rules of its own: a page's
 * title at the largest size token, help lines at 11px in one place and 12px in another, in two
 * greys. Each is now one of the roles, held here through the window's own cascade.
 *
 * **A form's label is the one word that wears none**: it is the medium weight (#2317), which no
 * role reads.
 */

afterEach(cleanup);

function page() {
  return render(
    <SettingsLayout
      levels={[{ id: "you", label: "You" }]}
      level="you"
      onLevelChange={() => {}}
      about="Your settings, on every machine you sign in on."
      groups={[{ id: "look", label: "Look" }]}
      group="look"
      onGroupChange={() => {}}
      filter=""
      onFilterChange={() => {}}
    >
      <SettingGroup
        label="Look"
        help="How the window looks."
        kept="Kept in purlis.toml, which your team sees."
        file="purlis.toml"
      >
        <SettingGroup label="Text" level={3} help="The size of the window's words.">
          <SettingRow
            label="Text size"
            help="Every word scales with it."
            control={({ id }) => <input id={id} />}
          />
        </SettingGroup>
      </SettingGroup>
    </SettingsLayout>,
  ).container;
}

describe("Settings' words, each on its text role", () => {
  it("titles a page with the title and says what it is and where it is kept as secondary", () => {
    const drawn = page();
    wears(screen.getByRole("heading", { level: 2, name: "Look" }), "title", "a page's title");
    wears(drawn.querySelector(".ui-setting-blurb"), "secondary", "a page's blurb");
    wears(drawn.querySelector(".ui-setting-where"), "secondary", "where a page is kept");
    const file = drawn.querySelector(".ui-setting-where code");
    expect(file).toHaveClass("ui-text-code");
    expect(typeOf(file as Element).colour).toBe("--text-secondary");
    wears(drawn.querySelector(".ui-settings-about"), "secondary", "the level's about line");
  });

  it("heads a page's group with the heading and its line and a row's help as secondary", () => {
    page();
    wears(screen.getByRole("heading", { level: 3, name: "Text" }), "heading", "a group's heading");
    const help = screen.getAllByText(/The size of the window's words|Every word scales/);
    expect(help).toHaveLength(2);
    for (const line of help) wears(line, "secondary", `the help "${line.textContent}"`);
  });

  it("keeps a row's label at the body's colour and the medium weight, the one word with no role", () => {
    page();
    const label = screen.getByText("Text size");
    expect(label.className).not.toMatch(/ui-text-/);
    expect(drawnWith(label, "font-weight")).toBe("var(--font-weight-medium)");
  });

  it("writes no size or colour of words of its own for what now wears a role", () => {
    const worn =
      /^\.ui-(?:settings-about|settings-raw-label|setting-blurb|setting-where|setting-help|record-where|collection-done|machine-path)\b|^\.ui-setting-head h2$|^\.ui-collection-head h3$|^\.ui-settings-page .ui-setting-section > h3$/;
    const typed = declarations(readFileSync(join(process.cwd(), "src", "App.css"), "utf8"))
      .filter(({ context }) => worn.test(context[context.length - 1] ?? ""))
      .filter(({ property }) =>
        ["font-size", "font-weight", "color", "font-family"].includes(property),
      )
      .map(({ context, property, value }) => `${context.join(" ")} | ${property}: ${value}`);
    expect(typed).toEqual([]);
  });
});
