import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Chat as ChatRow, DesktopRow } from "./ipc.ts";
import type { Mirror, Opener } from "./terminal.ts";

const ipc = vi.hoisted(() => {
  const handlers = new Map<string, Set<(payload: unknown) => void>>();
  const resolved = () => vi.fn().mockResolvedValue(null);
  return {
    handlers,
    fire(event: string, payload: unknown) {
      for (const handler of handlers.get(event) ?? []) handler(payload);
    },
    commands: {
      attach: vi.fn().mockResolvedValue({ view: 1, cols: 80, rows: 24 }),
      attachTimeline: vi.fn().mockResolvedValue({ timeline: 1, level: 2 }),
      ack: resolved(),
      detach: resolved(),
      detachTimeline: resolved(),
      write: resolved(),
      prompt: resolved(),
      stop: resolved(),
      answer: resolved(),
    },
  };
});

vi.mock("./ipc", () => ({
  commands: ipc.commands,
  on: (event: string, handler: (payload: unknown) => void) => {
    const set = ipc.handlers.get(event) ?? new Set();
    set.add(handler);
    ipc.handlers.set(event, set);
    return Promise.resolve(() => set.delete(handler));
  },
  said: (err: unknown) => (err instanceof Error ? err.message : String(err)),
}));

import { Chat } from "./Chat.tsx";

const written: Uint8Array[] = [];
const disposed = vi.fn();
const fakeMirror: Mirror = {
  renderer: "dom",
  write(bytes, drawn) {
    written.push(bytes);
    drawn();
  },
  onData() {},
  stdin() {},
  dispose: disposed,
};
const open: Opener = () => fakeMirror;

const connected: DesktopRow = {
  device: "mac",
  name: "Mac",
  addresses: [],
  standing: "connected",
  why: null,
  generation: 1,
};
const away: DesktopRow = { ...connected, standing: "reconnecting" };
const chat: ChatRow = {
  chat: "c1",
  project: "purlis",
  state: "working",
  title: "Fix it",
  level: 2,
};

beforeEach(() => {
  ipc.handlers.clear();
  written.length = 0;
  disposed.mockClear();
  for (const command of Object.values(ipc.commands)) command.mockClear();
});

describe("Chat", () => {
  it("disables its controls and attaches nothing while the desktop is away", () => {
    render(<Chat desktop={away} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Enter" })).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent("Reconnecting…");
    expect(ipc.commands.attach).not.toHaveBeenCalled();
  });

  it("sends the composer's text as a prompt", async () => {
    render(<Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    await userEvent.type(screen.getByLabelText("Message"), "run the tests");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(ipc.commands.prompt).toHaveBeenCalledWith("mac", "c1", "run the tests");
    await waitFor(() => expect(screen.getByLabelText("Message")).toHaveValue(""));
  });

  it("sends a quick key as raw bytes", async () => {
    render(<Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    await userEvent.click(screen.getByRole("button", { name: "Enter" }));
    expect(ipc.commands.write).toHaveBeenCalledWith("mac", "c1", [0x0d]);
  });

  it("stops the chat only after End is confirmed", async () => {
    render(<Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(ipc.commands.stop).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "End" }));
    expect(ipc.commands.stop).toHaveBeenCalledWith("mac", "c1");
  });

  it("draws the view's chunks and acknowledges each once it is drawn", async () => {
    render(<Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalledWith("mac", "c1"));
    await act(async () => {
      ipc.fire("view", {
        device: "mac",
        chat: "c1",
        view: 1,
        kind: "snapshot",
        b64: btoa("hello"),
      });
    });
    await waitFor(() => expect(ipc.commands.ack).toHaveBeenCalledWith("mac", "c1", 1, 5));
    expect(ipc.commands.ack).toHaveBeenCalledTimes(1);
    expect(written).toHaveLength(1);
  });

  it("attaches again, afresh, when the desktop comes back on a new link", async () => {
    const { rerender } = render(
      <Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />,
    );
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalledTimes(1));
    rerender(<Chat desktop={away} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    await waitFor(() => expect(ipc.commands.detach).toHaveBeenCalledTimes(1));
    ipc.commands.attach.mockResolvedValueOnce({ view: 2, cols: 80, rows: 24 });
    rerender(
      <Chat
        desktop={{ ...connected, generation: 2 }}
        chat={chat}
        onBack={vi.fn()}
        onRetry={vi.fn()}
        open={open}
      />,
    );
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(disposed).toHaveBeenCalled());
    await act(async () => {
      ipc.fire("view", { device: "mac", chat: "c1", view: 1, kind: "live", b64: btoa("old") });
      ipc.fire("view", { device: "mac", chat: "c1", view: 2, kind: "snapshot", b64: btoa("new") });
    });
    await waitFor(() => expect(ipc.commands.ack).toHaveBeenCalledWith("mac", "c1", 2, 3));
    expect(ipc.commands.ack).toHaveBeenCalledTimes(1);
  });

  it("attaches again on its own when a link that was never seen to drop was replaced", async () => {
    const { rerender } = render(
      <Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />,
    );
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalledTimes(1));
    rerender(
      <Chat
        desktop={{ ...connected, generation: 2 }}
        chat={chat}
        onBack={vi.fn()}
        onRetry={vi.fn()}
        open={open}
      />,
    );
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalledTimes(2));
    expect(ipc.commands.detach).toHaveBeenCalledTimes(1);
  });

  it("offers Reload when the desktop closes the view at once", async () => {
    render(<Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalledTimes(1));
    await act(async () => {
      ipc.fire("view-ended", { device: "mac", chat: "c1", number: 1 });
    });
    expect(await screen.findByText("The desktop closed this view.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Reload" }));
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalledTimes(2));
  });

  it("ignores a chunk of another chat or another view", async () => {
    render(<Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalled());
    await act(async () => {
      ipc.fire("view", { device: "mac", chat: "c2", view: 1, kind: "live", b64: btoa("x") });
      ipc.fire("view", { device: "mac", chat: "c1", view: 9, kind: "live", b64: btoa("x") });
    });
    expect(written).toHaveLength(0);
    expect(ipc.commands.ack).not.toHaveBeenCalled();
  });

  it("shows the timeline's story and its open ask", async () => {
    render(<Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />);
    await waitFor(() => expect(ipc.commands.attachTimeline).toHaveBeenCalledWith("mac", "c1"));
    await act(async () => {
      ipc.fire("timeline", {
        device: "mac",
        chat: "c1",
        timeline: 1,
        item: { level: 2, item: "message", turn: "t", role: "agent", text: "Done.", done: true },
      });
      ipc.fire("timeline", {
        device: "mac",
        chat: "c1",
        timeline: 1,
        item: {
          level: 2,
          item: "ask",
          ask: "a1",
          summary: "Delete it?",
          choices: [{ id: "y", label: "Yes", lasts: false }],
          elicits_secret: false,
        },
      });
    });
    expect(screen.getByRole("log")).toHaveTextContent("Agent: Done.");
    await userEvent.click(screen.getByRole("button", { name: "Yes" }));
    expect(ipc.commands.answer).toHaveBeenCalledWith("mac", "c1", "a1", "y");
  });

  it("draws a chat with no level as a shell tab, as an older host lists it", () => {
    render(
      <Chat
        desktop={connected}
        chat={{ ...chat, level: undefined }}
        onBack={vi.fn()}
        onRetry={vi.fn()}
        open={open}
      />,
    );
    expect(screen.queryByLabelText("Message")).toBeNull();
  });

  it("gives a shell tab the mirror and the keys only", () => {
    render(
      <Chat
        desktop={connected}
        chat={{ ...chat, level: 1 }}
        onBack={vi.fn()}
        onRetry={vi.fn()}
        open={open}
      />,
    );
    expect(screen.queryByLabelText("Message")).toBeNull();
    expect(screen.queryByRole("log")).toBeNull();
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
    expect(screen.getByRole("button", { name: "Ctrl-C" })).toBeInTheDocument();
    expect(ipc.commands.attachTimeline).not.toHaveBeenCalled();
  });

  it("detaches when it goes away", async () => {
    const { unmount } = render(
      <Chat desktop={connected} chat={chat} onBack={vi.fn()} onRetry={vi.fn()} open={open} />,
    );
    await waitFor(() => expect(ipc.commands.attach).toHaveBeenCalled());
    unmount();
    // The detach is queued behind the attach, so it lands a tick later.
    await waitFor(() => expect(ipc.commands.detach).toHaveBeenCalledWith("mac", "c1"));
    await waitFor(() => expect(ipc.commands.detachTimeline).toHaveBeenCalledWith("mac", "c1"));
  });
});
