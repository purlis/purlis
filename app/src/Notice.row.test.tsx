// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "./cascade.testkit";
import { Notice, NoticeInCentre, NoticeList, NoticeOf, type NoticeProps } from "./Notice";

/**
 * **A Notice is a row of the Notices view** (#5649, spec #5638, ADR 0119 decision 6; a card of its
 * own before, #5217): the message family's row, as Asks and Updates draw theirs. Its tone's glyph
 * in a tinted round mark; its title, with its age at the right and its × in that place under the
 * pointer; its state under it; purlis's sentence; its ways as text links, one leading in the
 * accent's words; no box. A Notice waiting on consent keeps its waiting edge and its answers'
 * equal weight (ADR 0097 §6).
 */

afterEach(cleanup);

/** A Notice as the Notices view's list draws it: its row. */
async function inTheView(props: NoticeProps, whose?: { whose: string; onGo: () => void }) {
  render(
    <NoticeList>
      <NoticeInCentre.Provider value={whose === undefined ? null : () => {}}>
        <NoticeOf.Provider value={whose ?? null}>
          <Notice {...props} />
        </NoticeOf.Provider>
      </NoticeInCentre.Provider>
    </NoticeList>,
  );
  const row = () =>
    document.querySelector<HTMLElement>(
      '[data-scope="notice-list"] [data-scope="message"][data-part="root"]',
    );
  await waitFor(() => expect(row()).not.toBeNull());
  return row() as HTMLElement;
}

const part = (row: HTMLElement, name: string) =>
  row.querySelector<HTMLElement>(`[data-scope="message"][data-part="${name}"]`);

describe("a Notice in the Notices view (#5649)", () => {
  it("is a row: whose it is, its state under it, Go to chat a link, and a × over its time", async () => {
    const go = vi.fn();
    const dismiss = vi.fn();
    const row = await inTheView(
      {
        cause: "mcp:4",
        at: "pane",
        tone: "trouble",
        onDismiss: dismiss,
        children: "Its MCP server did not start.",
      },
      { whose: "steward 4", onGo: go },
    );

    expect(part(row, "title")).toHaveTextContent("steward 4");
    expect(part(row, "meta")).toHaveTextContent(/^Problem$/);
    expect(part(row, "time")).toHaveTextContent(/^now$/);
    expect(part(row, "body")).toHaveTextContent("Its MCP server did not start.");
    // Its state's mark: trouble is failed, tinted, and hidden from a reader.
    expect(row).toHaveAttribute("data-variant", "failed");
    expect(part(row, "mark")).toHaveAttribute("aria-hidden", "true");

    const toChat = within(part(row, "links") as HTMLElement).getByRole("button", {
      name: "Go to chat steward 4",
    });
    expect(toChat).toHaveAttribute("data-variant", "quiet-link");
    await userEvent.click(toChat);
    expect(go).toHaveBeenCalledOnce();

    await userEvent.click(
      within(part(row, "dismiss") as HTMLElement).getByRole("button", { name: "Dismiss" }),
    );
    expect(dismiss).toHaveBeenCalledOnce();
  });

  it("draws no box, and no card remains around it", async () => {
    const row = await inTheView({ cause: "one", onDismiss: () => {}, children: "One." });
    expect(drawnWith(row, "border")).toBe("0");
    expect(drawnWith(row, "background-color")).toBe("transparent");
    expect(document.querySelector('[data-scope="card"]')).toBeNull();
    expect(document.querySelector('[data-scope="notice"][data-part="root"]')).toBeNull();
    const list = document.querySelector('[data-scope="notice-list"]') as HTMLElement;
    expect(drawnWith(list, "border-width")).toBe("0");
    expect(drawnWith(list, "background-color")).toBe("transparent");
  });

  it("gives its time's place to the × under the pointer or the keyboard", async () => {
    const row = await inTheView({ cause: "hover", onDismiss: () => {}, children: "It stands." });
    const time = part(row, "time") as HTMLElement;
    const dismiss = part(row, "dismiss") as HTMLElement;
    expect(row).toHaveAttribute("data-dismiss");
    expect(drawnWith(time, "visibility")).not.toBe("hidden");
    expect(drawnWith(dismiss, "opacity")).toBe("0");
    // The pointer on the row: the cascade's hover mark on the row itself, as the browser's.
    row.setAttribute("data-cascade-hover", "");
    expect(drawnWith(time, "visibility", { hover: true })).toBe("hidden");
    expect(drawnWith(dismiss, "opacity", { hover: true })).toBe("1");
    row.removeAttribute("data-cascade-hover");
  });

  it("titles itself with its state where it names nothing else, said once", async () => {
    const row = await inTheView({
      cause: "pin-forgotten:ide",
      fixes: [{ label: "Undo", onPress: () => {} }],
      children: "Forgot the pin to ide.",
    });

    expect(part(row, "title")).toHaveTextContent(/^Note$/);
    expect(part(row, "meta")).toBeNull();
    expect(row).toHaveAttribute("data-variant", "plain");
  });

  it("titles itself with its label where it has one, the row its live words", async () => {
    const row = await inTheView({
      cause: "runner-key",
      label: "Runner key changed",
      onDismiss: () => {},
      children: "The runner's key changed.",
    });

    expect(part(row, "title")).toHaveTextContent("Runner key changed");
    expect(part(row, "meta")).toHaveTextContent("Note");
    expect(screen.getByRole("status", { name: "Runner key changed" })).toContainElement(row);
  });

  it("takes no whole sentence for its title: a long label only names it for a reader", async () => {
    const said = "While you were away, 3 chats finished and one is waiting on you";
    await inTheView({ cause: "away-summary", label: said, onDismiss: () => {}, children: said });
    expect(screen.getByRole("status", { name: said })).toBeTruthy();
    expect(
      part(screen.getByRole("status", { name: said }), "title") as HTMLElement,
    ).toHaveTextContent(/^Note$/);
  });

  it("says its age at the right once it has stood a while, never reading the row out for it", async () => {
    vi.useFakeTimers({ toFake: ["Date", "setInterval", "clearInterval"], now: 1_000_000 });
    try {
      const row = await inTheView({
        cause: "aged",
        label: "Aged",
        onDismiss: () => {},
        children: "It stands.",
      });
      expect(part(row, "time")).toHaveTextContent(/^now$/);
      act(() => vi.advanceTimersByTime(5 * 60_000));
      expect(part(row, "time")).toHaveTextContent(/^5m$/);
      expect(part(row, "time")?.querySelector("time")).toHaveAttribute("aria-live", "off");
    } finally {
      vi.useRealTimers();
    }
  });

  it("draws every way as a text link, its one answer leading in the accent's words", async () => {
    const row = await inTheView({
      cause: "many-ways",
      fixes: [
        { label: "Retry", onPress: () => {} },
        { label: "Open log", onPress: () => {} },
      ],
      link: { label: "Open Settings", onPress: () => {} },
      copy: "purlis doctor",
      onDismiss: () => {},
      children: "It could not start.",
    });
    const links = within(part(row, "links") as HTMLElement);
    for (const name of ["Retry", "Open log", "Open Settings", "Copy command"])
      expect(links.getByRole("button", { name })).toHaveAttribute("data-variant", "quiet-link");
    expect(row.querySelector('[data-variant="primary"]')).toBeNull();
    const retry = links.getByRole("button", { name: "Retry" });
    expect(retry).toHaveAttribute("data-lead");
    expect(drawnWith(retry, "color")).toBe("var(--accent-text)");
    expect(row.querySelectorAll("[data-lead]")).toHaveLength(1);
  });

  it("leads with the way the caller named, where it names one", async () => {
    const row = await inTheView({
      cause: "named-way",
      title: "It stopped",
      fixes: [
        { label: "Later", onPress: () => {} },
        { label: "Start it", onPress: () => {}, primary: true },
      ],
      children: "It stopped.",
    });
    expect(within(row).getByRole("button", { name: "Start it" })).toHaveAttribute("data-lead");
    expect(within(row).getByRole("button", { name: "Later" })).not.toHaveAttribute("data-lead");
  });

  it("draws purlis's sentence as plain words, never as markdown", async () => {
    const row = await inTheView({
      cause: "plain",
      onDismiss: () => {},
      children: "It wrote **this** to the log.",
    });
    expect(part(row, "body")).toHaveTextContent("It wrote **this** to the log.");
    expect(part(row, "body")?.querySelector("strong")).toBeNull();
  });

  it("marks each tone's state by its mark's tint and its words, never its colour alone", async () => {
    for (const [tone, variant, colour] of [
      ["trouble", "failed", "var(--state-failed)"],
      ["attention", "waiting", "var(--state-waiting)"],
      ["news", "plain", "var(--text-secondary)"],
    ] as const) {
      const row = await inTheView({
        cause: `mark:${tone}`,
        tone,
        label: "Named",
        onDismiss: () => {},
        children: "It stands.",
      });
      expect(row, tone).toHaveAttribute("data-variant", variant);
      expect(drawnWith(part(row, "mark") as HTMLElement, "color"), tone).toBe(colour);
      expect(part(row, "meta")?.textContent, tone).not.toBe("");
      cleanup();
    }
  });
});

describe("an arrival in the Notices view (#4759, #5649)", () => {
  it("has Go to chat as a link, with no primary button", async () => {
    const go = vi.fn();
    const row = await inTheView({
      cause: "result:chat.arrived:1",
      title: "steward 6 arrived from steward 4",
      quiet: true,
      fixes: [{ label: "Go to it", onPress: go, primary: true, go: true }],
      onDismiss: () => {},
      children: undefined,
    });

    expect(part(row, "title")).toHaveTextContent("steward 6 arrived from steward 4");
    const toChat = within(part(row, "links") as HTMLElement).getByRole("button", {
      name: "Go to chat",
    });
    expect(toChat).toHaveAttribute("data-variant", "quiet-link");
    expect(row.querySelector('[data-variant="primary"]')).toBeNull();
    await userEvent.click(toChat);
    expect(go).toHaveBeenCalledOnce();
  });
});

describe("a Notice waiting on consent in the Notices view (ADR 0097 §6)", () => {
  const consent: NoticeProps = {
    cause: "sandbox-block:1",
    at: "pane",
    label: "Sandbox block",
    consent: true,
    tone: "trouble",
    fixes: [
      { label: "Allow", onPress: () => {}, allows: true },
      { label: "Keep blocked", onPress: () => {} },
      { label: "Details", onPress: () => {}, opens: { id: "x", open: false } },
    ],
    onDismiss: () => {},
    children: "The sandbox blocked a host.",
  };

  it("answers in the row: Allow grants, Keep blocked as plain beside it, what opens more a link", async () => {
    const row = await inTheView(consent, { whose: "steward 4", onGo: () => {} });
    const form = within(part(row, "form") as HTMLElement);

    expect(form.getByRole("button", { name: "Allow" })).toHaveAttribute("data-variant", "grants");
    expect(form.getByRole("button", { name: "Keep blocked" })).toHaveAttribute(
      "data-variant",
      "secondary",
    );
    expect(
      within(part(row, "links") as HTMLElement).getByRole("button", { name: "Details" }),
    ).toHaveAttribute("data-variant", "quiet-link");
    expect(row.querySelector('[data-variant="primary"], [data-lead]')).toBeNull();
    expect(part(row, "meta")).toHaveTextContent("Waiting on you");
    expect(row).toHaveAttribute("data-variant", "waiting");
  });

  it("keeps its waiting edge, which a note has none of", async () => {
    const row = await inTheView(consent, { whose: "steward 4", onGo: () => {} });
    expect(row).toHaveAttribute("data-consent");
    expect(drawnWith(row, "box-shadow")).toBe("inset var(--space-1) 0 0 var(--state-waiting)");
    cleanup();

    const note = await inTheView({ cause: "note", onDismiss: () => {}, children: "It is done." });
    expect(note).not.toHaveAttribute("data-consent");
  });

  it("says it waits on you only where there is something to answer, and keeps its edge", async () => {
    const row = await inTheView(
      {
        cause: "sandbox-block:never",
        at: "pane",
        label: "Sandbox block",
        consent: true,
        tone: "trouble",
        onDismiss: () => {},
        children: "The sandbox blocked it. purlis never allows that to a chat.",
      },
      { whose: "steward 4", onGo: () => {} },
    );
    expect(part(row, "meta")).toHaveTextContent(/^Problem/);
    expect(row).toHaveAttribute("data-consent");
  });

  it("is a consent whenever a way out allows, though its caller says nothing more", async () => {
    const row = await inTheView({
      cause: "grants",
      fixes: [{ label: "Allow", onPress: () => {}, allows: true }],
      children: "Allow it?",
    });
    expect(row).toHaveAttribute("data-consent");
  });
});

describe("a Notice out of the Notices view (#5649)", () => {
  it("stays as it was on a pane's band", () => {
    render(
      <Notice cause="band" at="pane" onDismiss={() => {}}>
        On the band.
      </Notice>,
    );
    const box = document.querySelector('[data-placement="pane"]') as HTMLElement;
    expect(box.querySelector('[data-scope="message"]')).toBeNull();
    expect(within(box).getByRole("button", { name: "Dismiss" }).textContent).toBe("Dismiss");
  });

  it("stays as it was alone on a page", () => {
    render(
      <Notice cause="alone" title="Done" onDismiss={() => {}}>
        Alone.
      </Notice>,
    );
    const box = document.querySelector('[data-placement="alone"]') as HTMLElement;
    expect(box).toHaveAttribute("data-layout", "card");
    expect(box.querySelector('[data-scope="message"]')).toBeNull();
  });
});
