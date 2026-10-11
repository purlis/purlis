import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { handReferences, picksSaid } from "./referencesMany";
import type { Referenced } from "./references";

/**
 * **Several rows into a chat as one paste** (#1151): the explorer's picked rows, handed to the
 * core's `references_into_chat` once, which types them as one bracketed paste and never sends.
 */

afterEach(clearMocks);

const AT = { plane: "/p", workspace: "alpha", repo: "svc", piece: "fix-it" } as const;
const A: Referenced = { ...AT, path: "src/main.rs", folder: false };
const B: Referenced = { ...AT, path: "docs", folder: true };

describe("several references into a chat", () => {
  it("asks the core once, with the branch and every path in order, and says its answer", async () => {
    const asked: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      return { kind: "typed", text: "@src/main.rs @docs/" };
    });

    const ran = await handReferences("/p", 3, "steward 3", [A, B]);

    expect(asked).toEqual([
      {
        cmd: "references_into_chat",
        args: {
          plane: "/p",
          workspace: "alpha",
          repo: "svc",
          piece: "fix-it",
          paths: ["src/main.rs", "docs"],
          session: 3,
        },
      },
    ]);
    expect(ran).toEqual({
      ok: true,
      said: "Typed @src/main.rs @docs/ into steward 3. Nothing was sent.",
    });
  });

  it("refuses rows of two branches, or of another project, without asking the core", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
    });

    expect(await handReferences("/p", 3, "steward 3", [A, { ...B, piece: "other" }])).toEqual({
      ok: false,
      refused: "The picked rows are in more than one branch, so nothing was handed to steward 3.",
    });
    expect(await handReferences("/p", 3, "steward 3", [A, { ...B, plane: "/q" }])).toEqual({
      ok: false,
      refused: "The picked rows are in another project, so nothing was handed to steward 3.",
    });
    expect(asked).toEqual([]);
  });

  it("says the core's refusal as it said it", async () => {
    mockIPC(() => {
      throw "more than 64 references at once";
    });

    expect(await handReferences("/p", 3, "steward 3", [A, B])).toEqual({
      ok: false,
      refused: "more than 64 references at once",
    });
  });

  it("names one row by its path, and several by how many", () => {
    expect(picksSaid([A])).toBe("src/main.rs");
    expect(picksSaid([A, B])).toBe("2 picked rows");
  });
});
