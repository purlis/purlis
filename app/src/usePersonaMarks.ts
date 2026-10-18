// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type PersonaMark as Mark, type PlaneId } from "./bindings";

// The one read of the marks, apart from `PersonaMark.tsx`, which draws them: a Notice draws a
// mark without importing what asks the core for them (the theme specimen, #2330, imports a
// Notice).

/**
 * The marks of the plane's personas, read at once and again whenever `changed` moves (a
 * project view passes its count of changes on disk, so an `icon.png` a chat writes is drawn
 * without a restart). `reload` reads them again after a pick here.
 *
 * A read that fails leaves what was drawn: a mark is decoration, and every persona still has
 * its initials.
 */
export function usePersonaMarks(
  plane: PlaneId,
  changed: number,
): { marks: ReadonlyMap<string, Mark>; reload: () => void } {
  const [marks, setMarks] = useState<ReadonlyMap<string, Mark>>(() => new Map());
  const [asked, setAsked] = useState(0);
  useEffect(() => {
    let gone = false;
    void commands
      .personaMarks(plane)
      .then((said) => {
        if (gone || said.status !== "ok" || !Array.isArray(said.data)) return;
        setMarks(new Map(said.data.map((one) => [one.name, one])));
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane, changed, asked]);
  const reload = useCallback(() => setAsked((was) => was + 1), []);
  return { marks, reload };
}
