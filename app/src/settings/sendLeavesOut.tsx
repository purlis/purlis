// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type PlaneId } from "../bindings";
import { Field } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/** The entries a list field's text names: one per line, blank lines dropped. */
const entriesOf = (text: string): string[] =>
  text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");

/**
 * **What a workspace's sends to a pool's runner leave out** (#2046, the operator's ruling of
 * 2026-10-25). A send never carries what a save to the forge refuses (credential files, a token
 * or a private key in a file's text, hard links), and Changes names what each send left out.
 * This is the person's own list on top: files and folders in each of the workspace's clones,
 * kept in its `workspace.json`. Offered only once this machine keeps a pool, as Runs on is.
 */
export function sendLeavesOutGroup(
  plane: PlaneId,
  workspace: string,
  poolsKept: boolean,
): SettingsGroup | undefined {
  if (!poolsKept) return undefined;
  return {
    id: "workspace.sends",
    label: "Sent to runners",
    help: "What a pool's runner gets of this workspace's clones. A send leaves out whatever a save to the forge refuses to push, and Changes names it.",
    kept: "Kept in the project's settings file, which your team sees.",
    settings: [leaveOutSetting(plane, workspace)],
  };
}

function leaveOutSetting(plane: PlaneId, workspace: string): LiveSetting {
  return {
    id: "workspace.sends.leave-out",
    label: "Also leave out",
    help: "One file or folder per line, from the top of each clone, such as secrets/ or config/local.yml. Uncommitted changes to them stay on this machine.",
    // A committed file: absent, the row is off on a project purlis opens read-only.
    useControl: function useLeaveOut() {
      const [text, setText] = useState<string>();
      const [error, setError] = useState<string[]>();
      const saved = useRef<string[]>([]);
      useEffect(() => {
        let gone = false;
        void commands
          .sendLeavesOut(plane, workspace)
          .then((said) => {
            if (gone) return;
            if (said.status === "ok") {
              saved.current = said.data;
              setText(said.data.join("\n"));
            } else setError([said.error]);
          })
          .catch((err: unknown) => !gone && setError([String(err)]));
        return () => {
          gone = true;
        };
      }, []);
      const commit = useCallback(() => {
        if (text === undefined) return;
        const entries = entriesOf(text);
        if (entries.join("\n") === saved.current.join("\n")) return;
        setError(undefined);
        void commands
          .setSendLeavesOut(plane, workspace, entries)
          .then((said) => {
            if (said.status === "ok") {
              saved.current = said.data;
              setText(said.data.join("\n"));
            } else setError([said.error]);
          })
          .catch((err: unknown) => setError([String(err)]));
      }, [text]);
      return {
        error,
        control: (ids) => (
          <Field kind="list" ids={ids} value={text ?? ""} onChange={setText} onCommit={commit} />
        ),
      };
    },
  };
}
