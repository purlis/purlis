// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Artifacts in the palette's search** (ADR 0122, spec #5948, #5968): the pinned artifacts of
 * the project in front whose pages say what was typed, as the core finds them in the project's
 * search index (`artifact_search.rs`). What is read, and what never is, is the core's: a pinned
 * folder only, through its gate, its template's words and never anyone's data. This asks and
 * keeps the newest answer.
 */
import { useEffect, useRef, useState } from "react";
import { commands, type ArtifactFound, type PlaneId } from "./bindings";

/**
 * The artifacts of project `plane` that say `query`, best first; none while nothing is typed,
 * there is no project to ask, or the answer to what is typed now has not come. An answer to an
 * older query is never shown, and a refusal finds nothing.
 */
export function useArtifactFind(plane: PlaneId | undefined, query: string): ArtifactFound[] {
  const [found, setFound] = useState<{ asked: string; artifacts: ArtifactFound[] }>({
    asked: "",
    artifacts: [],
  });
  const newest = useRef(0);
  const words = query.trim();
  const asked = `${plane ?? ""}\n${words}`;
  useEffect(() => {
    if (plane === undefined || words === "") return;
    const ask = ++newest.current;
    void commands
      .artifactSearch(plane, words)
      .then((said) => {
        if (ask !== newest.current) return;
        // Only a list is an answer: anything else finds nothing.
        const artifacts = said.status === "ok" && Array.isArray(said.data) ? said.data : [];
        setFound({ asked, artifacts });
      })
      .catch(() => {
        if (ask === newest.current) setFound({ asked, artifacts: [] });
      });
  }, [plane, words, asked]);
  return plane === undefined || words === "" || found.asked !== asked ? [] : found.artifacts;
}
