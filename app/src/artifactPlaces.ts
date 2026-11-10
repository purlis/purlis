// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Does, Offer } from "./actions";
import { todoView } from "./todos";

/**
 * **Where the window goes for a `purlis://` link in an artifact's page** (ADR 0122 §5 as
 * amended, #5967; `docs/artifact-links.md`).
 *
 * The core reads the link into a place of a closed list and checks it is in the artifact's
 * project (`artifact_places.rs`); the window hears only one of these. Each is a row the window
 * runs as its palette would, and every one only navigates ({@link NAVIGATIONS}): a link never
 * acts.
 */
export type Goes =
  | { to: "workspace"; workspace: string }
  | { to: "todo"; workspace: string; todo: string; title: string }
  | { to: "chat"; session: number };

/** **The only verbs a link runs**: each goes somewhere, and none changes anything. */
export const NAVIGATIONS: readonly Does["verb"][] = ["focusWorkspace", "openView", "showChat"];

/** **The row the window runs for `goes`**, or one that does nothing for a place it does not know. */
export function placeOffer(goes: Goes): Offer {
  const row = (id: string, title: string, does: Does): Offer => ({
    id,
    title,
    available: true,
    reason: "",
    does,
  });
  switch (goes.to) {
    case "workspace":
      return row(`workspace.focus:${goes.workspace}`, `Focus workspace ${goes.workspace}`, {
        verb: "focusWorkspace",
        workspace: goes.workspace,
      });
    case "todo":
      return row(`todo.open:${goes.todo}`, `Open todo: ${goes.title}`, {
        verb: "openView",
        view: todoView({ workspace: goes.workspace, slug: goes.todo }),
        title: goes.title,
      });
    case "chat":
      return row(`chat.show:${goes.session}`, "Show the chat", {
        verb: "showChat",
        session: goes.session,
      });
    default:
      return {
        id: "link.nowhere",
        title: "Go nowhere",
        available: false,
        reason: "purlis does not know that place",
        does: { verb: "nothing" },
      };
  }
}
