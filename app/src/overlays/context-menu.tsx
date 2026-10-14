// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import * as Radix from "@radix-ui/react-context-menu";
import { holdingOnlyWhileShown, onlyWhileShown } from "../projectShown";

/**
 * Radix's `context-menu`, with what it puts over the window **drawn only while its project is in
 * front** (`projectShown.tsx`, `onlyWhileShown`, and `holdingOnlyWhileShown` for its portal). Every part is Radix's own; the ones below are
 * Radix's, asked first whether the project they are under is shown. Outside any project, in the
 * window's own palette and dialogs, that is always.
 *
 * The window's overlays are written through this file and never from the package:
 * `projectShown.guard.test.tsx` fails for one that is not.
 */
export * from "@radix-ui/react-context-menu";

export const Portal = holdingOnlyWhileShown(Radix.Portal);
export const Content = onlyWhileShown(Radix.Content);
