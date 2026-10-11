import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { forgetThisLaunch } from "./regions";
import { showTheExplorer } from "./test-strips";

/**
 * **"Start a chat here" from a file's own tab, against the whole window** (#1151, lane KA's
 * f44fa3c6): the preview's button reaches the core's `start_chat_here` with the file's branch
 * and path, through the window's wiring (`PlaneView`'s `start`), and a refusal is said where
 * every row's answer is. The button itself, and the lines picked going with it, are
 * `references.test.tsx`'s; this is the wiring between them.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

type Asked = { cmd: string; args: Record<string, unknown> };

/** One workspace with one clone and one branch holding `README.md`; `refuse` is what the core
 *  says to a start, when it refuses it. */
function core({ refuse }: { refuse?: string } = {}) {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return [];
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [] }],
      };
    if (cmd === "workspace_panels")
      return {
        workspace: a.workspace,
        repos: ["svc"],
        paths: { svc: `${ALPHA}/svc` },
        absent: [],
        refused: [],
        todos: [],
        todos_refused: null,
        personas: ["steward"],
        persona: "steward",
      };
    if (cmd === "workspace_repos")
      return { workspace: a.workspace, repos: [], cache_refused: null };
    if (cmd === "worktree_list")
      return [
        {
          piece: "one",
          path: `${ALPHA}/.worktrees/svc/one`,
          branch: "one",
          wired: true,
          stale: false,
        },
      ];
    if (cmd === "branch_tree")
      return a.folder === "" && a.piece === "one"
        ? {
            entries: [{ name: "README.md", kind: "file", ignored: false, refused: null }],
            more: 0,
          }
        : { entries: [], more: 0 };
    if (cmd === "branch_status") return { changes: [], folders: [], more: 0, base: "main" };
    if (cmd === "piece_file") return { kind: "text", text: "# svc\n\nnotes\n" };
    if (cmd === "start_chat_here") {
      if (refuse !== undefined) throw refuse;
      return {
        session: 7,
        name: "7",
        label: "About README.md",
        harness: "claude",
        workspace: "alpha",
        text: "@README.md",
        copied: null,
      };
    }
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    return null;
  });
  return asked;
}

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const starts = (asked: Asked[]) =>
  asked.filter((one) => one.cmd === "start_chat_here").map((one) => one.args);

/** Picks the branch in the Explorer and opens `README.md` in a tab of its own, as a press on its
 *  row does; answers its "Start a chat here". */
async function startFromThePreview() {
  const explorer = await showTheExplorer();
  await userEvent.click(await within(explorer).findByRole("treeitem", { name: /^one in svc/ }));
  const tree = await screen.findByRole("tree", { name: "Files of one in svc" });
  await userEvent.click(await within(tree).findByRole("treeitem", { name: /^README\.md/ }));
  await userEvent.click(
    await screen.findByRole("button", { name: /^Start a chat here on README\.md/ }),
  );
}

describe("Start a chat here, from a file's preview (#1151)", () => {
  it("starts a chat on the file's branch and path, through the core's own start", async () => {
    const asked = core();
    render(<App />);

    await startFromThePreview();

    await waitFor(() =>
      expect(starts(asked)).toEqual([
        expect.objectContaining({
          plane: PLANE,
          workspace: "alpha",
          repo: "svc",
          piece: "one",
          path: "README.md",
          // Nothing picked in the preview: the reference names the file whole.
          lines: null,
        }),
      ]),
    );
    // Its tab opens, named for the file, as the row's start opens one.
    expect((await screen.findAllByText("About README.md")).length).toBeGreaterThan(0);
  });

  it("says the core's refusal where the window says what an action answered", async () => {
    core({ refuse: "purlis could not start a chat in svc: the branch's folder is gone" });
    render(<App />);

    await startFromThePreview();

    expect(
      await screen.findByText("purlis could not start a chat in svc: the branch's folder is gone"),
    ).toBeInTheDocument();
  });
});
