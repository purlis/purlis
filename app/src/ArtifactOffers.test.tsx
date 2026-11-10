// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Offer } from "./actions";
import { ARTIFACT_OFFERS, ArtifactOffers, type ArtifactOffersHeard } from "./ArtifactOffers";
import { resolve } from "./pageOffers";
import { todoView } from "./todos";

/** The catalogue's todo rows for workspace `ws`, as `actions.catalogue` builds them. */
function catalogue(ws: string): Map<string, Offer> {
  const rows: Offer[] = [
    {
      id: "todo.open:fix-login",
      title: "Open todo: Fix login",
      available: true,
      reason: "",
      does: {
        verb: "openView",
        view: todoView({ workspace: ws, slug: "fix-login" }),
        title: "Fix login",
      },
    },
    {
      id: "todo.done:fix-login",
      title: "Mark done: Fix login",
      available: true,
      reason: "",
      does: { verb: "closeTodo", workspace: ws, slug: "fix-login" },
    },
    {
      id: "todo.delete:old-one",
      title: "Delete done todo Old one",
      available: true,
      reason: "",
      does: { verb: "forgetDoneTodo", workspace: ws, slug: "old-one" },
      note: `Deletes it from ${ws}'s done todos with nothing journalled. There is no undo.`,
    },
    {
      id: "workspace.remove:alpha",
      title: "Delete workspace alpha…",
      available: true,
      reason: "",
      does: { verb: "removeWorkspace", workspace: "alpha" },
    },
  ];
  return new Map(rows.map((row) => [row.id, row]));
}

const HERE = catalogue("alpha");
const offerFor = (id: string) => HERE.get(id);

async function offer(offers: ArtifactOffersHeard["offers"], label = "artifact-3") {
  await emit(ARTIFACT_OFFERS, { label, offers } satisfies ArtifactOffersHeard);
}

function bar(onPress = vi.fn()) {
  render(
    <ArtifactOffers
      label="artifact-3"
      workspace="alpha"
      name="Todo board"
      offerFor={offerFor}
      onPress={onPress}
    />,
  );
  return onPress;
}

beforeEach(() => {
  mockIPC(() => null, { shouldMockEvents: true });
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

describe("what a page may offer", () => {
  it("is the catalogue's own row, in its words, for the artifact's workspace", () => {
    const drawn = resolve({ does: "todo.done", subject: "fix-login" }, "alpha", offerFor);
    expect(drawn).toEqual({ key: "todo.done:fix-login", offer: HERE.get("todo.done:fix-login") });
    expect(resolve({ does: "todo.open", subject: "fix-login" }, "alpha", offerFor)).toHaveProperty(
      "offer",
    );
  });

  it("asks first for a delete", () => {
    expect(resolve({ does: "todo.delete", subject: "old-one" }, "alpha", offerFor)).toHaveProperty(
      "asks",
      "Delete",
    );
  });

  it("refuses an id a page may not offer, though the catalogue has the row", () => {
    expect(
      resolve({ does: "workspace.remove", subject: "alpha" }, "alpha", offerFor),
    ).toHaveProperty(
      "refused",
      "A page cannot offer workspace.remove, so purlis did not draw it. Change the page to offer one of todo.open, todo.done, todo.reopen, todo.forget or todo.delete",
    );
    expect(resolve({ does: "constructor", subject: "x" }, "alpha", offerFor)).toHaveProperty(
      "refused",
    );
  });

  it("refuses a row the catalogue lacks, or built for another workspace", () => {
    expect(resolve({ does: "todo.done", subject: "nope" }, "alpha", offerFor)).toHaveProperty(
      "refused",
      "This page offered todo.done for nope, which alpha does not have now",
    );
    const elsewhere = catalogue("beta");
    expect(
      resolve({ does: "todo.done", subject: "fix-login" }, "alpha", (id) => elsewhere.get(id)),
    ).toHaveProperty("refused");
    expect(
      resolve({ does: "todo.open", subject: "fix-login" }, "alpha", (id) => elsewhere.get(id)),
    ).toHaveProperty("refused");
  });
});

describe("the offers on an artifact's bar", () => {
  it("draws nothing until its page offers", () => {
    bar();
    expect(screen.queryByRole("group")).toBeNull();
  });

  it("draws an offer in purlis's words, and only the person's press runs its row", async () => {
    const onPress = bar();
    await offer([{ does: "todo.done", subject: "fix-login" }]);
    const button = await screen.findByRole("button", { name: "Mark done: Fix login" });
    expect(screen.getByRole("group", { name: "Offered by Todo board" })).toContainElement(button);
    expect(onPress).not.toHaveBeenCalled();
    await userEvent.click(button);
    expect(onPress).toHaveBeenCalledWith(HERE.get("todo.done:fix-login"));
  });

  it("asks before a delete, and Cancel does nothing", async () => {
    const onPress = bar();
    await offer([{ does: "todo.delete", subject: "old-one" }]);
    await userEvent.click(await screen.findByRole("button", { name: "Delete done todo Old one…" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete done todo Old one?" });
    expect(dialog.textContent).toContain("Todo board offered this.");
    expect(dialog.textContent).toContain("There is no undo.");
    expect(onPress).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(onPress).not.toHaveBeenCalled();
  });

  it("deletes once the question is answered", async () => {
    const onPress = bar();
    await offer([{ does: "todo.delete", subject: "old-one" }]);
    await userEvent.click(await screen.findByRole("button", { name: "Delete done todo Old one…" }));
    await userEvent.click(await screen.findByRole("button", { name: "Delete" }));
    expect(onPress).toHaveBeenCalledWith(HERE.get("todo.delete:old-one"));
  });

  it("says why it drew none of a refused offer", async () => {
    bar();
    await offer([{ does: "workspace.remove", subject: "alpha" }]);
    expect(await screen.findByTestId("artifact-offer-refused")).toHaveTextContent(
      "A page cannot offer workspace.remove",
    );
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("hears only its own page, and a new set replaces the last", async () => {
    bar();
    await offer([{ does: "todo.done", subject: "fix-login" }], "artifact-9");
    await offer([{ does: "todo.done", subject: "fix-login" }]);
    await screen.findByRole("button", { name: "Mark done: Fix login" });
    await offer([]);
    await waitFor(() => expect(screen.queryByRole("button")).toBeNull());
  });
});
