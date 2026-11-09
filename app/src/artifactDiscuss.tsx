// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import { MessageSquare } from "lucide-react";
import { commands, type PlaneId, type StartedHere } from "./bindings";
import type { Said } from "./actions";
import { startRefused, type StartApproving } from "./StartApproval";
import { Button } from "./ui/button/Button";

/**
 * **Discuss**, in an artifact's bar (ADR 0122 §5, #5957): a new chat about the page.
 *
 * The core quotes the page as third-party content, names its pinner and commit, and starts a
 * chat in the workspace with one line typed and never sent (`artifact_discuss`); the person
 * reads it, adds their question and presses Enter. The bar asks the project's window, which
 * places the chat's tab as it places every chat the core starts (`PlaneView`, with
 * {@link useArtifactDiscuss}).
 */
export const ARTIFACT_DISCUSS = "purlis-artifact-discuss";

/** What an artifact's bar asked of its project's window. */
export type ArtifactDiscussAsk = {
  plane: PlaneId;
  workspace: string;
  folder: string;
};

/** Asks the window of project `ask.plane` to open a chat about the artifact. */
export function askArtifactDiscuss(ask: ArtifactDiscussAsk): void {
  window.dispatchEvent(new CustomEvent<ArtifactDiscussAsk>(ARTIFACT_DISCUSS, { detail: ask }));
}

/** The bar's Discuss button. */
export function DiscussButton(ask: ArtifactDiscussAsk) {
  return (
    <Button
      size="sm"
      icon={<MessageSquare aria-hidden="true" />}
      onPress={() => askArtifactDiscuss(ask)}
    >
      Discuss
    </Button>
  );
}

/** The chat sizes a chat starts at before its pane says its own (`PlaneView`'s). */
const STARTING_SIZE = { columns: 80, rows: 24 };

/**
 * **The project window's half of Discuss**: opens the chat the bar asked for, and places its
 * tab (`placed`). A refusal is the core's sentence, with Approve and start… where its profile
 * needs approving; a chat whose harness purlis cannot type into opens all the same, and the
 * window says the line is on the clipboard.
 */
export function useArtifactDiscuss(
  plane: PlaneId,
  placed: (chat: StartedHere & { persona: null }) => void,
  said: (said: Said) => void,
  approving: (asked: StartApproving) => void,
): void {
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<ArtifactDiscussAsk>).detail;
      if (wanted.plane !== plane) return;
      void commands
        .artifactDiscuss(
          plane,
          wanted.workspace,
          wanted.folder,
          STARTING_SIZE.columns,
          STARTING_SIZE.rows,
        )
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
        .then((started) => {
          if (started.status === "error") {
            const refused = startRefused(
              started.error,
              {
                named: "Discuss",
                typed: "its line about the page",
                again: () => askArtifactDiscuss(wanted),
              },
              approving,
            );
            said({ from: "artifact.discuss", refused: true, ...refused });
            return;
          }
          const chat = started.data;
          placed({ ...chat, persona: null });
          if (chat.copied !== null) {
            const why = chat.copied.charAt(0).toUpperCase() + chat.copied.slice(1);
            said({ from: "artifact.discuss", refused: false, words: `${why}.` });
          }
        });
    };
    window.addEventListener(ARTIFACT_DISCUSS, asked);
    return () => window.removeEventListener(ARTIFACT_DISCUSS, asked);
  }, [plane, placed, said, approving]);
}
