// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type PlaneId, type ProjectHarnesses } from "../bindings";
import { NO_EDITOR, yourEditor } from "../yourEditor";
import type { LiveSetting } from "./groups";
import { Button } from "../ui/button/Button";

/**
 * **The project's harness declarations, read-only, each with Open file** (#1389, spec #1221's
 * collection shapes): each `harnesses/<name>.toml` the project declares, and each one purlis
 * would not read with its reason. Editing one is its file's, in your editor; nothing here
 * writes it. What a declaration says is a committed file's words, drawn as text.
 */
export function harnessDeclarationRows(
  plane: PlaneId,
  declared: ProjectHarnesses | undefined,
): LiveSetting[] {
  if (declared === undefined) return [];
  return [
    ...declared.declared.map((one) => ({
      id: `project.harness.declared.${one.name}`,
      label: `${one.title} (${one.name})`,
      help:
        `A harness this project declares in ${one.file}: it starts ${one.program}` +
        `${one.acp ? ", and can run as an ACP agent" : ""}. Read-only here: edit the file to change it.`,
      // Open file hands the path to your editor: purlis writes nothing, so it stays on in a
      // read-only project (#1795).
      writes: "nothing" as const,
      useControl: opening(plane, one.file),
    })),
    ...declared.refused.map((one) => ({
      id: `project.harness.refused.${one.file}`,
      label: one.file,
      help: `purlis did not read this harness declaration: ${one.reason}`,
      writes: "nothing" as const,
      // The folder itself is no file to open.
      useControl: one.file.endsWith("/") ? nothingToOpen : opening(plane, one.file),
    })),
  ];
}

/** A row with no Open file: the reason is its help. */
function nothingToOpen() {
  return { control: () => null };
}

/** A row's Open file: the declaration in your editor, or why not, said beside it. */
function opening(plane: PlaneId, file: string): LiveSetting["useControl"] {
  return function useOpenFile() {
    const [said, setSaid] = useState<string>();
    const open = () => {
      const editor = yourEditor();
      if (editor === undefined) {
        setSaid(NO_EDITOR);
        return;
      }
      void commands
        .openHarnessDeclaration(plane, file, editor)
        .then((done) => setSaid(done.status === "error" ? done.error : undefined))
        .catch((err: unknown) => setSaid(String(err)));
    };
    return {
      grouped: true,
      error: said === undefined ? undefined : [said],
      control: (ids) => (
        <div
          id={ids.id}
          role="group"
          className="ui-setting-status"
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
        >
          <Button onPress={open}>Open file</Button>
        </div>
      ),
    };
  };
}
