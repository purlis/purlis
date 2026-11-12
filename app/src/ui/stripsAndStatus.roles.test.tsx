// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { drawnWith } from "../cascade.testkit";
import { declarations } from "../guards.testkit";
import { typeOf, ROLES } from "../textRoles.testkit";
import { TabCell } from "./tabs/Tabs";
import { PageTabs } from "./tabs/PageTabs";
import { StatusBar } from "./status/StatusBar";
import { StatusCell, StatusItem } from "./status/StatusItem";

/**
 * **The tab strips and the status bar wear the text roles** (#1920, the fourth surface to move;
 * `docs/design-system.md` "Text roles"). Their words already sat on the scale, each family
 * writing its own token; now each wears its role, and its family keeps only the tier its state
 * gives the words (quiet at rest, primary under the pointer and chosen), which outranks the
 * role's colour, so a theme's parts and a tone still tint them.
 */

afterEach(cleanup);

/** A word's size and weight, the role's, whatever colour its family's tier gives it. */
function sizedAs(word: Element | null, role: keyof typeof ROLES, what: string) {
  expect(word, `${what} was not drawn`).toBeTruthy();
  const { size, weight } = typeOf(word as Element);
  expect({ size, weight }, what).toEqual({ size: ROLES[role].size, weight: ROLES[role].weight });
}

describe("the tab strips' words, each on its text role", () => {
  it("draws a tab's words at the body's size, in the quiet tier at rest and primary chosen", () => {
    render(
      <div>
        <TabCell variant="chat" selected={false} data-testid="rest">
          <span>steward 4</span>
        </TabCell>
        <TabCell variant="chat" selected data-testid="chosen" className="tab">
          <span>talk</span>
        </TabCell>
      </div>,
    );
    const rest = screen.getByTestId("rest");
    const chosen = screen.getByTestId("chosen");
    expect(rest).toHaveClass("ui-text-body");
    // A screen's own `className` still places it, beside the role.
    expect(chosen).toHaveClass("ui-text-body", "tab");
    sizedAs(rest, "body", "a tab at rest");
    sizedAs(chosen, "body", "the chosen tab");
    expect(drawnWith(rest, "color")).toBe("var(--text-quiet)");
    expect(drawnWith(chosen, "color")).toBe("var(--text-primary)");
  });

  it("draws a page tab's words as the body and its count as the meta", () => {
    render(
      <PageTabs
        label="deploy's sections"
        tabs={[
          { id: "overview", label: "Overview" },
          { id: "history", label: "History", count: 4 },
        ]}
        selected="overview"
        onSelect={() => {}}
      >
        {(id) => <p>{id}</p>}
      </PageTabs>,
    );
    const history = screen.getByRole("tab", { name: "History" });
    expect(history).toHaveClass("ui-text-body");
    sizedAs(history, "body", "a page tab");
    expect(drawnWith(history, "color")).toBe("var(--text-muted)");
    expect(drawnWith(screen.getByRole("tab", { name: "Overview" }), "color")).toBe(
      "var(--text-primary)",
    );
    const count = history.querySelector('[data-part="count"]');
    expect(count).toHaveClass("ui-text-meta");
    expect(typeOf(count as Element)).toEqual({
      size: ROLES.meta.size,
      weight: ROLES.meta.weight,
      colour: ROLES.meta.colour,
    });
  });

  it("says a task's name after the session's in the meta's colour, at the tab's size", () => {
    render(
      <TabCell variant="chat" selected data-testid="tab">
        <span className="tab-name">
          steward 4<span className="tab-task">talk</span>
        </span>
      </TabCell>,
    );
    const task = screen.getByText("talk");
    expect(drawnWith(task, "color")).toBe("var(--text-quiet)");
    sizedAs(task, "body", "a task's name in its tab");
  });
});

describe("the status bar's words, each on its text role", () => {
  it("draws every item's words at the secondary's size, in the item's own tier", () => {
    render(
      <StatusBar
        aria-label="Status"
        items={[
          {
            id: "purlis.project",
            name: "Project",
            side: "left",
            priority: 1,
            item: (
              <StatusCell strong data-testid="project">
                purlis
              </StatusCell>
            ),
          },
          {
            id: "purlis.todos",
            name: "Todos",
            side: "right",
            priority: 1,
            item: (
              <StatusItem count={3} data-testid="todos" aria-label="Todos">
                Todos
              </StatusItem>
            ),
          },
        ]}
      />,
    );
    const bar = screen.getByRole("contentinfo", { name: "Status" });
    expect(bar).toHaveClass("ui-text-secondary");
    for (const id of ["project", "todos"]) {
      const item = screen.getByTestId(id);
      expect(item, id).toHaveClass("ui-text-secondary");
      sizedAs(item, "secondary", `the ${id} item`);
      // The item's tier, not the role's: a tone, a variant and a theme's parts tint it.
      expect(drawnWith(item, "color"), id).toBe("var(--text-primary)");
    }
    // The project's name is the one strong word of the bar.
    const project = screen.getByTestId("project").querySelector('[data-part="label"]');
    expect(typeOf(project as Element).weight).toBe("600");
  });

  it("keeps a tone's colour over the role's", () => {
    render(
      <StatusCell tone="quiet" data-testid="stale">
        3 behind
      </StatusCell>,
    );
    expect(drawnWith(screen.getByTestId("stale"), "color")).toBe("var(--text-muted)");
  });
});

describe("the families' own sheets", () => {
  it("write no size of words of their own for a tab, a page tab or a status item", () => {
    const sized = ["ui/tabs/tabs.css", "ui/status/status.css"].flatMap((file) =>
      declarations(readFileSync(join(process.cwd(), "src", file), "utf8"))
        .filter(({ property }) => property === "font-size" || property === "font")
        .filter(({ context }) =>
          /\[data-scope="(?:tabs|page-tabs)"\]\[data-part="(?:tab|count)"\]$|data-scope="status-(?:bar|item)"\]?(?:\[data-part="root"\])?$/.test(
            context[context.length - 1] ?? "",
          ),
        )
        .map(
          ({ context, property, value }) => `${file} ${context.join(" ")} | ${property}: ${value}`,
        ),
    );
    expect(sized).toEqual([]);
  });
});
