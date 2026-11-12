// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { declarations } from "../guards.testkit";
import { ROLES, typeOf, wears, type Role } from "../textRoles.testkit";
import { Dialog } from "./dialog/Dialog";
import { NoticeSub, NoticeTitle, NoticeWhose } from "./notice/Notice";

/**
 * **A dialog's and a Notice's words wear the text roles** (#1920, the sixth surface to move;
 * `docs/design-system.md` "Text roles"). Each family ranked its own: a dialog's title at the
 * title's size by a rule of its own, its subtitle muted, its description secondary at the
 * dialog's 14px; a Notice's title at the medium weight, and its sub-line muted at 12px under a
 * meta class that rule outranked. Each is now its role, held here through the window's cascade.
 */

afterEach(cleanup);

describe("a dialog's words, each on its text role", () => {
  it("titles it with the title; its subtitle and summary are the secondary's size, its description the body's, each in its tier", () => {
    render(
      <Dialog
        onClose={() => {}}
        title="New workspace"
        subtitle="in purlis"
        description="A workspace holds its repos and its chats."
        summary="Creates one folder"
        main={{ children: "Create" }}
      >
        <p>What it is for.</p>
      </Dialog>,
    );
    const dialog = screen.getByRole("dialog", { name: "New workspace" });
    wears(dialog.querySelector('[data-part="title"]'), "title", "the dialog's title");
    // The rest wear their role's size and weight in the dialog's own tiers, which the parts
    // manifest measures each on the dialog's fill (`DIALOG_TIERS`, `parts_dialog.rs`).
    const tiered: [part: string, role: Role, tier: string][] = [
      ["subtitle", "secondary", "--text-muted"],
      ["description", "body", "--text-secondary"],
      ["summary", "secondary", "--text-quiet"],
    ];
    for (const [part, role, tier] of tiered) {
      const word = dialog.querySelector(`[data-part="${part}"]`);
      expect(word, part).toHaveClass(ROLES[role].class);
      expect(typeOf(word as Element), part).toEqual({
        size: ROLES[role].size,
        weight: ROLES[role].weight,
        colour: tier,
      });
    }
  });
});

describe("a Notice's words, each on its text role", () => {
  it("draws a card's title as the body's strong line, whose it is strong and its sub-line as the meta", () => {
    render(
      <div data-scope="notice" data-part="root">
        <NoticeTitle>Update ready</NoticeTitle>
        <NoticeWhose>steward 4</NoticeWhose>
        <NoticeSub>asks · 2m</NoticeSub>
      </div>,
    );
    wears(screen.getByText("Update ready"), "body", "a card's title", true);
    const whose = screen.getByText("steward 4");
    expect(whose).toHaveClass("ui-text-strong");
    expect(typeOf(whose).weight).toBe("600");
    wears(screen.getByText("asks · 2m"), "meta", "a row's sub-line");
  });
});

describe("the families' own sheets", () => {
  it("write no size or weight for a word that now wears a role", () => {
    const worn =
      /\[data-scope="(?:dialog|notice)"\]\[data-part="(?:title|subtitle|description|summary|sub|whose)"\]$/;
    const typed = ["ui/dialog/dialog.css", "ui/notice/notice.css"].flatMap((file) =>
      declarations(readFileSync(join(process.cwd(), "src", file), "utf8"))
        .filter(({ context }) => worn.test(context[context.length - 1] ?? ""))
        // A colour is a family's tier, which the parts manifest reads from its sheet.
        .filter(({ property }) => ["font-size", "font-weight"].includes(property))
        .map(
          ({ context, property, value }) => `${file} ${context.join(" ")} | ${property}: ${value}`,
        ),
    );
    expect(typed).toEqual([]);
  });
});
