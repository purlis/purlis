// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Palette } from "./Palette";
import type { Offer, Ran } from "./actions";

/**
 * **The palette and the key registry** (#2921): each row says the key the registry holds for
 * its act at its end, and its note stops saying it in words; and the palette's keys do nothing
 * while a modal dialog is up (the operator's ruling of 2026-10-24).
 */

afterEach(() => {
  cleanup();
  document.body.replaceChildren();
});

const ready = (id: string, title: string, note?: string) =>
  ({ id, title, note, available: true, reason: "", does: { verb: "nothing" } }) satisfies Offer;

const ok = (): Ran => ({ ok: true });

const OFFERS: Offer[] = [
  ready("shell.new", "New shell", "Your own shell, where a new tab would start. Ctrl+Shift+T."),
  ready("extensions.show", "Extensions…"),
];

describe("the palette's rows say their keys (N25)", () => {
  it("draws the registry's key at a row's end, as a reader is told it, and not in the note", async () => {
    render(<Palette offers={OFFERS} onRun={ok} />);
    await userEvent.keyboard("{F2}");
    const shell = await screen.findByRole("option", { name: /New shell/ });
    expect(shell.querySelector('[data-part="shortcut"]')).toHaveTextContent("Ctrl+Shift+T");
    expect(shell).toHaveAttribute("aria-keyshortcuts", "Control+Shift+T");
    expect(shell.querySelector(".palette-why")).toHaveTextContent(
      /^Your own shell, where a new tab would start\.$/,
    );
    const none = screen.getByRole("option", { name: /Extensions/ });
    expect(none.querySelector('[data-part="shortcut"]')).toBeNull();
    expect(none).not.toHaveAttribute("aria-keyshortcuts");
  });
});

describe("the palette's keys under a modal", () => {
  it("opens on nothing while another modal dialog is up", async () => {
    render(<Palette offers={OFFERS} onRun={ok} />);
    const modal = document.createElement("div");
    modal.dataset.scope = "dialog";
    modal.dataset.part = "root";
    document.body.append(modal);
    await userEvent.keyboard("{F2}");
    await userEvent.keyboard("{Control>}k{/Control}");
    expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
    modal.remove();
    await userEvent.keyboard("{F2}");
    expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
  });

  it("keeps its own keys while it is the modal that is up", async () => {
    render(<Palette offers={OFFERS} onRun={ok} />);
    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
  });
});
