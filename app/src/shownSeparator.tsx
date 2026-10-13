// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ComponentProps } from "react";
import { Separator as ResizableSeparator } from "react-resizable-panels";
import { useProjectShown } from "./projectShown";

/**
 * `react-resizable-panels`' `Separator`, **out of the pointer's way while its project is not in
 * front** (`projectShown.tsx`).
 *
 * The library listens on the document and, for every press and move, asks each group it holds
 * whether the pointer is on one of its handles, by where the handle is and by what is drawn over
 * it. A handle in an element that is in no document is at no place at all, which it reads as
 * the window's corner, and has no ancestor in common with what was pressed, which it throws
 * for. A disabled handle is one it does not ask about.
 *
 * **The handle, and not its `Group`.** A handle's `disabled` is a mark the library changes in
 * place, and nothing is laid out for it. A group's registers the group again, which lays the
 * group out again from what it remembers (`RegionFrame.tsx` says what registering again costs
 * in this library).
 */
export function Separator(props: ComponentProps<typeof ResizableSeparator>) {
  const shown = useProjectShown();
  return <ResizableSeparator {...props} disabled={props.disabled === true || !shown} />;
}
