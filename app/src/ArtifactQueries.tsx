// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type ArtifactOpened } from "./bindings";
import { Button } from "./ui/button/Button";

/**
 * **What an artifact's page reads of purlis, said in purlis's bar** (ADR 0120 §7, #5969).
 *
 * - **The queries its page declared**, as its approval covers them: each `<id>@<N>`, and every
 *   declared parameter that widens what it reads, with what it then reads.
 * - **A deprecated or replaced query** still answers, and the bar warns of it.
 * - **Plane-wide reads are a grant of their own**: a page that declares a plane-scoped query
 *   reads only its workspace until the person allows them here, on top of approving its script.
 */
export function ArtifactQueries({ opened }: { opened: ArtifactOpened }) {
  const [failed, setFailed] = useState<string | null>(null);
  if (opened.queries.length === 0) return null;
  const plane = opened.queries.filter((query) => query.scope === "plane");
  const grant = () => {
    setFailed(null);
    commands.artifactGrantPlane(opened.label).then(
      (answer) => {
        if (answer.status === "error") setFailed(answer.error);
      },
      (why: unknown) => setFailed(String(why)),
    );
  };
  return (
    <>
      <span className="artifact-bar-standing" data-testid="artifact-queries">
        {readsSaid(opened)}
      </span>
      {opened.queries.flatMap((query) =>
        query.warning === null
          ? []
          : [
              <span
                key={query.id}
                className="artifact-bar-standing"
                data-testid="artifact-query-warning"
              >
                {query.warning}
              </span>,
            ],
      )}
      {plane.length === 0 ? null : (
        <span className="artifact-bar-standing" data-testid="artifact-plane">
          {failed ??
            (opened.plane_granted
              ? "Project-wide reads allowed on this machine"
              : `Reads only this workspace until you allow project-wide reads for ${plane.map((query) => query.id).join(", ")}`)}
        </span>
      )}
      {plane.length > 0 && opened.standing === "approved" && !opened.plane_granted ? (
        <Button variant="grants" size="sm" onPress={grant}>
          Allow project-wide reads
        </Button>
      ) : null}
    </>
  );
}

/** The bar's sentence of what the page reads: each query, and what widens it. */
export function readsSaid(opened: ArtifactOpened): string {
  const each = opened.queries.map((query) =>
    query.widens.length === 0 ? query.id : `${query.id} (${query.widens.join("; ")})`,
  );
  return `Reads ${each.join(", ")}`;
}
