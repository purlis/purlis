// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "./cascade.testkit";
import { Notice, NoticeInCentre, NoticeList, NoticeOf, type NoticeProps } from "./Notice";

/**
 * **The Notices centre's cards take the Inbox card's style** (#5217, spec #5191): a one-line head
 * with the notice's mark, its title, its state and age muted beneath, Go to chat as a quiet link
 * and a ×; purlis's sentence under it; one primary button where the notice has an answer, and
 * every other way a quiet link. A card waiting on consent keeps its full waiting edge (ADR 0097
 * §6), and its answers keep their equal weight.
 */

afterEach(cleanup);

/** A Notice as the centre's list draws it, with the card it stands in. */
async function inTheCentre(props: NoticeProps, whose?: { whose: string; onGo: () => void }) {
  render(
    <NoticeList>
      <NoticeInCentre.Provider value={whose === undefined ? null : () => {}}>
        <NoticeOf.Provider value={whose ?? null}>
          <Notice {...props} />
        </NoticeOf.Provider>
      </NoticeInCentre.Provider>
    </NoticeList>,
  );
  await waitFor(() =>
    expect(document.querySelector('[data-scope="notice"][data-placement="inbox"]')).not.toBeNull(),
  );
  return document.querySelector('[data-scope="notice"][data-placement="inbox"]') as HTMLElement;
}

const part = (card: HTMLElement, name: string) =>
  card.querySelector<HTMLElement>(`[data-scope="notice"][data-part="${name}"]`);

describe("a note in the Notices centre (#5217)", () => {
  it("heads with whose it is, its state beneath, Go to chat as a quiet link, and a ×", async () => {
    const go = vi.fn();
    const dismiss = vi.fn();
    const card = await inTheCentre(
      {
        cause: "mcp:4",
        at: "pane",
        tone: "trouble",
        onDismiss: dismiss,
        children: "Its MCP server did not start.",
      },
      { whose: "steward 4", onGo: go },
    );

    const head = within(part(card, "head") as HTMLElement);
    expect(part(card, "title")).toHaveTextContent("steward 4");
    expect(part(card, "sub")).toHaveTextContent("Problem");
    expect(part(card, "head")).not.toHaveTextContent("Its MCP server");
    expect(part(card, "message")).toHaveTextContent("Its MCP server did not start.");

    const toChat = head.getByRole("button", { name: "Go to chat steward 4" });
    expect(toChat).toHaveAttribute("data-variant", "quiet-link");
    await userEvent.click(toChat);
    expect(go).toHaveBeenCalledOnce();

    await userEvent.click(head.getByRole("button", { name: "Dismiss" }));
    expect(dismiss).toHaveBeenCalledOnce();
  });

  it("heads with its state where it names nothing else, said once", async () => {
    const card = await inTheCentre({
      cause: "pin-forgotten:ide",
      fixes: [{ label: "Undo", onPress: () => {} }],
      children: "Forgot the pin to ide.",
    });

    expect(part(card, "title")).toHaveTextContent(/^Note$/);
    expect(part(card, "sub")).toHaveTextContent(/^now$/);
  });

  it("heads with its label where it has one", async () => {
    const card = await inTheCentre({
      cause: "runner-key",
      label: "Runner key changed",
      onDismiss: () => {},
      children: "The runner's key changed.",
    });

    expect(part(card, "title")).toHaveTextContent("Runner key changed");
    expect(part(card, "sub")).toHaveTextContent("Note");
    expect(screen.getByRole("status", { name: "Runner key changed" })).toBe(
      card.querySelector('[role="status"]'),
    );
  });

  it("takes no whole sentence for its title: a long label only names it for a reader", async () => {
    const said = "While you were away, 3 chats finished and one is waiting on you";
    const card = await inTheCentre({
      cause: "away-summary",
      label: said,
      onDismiss: () => {},
      children: said,
    });
    expect(part(card, "title")).toHaveTextContent(/^Note$/);
    expect(screen.getByRole("status", { name: said })).toBe(card.querySelector('[role="status"]'));
  });

  it("says its age beside its state once it has stood a while", async () => {
    vi.useFakeTimers({ toFake: ["Date", "setInterval", "clearInterval"], now: 1_000_000 });
    try {
      const card = await inTheCentre({
        cause: "aged",
        label: "Aged",
        onDismiss: () => {},
        children: "It stands.",
      });
      expect(part(card, "sub")).toHaveTextContent(/^Note · now$/);
      act(() => vi.advanceTimersByTime(5 * 60_000));
      expect(part(card, "sub")).toHaveTextContent(/^Note · 5m$/);
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps its state and age muted in every tone, and never reads the card out for its age", async () => {
    const card = await inTheCentre({
      cause: "muted",
      tone: "trouble",
      label: "Muted",
      onDismiss: () => {},
      children: "It failed.",
    });
    const sub = part(card, "sub") as HTMLElement;
    expect(drawnWith(sub, "color")).toBe("var(--text-muted)");
    expect(sub).toHaveAttribute("aria-live", "off");
  });

  it("has one primary button where it has an answer, and every other way is a quiet link", async () => {
    const card = await inTheCentre({
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

    const ways = within(card);
    expect(ways.getByRole("button", { name: "Retry" })).toHaveAttribute("data-variant", "primary");
    for (const name of ["Open log", "Open Settings", "Copy command"])
      expect(ways.getByRole("button", { name })).toHaveAttribute("data-variant", "quiet-link");
    expect(card.querySelectorAll('[data-variant="primary"]')).toHaveLength(1);
  });

  it("draws a quiet link in its tone's words on trouble's and attention's grounds", async () => {
    for (const [tone, words] of [
      ["trouble", "var(--danger-text)"],
      ["attention", "var(--attention-text)"],
    ] as const) {
      const card = await inTheCentre({
        cause: `quiet:${tone}`,
        tone,
        link: { label: "See its settings", onPress: () => {} },
        children: "It stands.",
      });
      const link = within(card).getByRole("button", { name: "See its settings" });
      expect(drawnWith(link, "color"), tone).toBe(words);
      cleanup();
    }
  });

  it("draws its mark in its tone's colour, though the mark stands in its head", async () => {
    for (const [tone, colour] of [
      ["trouble", "var(--danger-text)"],
      ["attention", "var(--attention-text)"],
      ["news", "var(--state-success)"],
    ] as const) {
      const card = await inTheCentre({
        cause: `mark:${tone}`,
        tone,
        onDismiss: () => {},
        children: "It stands.",
      });
      expect(drawnWith(part(card, "mark") as HTMLElement, "color"), tone).toBe(colour);
      cleanup();
    }
  });

  it("sets a quiet link among its ways under its words, with no room of its own before it", async () => {
    const card = await inTheCentre({
      cause: "aligned",
      link: { label: "Open the pins", onPress: () => {} },
      children: "It stands.",
    });
    const link = within(card).getByRole("button", { name: "Open the pins" });
    expect(drawnWith(link, "padding-inline")).toBe("0");
  });

  it("leads with the way the caller named, where it names one", async () => {
    const card = await inTheCentre({
      cause: "named-way",
      title: "It stopped",
      fixes: [
        { label: "Later", onPress: () => {} },
        { label: "Start it", onPress: () => {}, primary: true },
      ],
      children: "It stopped.",
    });

    expect(within(card).getByRole("button", { name: "Start it" })).toHaveAttribute(
      "data-variant",
      "primary",
    );
    expect(within(card).getByRole("button", { name: "Later" })).toHaveAttribute(
      "data-variant",
      "quiet-link",
    );
  });

  it("draws purlis's sentence as plain words, never as markdown", async () => {
    const card = await inTheCentre({
      cause: "plain",
      onDismiss: () => {},
      children: "It wrote **this** to the log.",
    });

    expect(part(card, "message")).toHaveTextContent("It wrote **this** to the log.");
    expect(part(card, "message")?.querySelector("strong")).toBeNull();
  });
});

describe("an arrival card in the Notices centre (#4759, #5217)", () => {
  it("takes Go to chat into its head as a quiet link, with no primary button", async () => {
    const go = vi.fn();
    const card = await inTheCentre({
      cause: "result:chat.arrived:1",
      title: "steward 6 arrived from steward 4",
      quiet: true,
      fixes: [{ label: "Go to it", onPress: go, primary: true, go: true }],
      onDismiss: () => {},
      children: undefined,
    });

    expect(part(card, "title")).toHaveTextContent("steward 6 arrived from steward 4");
    const toChat = within(part(card, "head") as HTMLElement).getByRole("button", {
      name: "Go to chat",
    });
    expect(toChat).toHaveAttribute("data-variant", "quiet-link");
    expect(card.querySelector('[data-variant="primary"]')).toBeNull();
    await userEvent.click(toChat);
    expect(go).toHaveBeenCalledOnce();
  });
});

describe("a card waiting on consent in the Notices centre (ADR 0097 §6, #5217)", () => {
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

  it("takes no primary: Allow grants, Keep blocked is as plain beside it, the rest quiet", async () => {
    const card = await inTheCentre(consent, { whose: "steward 4", onGo: () => {} });
    const ways = within(card);

    expect(ways.getByRole("button", { name: "Allow" })).toHaveAttribute("data-variant", "grants");
    expect(ways.getByRole("button", { name: "Keep blocked" })).toHaveAttribute(
      "data-variant",
      "secondary",
    );
    expect(ways.getByRole("button", { name: "Details" })).toHaveAttribute(
      "data-variant",
      "quiet-link",
    );
    expect(card.querySelector('[data-variant="primary"]')).toBeNull();
    expect(part(card, "sub")).toHaveTextContent("Waiting on you");
  });

  it("keeps its full waiting edge, where a note's outline is softer in its kind's colour", async () => {
    const card = await inTheCentre(consent, { whose: "steward 4", onGo: () => {} });
    expect(card).toHaveAttribute("data-consent");
    expect(drawnWith(card, "border-color")).toBe("var(--state-waiting)");
    cleanup();

    const trouble = await inTheCentre({
      cause: "soft:trouble",
      tone: "trouble",
      onDismiss: () => {},
      children: "It failed.",
    });
    expect(trouble).not.toHaveAttribute("data-consent");
    expect(drawnWith(trouble, "border-color")).toBe(
      "color-mix(in oklab, var(--danger-text) 40%, transparent)",
    );
    cleanup();

    const news = await inTheCentre({
      cause: "soft:news",
      onDismiss: () => {},
      children: "It is done.",
    });
    expect(drawnWith(news, "border-color")).toBe(
      "color-mix(in oklab, var(--state-success) 40%, transparent)",
    );
  });

  it("says it waits on you only where there is something to answer, and keeps its edge", async () => {
    const card = await inTheCentre(
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
    expect(part(card, "sub")).toHaveTextContent(/^Problem/);
    expect(drawnWith(card, "border-color")).toBe("var(--state-waiting)");
  });

  it("is a consent whenever a way out allows, though its caller says nothing more", async () => {
    const card = await inTheCentre({
      cause: "grants",
      fixes: [{ label: "Allow", onPress: () => {}, allows: true }],
      children: "Allow it?",
    });
    expect(card).toHaveAttribute("data-consent");
  });
});

describe("the Notices centre's list (#5217)", () => {
  it("draws no box of its own: each Notice is its own card", async () => {
    await inTheCentre({ cause: "one", onDismiss: () => {}, children: "One." });
    const list = document.querySelector('[data-scope="notice-list"]') as HTMLElement;
    expect(drawnWith(list, "border-width")).toBe("0");
    expect(drawnWith(list, "background-color")).toBe("transparent");
    expect(drawnWith(list, "gap")).toBe("var(--space-2)");
  });
});

describe("a Notice out of the Notices centre (#5217)", () => {
  it("stays as it was on a pane's band", () => {
    render(
      <Notice cause="band" at="pane" onDismiss={() => {}}>
        On the band.
      </Notice>,
    );
    const box = document.querySelector('[data-placement="pane"]') as HTMLElement;
    expect(part(box, "head")).toBeNull();
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
    expect(part(box, "head")).toBeNull();
  });
});
