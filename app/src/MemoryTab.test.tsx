// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { MemoryTab } from "./MemoryTab";
import { DRAFT, forgetDrafts, memoryKey, wantEdit, type MemoryRef } from "./memories";
import type { MemoryEdited, MemoryView } from "./bindings";
import { ReadOnlySaid } from "./ReadOnlyLine";
import { offeredIn, pickIn } from "./primitives/select.testkit";
import { retype, textOf, typeIn, typeable } from "./editor/typing.testkit";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetDrafts();
});

const PLANE = "/home/dev/plane";
const REF: MemoryRef = { scope: { kind: "persona", name: "steward" }, slug: "where-prod-1-is" };
const KEY = memoryKey(REF);

/** The body's text, once its editor has arrived (a chunk of its own, #1862); in the merge view,
 *  the edit's side. */
const body = () => screen.findByRole("textbox", { name: "Body" });
/** What the body holds, once it holds `text`. */
const holds = (text: string) => waitFor(async () => expect(textOf(await body())).toBe(text));

function memory(over: Partial<MemoryView> = {}): MemoryView {
  const title = over.title ?? "Where prod-1 is";
  const body = over.body ?? "It lives in **eu-west-1**.\n\n<script>alert(1)</script>";
  return {
    scope: REF.scope,
    slug: REF.slug,
    title,
    stamp: "2026-09-28 16:05",
    place: "steward",
    body,
    path: "personas/steward/memory/where-prod-1-is.md",
    text: `# ${title}\n\n_2026-09-28 16:05 · persistent_\n\n${body}\n`,
    ...over,
  };
}

type Asked = { cmd: string; args: Record<string, unknown> };

/** The core: `memory_read` answers `read` (in order, the last one repeated), and each write
 *  answers what `writes` says for it. */
function core(
  read: (MemoryView | null)[],
  writes: Record<string, unknown | ((args: Record<string, unknown>) => unknown)> = {},
): Asked[] {
  const asked: Asked[] = [];
  let reads = 0;
  mockIPC((cmd, args) => {
    const given = args as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "memory_read") return read[Math.min(reads++, read.length - 1)];
    const answer = writes[cmd];
    if (typeof answer === "function") return answer(given);
    if (answer instanceof Error) throw answer.message;
    return answer ?? null;
  });
  return asked;
}

function draw(
  over: {
    at?: MemoryRef;
    changed?: number;
    onSaved?: (memory: MemoryView) => void;
    onClose?: () => void;
    onOpenArchive?: () => void;
  } = {},
) {
  const onSaved = over.onSaved ?? vi.fn();
  const onClose = over.onClose ?? vi.fn();
  const onOpenArchive = over.onOpenArchive ?? vi.fn();
  const view = (changed: number) => (
    <MemoryTab
      plane={PLANE}
      at={over.at ?? REF}
      changed={changed}
      onSaved={onSaved}
      onClose={onClose}
      onOpenArchive={onOpenArchive}
    />
  );
  const drawn = render(view(over.changed ?? 0));
  return {
    onSaved,
    onClose,
    onOpenArchive,
    rerender: (changed: number) => drawn.rerender(view(changed)),
  };
}

describe("a memory's tab", () => {
  it("says where the memory is, and renders its body as Markdown with no HTML", async () => {
    core([memory()]);
    draw();

    const body = await screen.findByTestId("memory-body");
    expect(body.querySelector("strong")).toHaveTextContent("eu-west-1");
    expect(body.querySelector("script")).toBeNull();
    expect(body).not.toHaveTextContent("alert(1)");
    // The heading and the stamp line are the header's, not repeated in the body.
    expect(body).not.toHaveTextContent("Where prod-1 is");
    expect(body).not.toHaveTextContent("persistent");
    const meta = screen.getByTestId("memory-meta");
    expect(meta).toHaveTextContent("steward");
    expect(meta).toHaveTextContent("2026-09-28 16:05");
    expect(meta).toHaveTextContent("personas/steward/memory/where-prod-1-is.md");
  });

  it("says a memory that is not there any more is gone, rather than failing", async () => {
    core([null]);
    draw();

    expect(await screen.findByTestId("view-gone")).toHaveTextContent(/not here any more/);
  });

  it("offers the store's archive as a gone memory's way out (#1191)", async () => {
    core([null]);
    const { onOpenArchive } = draw();

    const gone = await screen.findByTestId("view-gone");
    await userEvent.click(within(gone).getByRole("button", { name: "Open steward's archive" }));

    expect(onOpenArchive).toHaveBeenCalledTimes(1);
  });

  it("reads the memory again when the plane changes on disk", async () => {
    const asked = core([memory(), memory({ body: "Moved to eu-west-2." })]);
    const { rerender } = draw();
    await screen.findByTestId("memory-body");

    rerender(1);

    await waitFor(() => expect(screen.getByTestId("memory-body")).toHaveTextContent("eu-west-2"));
    expect(asked.filter((one) => one.cmd === "memory_read")).toHaveLength(2);
  });
});

describe("editing a memory", () => {
  async function editing(read: MemoryView[] = [memory()], writes = {}) {
    const asked = core(read, writes);
    const drawn = draw();
    await screen.findByTestId("memory-body");
    act(() => wantEdit(PLANE, KEY));
    await screen.findByRole("textbox", { name: "Title" });
    return { asked, ...drawn };
  }

  it("flips the same tab into a title field and the raw body, and caps the title at 72", async () => {
    await editing();

    const title = screen.getByRole("textbox", { name: "Title" });
    expect(title).toHaveValue("Where prod-1 is");
    expect(title).toHaveAttribute("maxLength", "72");
    expect(title).toHaveAccessibleDescription("15 / 72");
    // The raw body, in the text editor with the Markdown grammar (#1862).
    await holds("It lives in **eu-west-1**.\n\n<script>alert(1)</script>");
    expect((await body()).closest('[data-testid="text-editor"]')).not.toBeNull();
    expect(typeable(await body())).toBe(true);
    expect(screen.queryByTestId("memory-body")).toBeNull();
  });

  it("opens straight into the editor when the edit was asked for before the read arrived", async () => {
    core([memory()]);
    act(() => wantEdit(PLANE, KEY));
    draw();

    expect(await screen.findByRole("textbox", { name: "Title" })).toHaveValue("Where prod-1 is");
  });

  it("saves against the text it read, and shows what was saved", async () => {
    const saved = memory({ title: "Where prod-1 lives", body: "eu-west-2 since May" });
    const { asked, onSaved } = await editing([memory()], {
      memory_edit: { kind: "saved", memory: saved } satisfies MemoryEdited,
    });

    await userEvent.clear(screen.getByRole("textbox", { name: "Title" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Title" }), "Where prod-1 lives");
    retype(await body(), "eu-west-2 since May");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(screen.getByTestId("memory-body")).toHaveTextContent("eu-west-2"));
    const edit = asked.find((one) => one.cmd === "memory_edit");
    expect(edit?.args).toMatchObject({
      scope: REF.scope,
      slug: REF.slug,
      title: "Where prod-1 lives",
      text: "eu-west-2 since May",
      read: memory().text,
      overwrite: false,
    });
    expect(onSaved).toHaveBeenCalledWith(saved);
  });

  it("keeps what was typed when the operator looks at another tab and comes back", async () => {
    // Only the tab in front is on screen, so the tab is unmounted in between.
    await editing();
    typeIn(await body(), " typed");
    cleanup();

    draw();

    await holds("It lives in **eu-west-1**.\n\n<script>alert(1)</script> typed");
  });

  it("cancels back to the memory as it was, and nothing is written", async () => {
    const { asked } = await editing();
    typeIn(await body(), " typed");

    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(await screen.findByTestId("memory-body")).not.toHaveTextContent("typed");
    expect(asked.some((one) => one.cmd === "memory_edit")).toBe(false);
  });

  it("says the core's refusal and stays in the editor", async () => {
    await editing([memory()], { memory_edit: new Error("empty memory") });
    retype(await body(), "");

    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("empty memory");
    expect(await body()).toBeInTheDocument();
  });
});

describe("a save the file changed under", () => {
  const theirs = memory({ body: "Changed by a chat meanwhile." });

  async function stale(writes: Record<string, unknown> = {}) {
    const asked = core([memory()], {
      memory_edit: (args: Record<string, unknown>) =>
        args.overwrite
          ? { kind: "saved", memory: memory({ body: "mine" }) }
          : { kind: "stale", now: theirs },
      ...writes,
    });
    draw();
    await screen.findByTestId("memory-body");
    act(() => wantEdit(PLANE, KEY));
    retype(await body(), "mine");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    await screen.findByRole("alert");
    return asked;
  }

  it("is refused, says so, and keeps what was typed", async () => {
    await stale();

    expect(screen.getByRole("alert")).toHaveTextContent(/changed on disk/);
    await holds("mine");
    expect(screen.getByRole("button", { name: "Reload" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Overwrite" })).toBeInTheDocument();
  });

  it("shows the memory as it is on disk now beside the edit, which is still typed into (#1862)", async () => {
    await stale();

    // The merge view: what is on disk, read only, beside the edit.
    const disk = await screen.findByRole("textbox", { name: "Body, on disk now" });
    expect(textOf(disk)).toBe("Changed by a chat meanwhile.");
    expect(typeable(disk)).toBe(false);
    expect(
      screen.getByRole("group", { name: "Body, on disk now, beside Body" }),
    ).toBeInTheDocument();
    expect(typeable(await body())).toBe(true);
    typeIn(await body(), " and more");
    await holds("mine and more");
    expect(textOf(disk)).toBe("Changed by a chat meanwhile.");
    // Nothing is saved until the person chooses: Reload or Overwrite.
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("overwrites with the edit as the merge view holds it", async () => {
    const asked = await stale();
    await screen.findByRole("textbox", { name: "Body, on disk now" });
    typeIn(await body(), " and more");

    await userEvent.click(screen.getByRole("button", { name: "Overwrite" }));

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "memory_edit").at(-1)?.args).toMatchObject({
        text: "mine and more",
        overwrite: true,
      }),
    );
  });

  it("reloads to what is on disk now, dropping the edit", async () => {
    await stale();

    await userEvent.click(screen.getByRole("button", { name: "Reload" }));

    expect(await screen.findByTestId("memory-body")).toHaveTextContent("Changed by a chat");
    expect(screen.queryByRole("textbox", { name: "Body" })).toBeNull();
  });

  it("overwrites what is on disk when told to", async () => {
    const asked = await stale();

    await userEvent.click(screen.getByRole("button", { name: "Overwrite" }));

    expect(await screen.findByTestId("memory-body")).toHaveTextContent("mine");
    const edits = asked.filter((one) => one.cmd === "memory_edit");
    expect(edits.map((one) => one.args.overwrite)).toEqual([false, true]);
  });

  it("offers no Overwrite when the memory is not there any more", async () => {
    await stale({ memory_edit: { kind: "stale", now: null } });

    expect(screen.getByRole("alert")).toHaveTextContent(/not there any more/);
    expect(screen.queryByRole("button", { name: "Overwrite" })).toBeNull();
    // Nothing is on disk to show beside the edit, which stays to copy from.
    await holds("mine");
    expect(screen.queryByRole("textbox", { name: "Body, on disk now" })).toBeNull();
  });
});

describe("a new memory's tab", () => {
  const DRAFT_AT: MemoryRef = { scope: { kind: "shared" }, slug: DRAFT };

  it("opens in the editor, reads nothing, and writes through create", async () => {
    const made = memory({ scope: { kind: "shared" }, place: "shared", slug: "freeze" });
    const asked = core([null], { memory_create: made });
    const { onSaved } = draw({ at: DRAFT_AT });

    await userEvent.type(await screen.findByRole("textbox", { name: "Title" }), "Freeze");
    typeIn(await body(), "No deploys on Friday");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(onSaved).toHaveBeenCalledWith(made));
    expect(asked.some((one) => one.cmd === "memory_read")).toBe(false);
    expect(asked.find((one) => one.cmd === "memory_create")?.args).toMatchObject({
      scope: { kind: "shared" },
      title: "Freeze",
      text: "No deploys on Friday",
    });
  });

  it("says under the title what an empty one becomes", async () => {
    core([null]);
    draw({ at: DRAFT_AT });

    expect(await screen.findByRole("textbox", { name: "Title" })).toHaveAccessibleDescription(
      "The body's first line, when left empty · 0 / 72",
    );
  });

  it("closes its tab on Cancel, since there is nothing to go back to", async () => {
    core([null]);
    const { onClose } = draw({ at: DRAFT_AT });

    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));

    expect(onClose).toHaveBeenCalled();
  });
});

describe("moving a memory (KN-3)", () => {
  const SCOPES = [
    { kind: "workspace", name: "alpha" },
    { kind: "persona", name: "steward" },
    { kind: "persona", name: "devops" },
    { kind: "shared" },
  ];
  const MOVED = memory({
    scope: { kind: "shared" },
    place: "shared",
    path: "personas/_shared/memory/where-prod-1-is.md",
  });

  it("offers every other store, and not the one the memory is in", async () => {
    core([memory()], { memory_scopes: SCOPES });
    draw();

    const choice = await screen.findByRole("combobox", { name: "Move to" });
    expect(await offeredIn(choice)).toEqual([
      "Pick a store…",
      "alpha — workspace",
      "devops — persona",
      "shared",
    ]);
    // A move out of a LOCAL journal publishes the memory: the help line says so.
    expect(choice).toHaveAccessibleDescription(/published with the project/);
  });

  it("holds the pick, and moves only on the Move button", async () => {
    const user = userEvent.setup();
    const asked = core([memory()], { memory_scopes: SCOPES, memory_move: MOVED });
    const { onSaved } = draw();
    const choice = await screen.findByRole("combobox", { name: "Move to" });
    const move = screen.getByRole("button", { name: "Move" });
    expect(move).toBeDisabled();

    await pickIn(choice, "shared", user);

    expect(asked.some((one) => one.cmd === "memory_move")).toBe(false);
    expect(move).toBeEnabled();
    await user.click(move);

    await waitFor(() => expect(onSaved).toHaveBeenCalledWith(MOVED));
    const moved = asked.filter((one) => one.cmd === "memory_move");
    expect(moved).toHaveLength(1);
    expect(moved[0].args).toMatchObject({
      plane: PLANE,
      scope: REF.scope,
      slug: REF.slug,
      to: { kind: "shared" },
      restoreAs: null,
    });
  });

  it("says why a move was refused, and stays where it is", async () => {
    const user = userEvent.setup();
    core([memory()], {
      memory_scopes: SCOPES,
      memory_move: new Error("personas/_shared/memory already holds a memory named 'x'"),
    });
    const { onSaved } = draw();
    await pickIn(await screen.findByRole("combobox", { name: "Move to" }), "shared", user);

    await user.click(screen.getByRole("button", { name: "Move" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("already holds");
    expect(onSaved).not.toHaveBeenCalled();
    expect(screen.getByTestId("memory-body")).toBeInTheDocument();
  });
});

describe("a memory's tab on a read-only project (#826)", () => {
  const SENTENCE =
    "this project requires the feature memory-proposals, which purlis 0.8.0 does not have; " +
    "purlis 0.9.0 or later has it. purlis opens the project read-only and writes nothing to it.";
  const SCOPES = [{ kind: "persona", name: "steward" }, { kind: "shared" }];
  const drawReadOnly = () =>
    render(
      <ReadOnlySaid.Provider value={SENTENCE}>
        <MemoryTab plane={PLANE} at={REF} changed={0} onSaved={vi.fn()} onClose={vi.fn()} />
      </ReadOnlySaid.Provider>,
    );

  it("keeps Save off with the project's sentence, and writes nothing on Enter", async () => {
    const asked = core([memory()], { memory_scopes: SCOPES });
    drawReadOnly();
    await screen.findByTestId("memory-body");
    act(() => wantEdit(PLANE, KEY));
    const title = await screen.findByRole("textbox", { name: "Title" });

    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toHaveAttribute("aria-disabled", "true");
    expect(tipOf(save)).toEqual(SENTENCE);
    await userEvent.type(title, " more{Enter}");
    expect(asked.some((one) => one.cmd === "memory_edit")).toBe(false);
    // Cancel still takes the person back to the memory.
    expect(screen.getByRole("button", { name: "Cancel" })).toBeEnabled();
  });

  it("keeps the Move off with the project's sentence", async () => {
    core([memory()], { memory_scopes: SCOPES });
    drawReadOnly();

    const choice = await screen.findByRole("combobox", { name: "Move to" });
    expect(choice).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move" })).toBeDisabled();
    // Why it is off: its group\'s description, which its legend\'s tooltip says too (#2908).
    expect(choice.closest("fieldset")).toHaveAccessibleDescription(SENTENCE);
  });
});
