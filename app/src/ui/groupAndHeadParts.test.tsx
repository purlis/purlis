// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { cascadeOf, drawnWith } from "../cascade.testkit";

/**
 * **A group's name and a column's name carry the section label as a part, not site by site**
 * (#6482, the leftovers of #1920's label spelling). A part that each site dressed by hand is one
 * a new site can miss, as a fold's group name did in #6473: so the list and tree families draw
 * their group header from one function, the table families their column name from one, and the
 * look of a group named by data is written in one rule, which the tree takes from the list.
 */

afterEach(cleanup);

const UI = join(process.cwd(), "src", "ui");
const source = (...files: string[]) =>
  files.map((file) => readFileSync(join(UI, file), "utf8")).join("\n");
/** How often `pattern` is written in `text`. */
const times = (text: string, pattern: RegExp) => [...text.matchAll(pattern)].length;

/** The label role's class, as a JSX class or a prop's value. */
const LABEL = /["']ui-text-label["']/g;

describe("a group named by data, in a list and a tree alike", () => {
  const listSheet = cascadeOf("ui/list/list.css");
  const treeSheet = cascadeOf("ui/tree/tree.css");

  function header(scope: "list" | "tree", framed: boolean) {
    const { container } = render(
      <div {...(framed ? { "data-scope": "panel-section", "data-frame": "" } : {})}>
        <div className="ui-text-label" data-scope={scope} data-part="header" data-named="">
          2026-11-13
        </div>
      </div>,
    );
    return container.querySelector('[data-part="header"]') as Element;
  }

  it("is written as its name by the list family's one rule, which a tree's header takes too", () => {
    for (const scope of ["list", "tree"] as const) {
      const named = header(scope, false);
      expect(listSheet(named, "text-transform"), `a ${scope}'s named group`).toBe("none");
      expect(treeSheet(named, "text-transform"), `tree.css on a ${scope}'s`).toBeUndefined();
      expect(drawnWith(named, "text-transform"), `a ${scope}'s, drawn`).toBe("none");
      cleanup();
    }
  });

  it("is the section label all the same where a frame labels its groups", () => {
    for (const scope of ["list", "tree"] as const) {
      const framed = header(scope, true);
      expect(listSheet(framed, "text-transform"), `a framed ${scope}'s`).toBeUndefined();
      expect(drawnWith(framed, "text-transform"), `a framed ${scope}'s, drawn`).toBe("uppercase");
      cleanup();
    }
  });
});

describe("the parts that carry the section label", () => {
  it("draws a list's, a fold's and a tree's group header from one function", () => {
    const families = source("list/List.tsx", "list/Fold.tsx", "tree/Tree.tsx");
    expect(times(families, LABEL), "ui-text-label written").toBe(1);
    expect(times(families, /data-named/g), "data-named written").toBe(1);
  });

  it("draws every column name of a Table and a ControlTable from one head part", () => {
    const families = source("table/Table.tsx", "table/ControlTable.tsx");
    expect(times(families, LABEL), "ui-text-label written").toBe(1);
    expect(
      times(families, /["']?data-part["']?\s*[=:]\s*["']head["']/g),
      'data-part="head" written',
    ).toBe(1);
  });
});
