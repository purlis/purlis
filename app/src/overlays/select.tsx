// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import * as Radix from "@radix-ui/react-select";
import { holdingOnlyWhileShown, onlyWhileShown } from "../projectShown";
import { inTheTopLayer } from "./topLayer";

/**
 * Radix's `select`, with what it puts over the window **drawn only while its project is in
 * front** (`projectShown.tsx`, `onlyWhileShown`, and `holdingOnlyWhileShown` for its portal).
 * Every part is Radix's own; the ones below the list are Radix's, asked first whether the
 * project they are under is shown. Outside any project, in the window's own palette and
 * dialogs, that is always.
 *
 * **Each part by its short name, and no other.** The package also exports every part under a
 * long one (`DialogContent` beside `Content`), which is the same part with nothing asked of
 * it: handed on whole, a part written by its long name would be drawn over the window by a
 * project behind. So the parts are listed, and a long name is not here to be written.
 *
 * The window's overlays are written through this file and never from the package:
 * `projectShown.guard.test.tsx` fails for one that is not.
 */
export {
  Root,
  Trigger,
  Value,
  Icon,
  Viewport,
  Group,
  Label,
  Item,
  ItemText,
  ItemIndicator,
  ScrollUpButton,
  ScrollDownButton,
  Separator,
  Arrow,
} from "@radix-ui/react-select";

export const Portal = holdingOnlyWhileShown(inTheTopLayer(Radix.Portal));
export const Content = onlyWhileShown(Radix.Content);
