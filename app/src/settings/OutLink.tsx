// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";
import type { PlaneId } from "../bindings";
import { askSettingsAction } from "./links";
import { Button } from "../ui/button/Button";

/**
 * **A link out of Settings, drawn** (#1387, #1388): a button, the shared one (DS-9), that
 * asks the project's window to run the catalogue's row `action` (`links.ts`) — open a vault's or
 * a persona's tab, the vault picker, the Extensions dialog.
 */
export function OutLink({
  plane,
  action,
  label,
  children,
}: {
  plane: PlaneId;
  action: string;
  /** Its accessible name, where a page names its buttons beyond their words (the dispatch
   *  table names each by what it shows, then whose row it is on). */
  label?: string;
  children: ReactNode;
}) {
  return (
    <Button aria-label={label} onPress={() => askSettingsAction(plane, action)}>
      {children}
    </Button>
  );
}
