// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { HardDrive } from "lucide-react";
import { commands, type PlaneId } from "./bindings";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { PUBLISHED, PublishProject } from "./PublishProject";
import { useReadOnlySaid } from "./ReadOnlyLine";
import { Button } from "./ui/button/Button";

/** What the line says: plainly where the project is, and what that costs. */
export const LIVES_HERE =
  "This project lives only on this machine. Nothing else has a copy, and nobody else can open it.";

/**
 * **Whether `plane` lives only on this machine** (#1820), and the repo name Publish suggests
 * when it does; `undefined` when it is somewhere else too, or until the core has answered.
 *
 * Asked at the mount, again when the project's own files change, and again when a publish of
 * it lands ({@link PUBLISHED}). A window that cannot ask says nothing: the line is an offer,
 * and the core checks again before it publishes anything.
 */
export function useLivesHere(plane: PlaneId): string | undefined {
  const changes = usePlaneChanged([plane], SETTINGS);
  const [published, setPublished] = useState(0);
  // Kept with the project it is about, so another project's tab never shows it for a moment.
  const [read, setRead] = useState<{ plane: PlaneId; suggested: string | undefined }>();
  useEffect(() => {
    const landed = (event: Event) => {
      if ((event as CustomEvent<string>).detail === plane) setPublished((n) => n + 1);
    };
    window.addEventListener(PUBLISHED, landed);
    return () => window.removeEventListener(PUBLISHED, landed);
  }, [plane]);
  useEffect(() => {
    let gone = false;
    void Promise.resolve(commands.projectLivesHere(plane))
      .then((answer) => {
        if (gone || answer?.status !== "ok") return;
        setRead({ plane, suggested: answer.data ?? undefined });
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane, changes, published]);
  return read?.plane === plane ? read.suggested : undefined;
}

/**
 * **A project that lives only on this machine says so plainly** (#1820, ruling Q9): one line
 * with Publish to GitHub or GitLab… beside it, which opens {@link PublishProject}. Drawn on the
 * project's tab (`as="line"`, above its workspaces, as the read-only line is) and in its
 * settings (`as="words"`, inside the level's sentence). Once the project is published the line
 * is gone; nothing is drawn for a project that has a remote.
 *
 * On a project this purlis opens read-only, Publish is off with that sentence as its reason: a
 * publish commits what is not committed.
 */
export function LivesHere({ plane, as = "line" }: { plane: PlaneId; as?: "line" | "words" }) {
  const suggested = useLivesHere(plane);
  const readOnly = useReadOnlySaid();
  const [publishing, setPublishing] = useState<string>();
  const dialog = publishing !== undefined && (
    <PublishProject plane={plane} suggested={publishing} onClose={() => setPublishing(undefined)} />
  );
  const publish = suggested !== undefined && (
    <Button
      isDisabled={readOnly !== undefined}
      tooltip={readOnly}
      onPress={() => setPublishing(suggested)}
    >
      Publish to GitHub or GitLab…
    </Button>
  );
  // The dialog stays where it is drawn whether or not the line is: once a publish lands the line
  // goes, and the dialog, which says where it went, must not be drawn afresh.
  const line =
    suggested === undefined ? null : as === "words" ? (
      <>
        {" "}
        <span role="note" aria-label="Only on this machine" className="lives-here-words">
          {LIVES_HERE}
        </span>{" "}
        {publish}
      </>
    ) : (
      <p className="read-only-line lives-here-line" role="note" aria-label="Only on this machine">
        <HardDrive aria-hidden="true" />
        <span>{LIVES_HERE}</span> {publish}
      </p>
    );
  return (
    <>
      {line}
      {dialog}
    </>
  );
}
