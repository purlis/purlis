// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useLayoutEffect, useState } from "react";
import type { Size } from "./paneDrop";

/**
 * **The room an element gives the panes**, as it is resized (#4766): nothing until it has been
 * laid out with some width and height (jsdom never lays out), so nothing folds before then.
 */
export function usePaneRoom(): [(element: HTMLElement | null) => void, Size | undefined] {
  const [element, setElement] = useState<HTMLElement | null>(null);
  const [room, setRoom] = useState<Size | undefined>(undefined);
  useLayoutEffect(() => {
    if (element === null) return;
    const measure = () => {
      const box = element.getBoundingClientRect();
      const next =
        box.width > 0 && box.height > 0
          ? { width: Math.round(box.width), height: Math.round(box.height) }
          : undefined;
      setRoom((was) => (was?.width === next?.width && was?.height === next?.height ? was : next));
    };
    measure();
    const watching = new ResizeObserver(measure);
    watching.observe(element);
    return () => watching.disconnect();
  }, [element]);
  const at = useCallback((one: HTMLElement | null) => setElement(one), []);
  return [at, room];
}
