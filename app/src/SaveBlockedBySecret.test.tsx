// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SaveBlockedBySecret, SHOWN_FINDINGS, whereAndWhy } from "./SaveBlockedBySecret";
import { SETTINGS_LINK } from "./settings/links";
import { forgetYourEditor, setYourEditor } from "./yourEditor";
import type { SecretAt } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetYourEditor();
});

const PLANE = "/home/dev/plane";

const NOTE: SecretAt = {
  path: "personas/devops/memory/the-client-is-now-tofu.md",
  line: 5,
  kind: "credential assignment",
  shown: "token = ********",
};

type Asked = { cmd: string; args: Record<string, unknown> };

/** The core: `open_blocked_file` answers `opened`, or throws it when it is an `Error`. */
function core(opened: null | Error = null): Asked[] {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "open_blocked_file" && opened instanceof Error) throw opened.message;
    return null;
  });
  return asked;
}

describe("SaveBlockedBySecret", () => {
  it("names the file, the line, the kind and what it matched, never a value", () => {
    core();
    render(
      <SaveBlockedBySecret plane={PLANE} secrets={[NOTE]} busy={false} onSaveAgain={vi.fn()} />,
    );

    const notice = screen.getByRole("status", {
      name: "The save is blocked in personas/devops/memory/the-client-is-now-tofu.md",
    });
    expect(notice).toHaveTextContent(
      "personas/devops/memory/the-client-is-now-tofu.md, line 5: credential assignment (token = ********).",
    );
    expect(notice).toHaveTextContent("If it is not, reword the line. Then save again.");
    expect(within(notice).getByRole("button", { name: "Open the file" })).toBeInTheDocument();
    expect(within(notice).getByRole("button", { name: "Save again" })).toBeInTheDocument();
  });

  it("says a finding with no line was read through the file's escapes", () => {
    expect(whereAndWhy({ path: "a.json", line: null, kind: "JWT", shown: null })).toBe(
      "a.json, read through its escapes: JWT.",
    );
  });

  it("asks the core to open the file it named, in your editor", async () => {
    const asked = core();
    setYourEditor("zed");
    render(
      <SaveBlockedBySecret plane={PLANE} secrets={[NOTE]} busy={false} onSaveAgain={vi.fn()} />,
    );

    await userEvent.click(screen.getByRole("button", { name: "Open the file" }));

    expect(asked.filter((one) => one.cmd === "open_blocked_file")).toEqual([
      {
        cmd: "open_blocked_file",
        args: { plane: PLANE, path: NOTE.path, editor: "zed" },
      },
    ]);
  });

  it("asks for an editor where none is chosen, with the link to where one is", async () => {
    const asked = core();
    const links: unknown[] = [];
    const heard = (event: Event) => links.push((event as CustomEvent).detail);
    window.addEventListener(SETTINGS_LINK, heard);
    render(
      <SaveBlockedBySecret plane={PLANE} secrets={[NOTE]} busy={false} onSaveAgain={vi.fn()} />,
    );

    await userEvent.click(screen.getByRole("button", { name: "Open the file" }));

    expect(asked.filter((one) => one.cmd === "open_blocked_file")).toEqual([]);
    expect(screen.getByRole("status")).toHaveTextContent("Choose your editor in Settings first.");
    await userEvent.click(screen.getByRole("button", { name: "Choose your editor" }));
    window.removeEventListener(SETTINGS_LINK, heard);
    expect(links).toHaveLength(1);
  });

  it("says the core's refusal beside the file it refused", async () => {
    core(new Error("purlis opens only a file the save is blocked in"));
    setYourEditor("zed");
    render(
      <SaveBlockedBySecret plane={PLANE} secrets={[NOTE]} busy={false} onSaveAgain={vi.fn()} />,
    );

    await userEvent.click(screen.getByRole("button", { name: "Open the file" }));

    expect(await screen.findByText(/purlis opens only a file the save is blocked in/)).toBeTruthy();
  });

  it("saves again on the press, and not while a save runs", async () => {
    core();
    const again = vi.fn();
    const { rerender } = render(
      <SaveBlockedBySecret plane={PLANE} secrets={[NOTE]} busy={false} onSaveAgain={again} />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Save again" }));
    expect(again).toHaveBeenCalledTimes(1);

    rerender(<SaveBlockedBySecret plane={PLANE} secrets={[NOTE]} busy onSaveAgain={again} />);
    const busy = screen.getByRole("button", { name: "Save again" });
    expect(busy).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(busy);
    expect(again).toHaveBeenCalledTimes(1);
  });

  it("draws a Notice for the first few findings and counts the rest", () => {
    core();
    const many: SecretAt[] = Array.from({ length: SHOWN_FINDINGS + 2 }, (_, n) => ({
      ...NOTE,
      path: `notes/${n}.md`,
    }));
    render(<SaveBlockedBySecret plane={PLANE} secrets={many} busy={false} onSaveAgain={vi.fn()} />);

    expect(screen.getAllByRole("status")).toHaveLength(SHOWN_FINDINGS);
    expect(screen.getByText(/And 2 more files the save is blocked in/)).toBeTruthy();
  });
});
