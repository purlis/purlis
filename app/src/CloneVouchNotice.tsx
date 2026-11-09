// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useCallback, useEffect, useState } from "react";
import { commands, type CloneToVouch, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { GIT, usePlaneChanged } from "./planeChanged";
import { useWindowFocused } from "./windowFocused";

/** The family of a clone-to-trust's cause, whose dismissal is kept until the clone or its
 *  remote changes. */
export const CLONE_VOUCH = "clone-vouch";

/**
 * What a clone-to-trust's Notice is keyed by: its folder and the address (or addresses) its
 * remote names. A dismissal hides that clone at that remote: once its remote changes, it shows
 * again, naming the new one.
 */
export const cloneVouchCause = (clone: CloneToVouch) =>
  `${CLONE_VOUCH}:${clone.folder}|${clone.place ?? clone.places.join(",")}`;

/** `GitHub` for a github.com address, else the address's host. */
const accessOn = (place: string) => {
  const host = place.split("/", 1)[0];
  return host === "github.com" ? "GitHub" : host;
};

/**
 * **A clone made outside purlis** (one Notice per clone): a clone in one of the project's
 * workspaces that purlis did not clone and the person has not trusted, with the address its
 * remote names now. *Trust this clone* records it at exactly that address (`vouch_clone`), so
 * its saves push with the person's sign-in and chats in its workspace can use their access with
 * it; the core refuses, and the Notice says why, when the remote changed since it was shown. A
 * clone whose remote names more than one address has no Trust: the Notice says to give origin
 * one address first. Dismiss hides it until the clone's remote changes.
 */
export function CloneVouchNotice({
  clone,
  busy,
  refused,
  onTrust,
  onDismiss,
}: {
  clone: CloneToVouch;
  busy: boolean;
  /** What the core said to the last Trust, where it refused. */
  refused?: string;
  /** Trusts the clone at `place`, the address this Notice shows. */
  onTrust: (place: string) => void;
  onDismiss: () => void;
}) {
  const cause = cloneVouchCause(clone);
  const tone = refused === undefined ? "attention" : "trouble";
  const place = clone.place;
  if (place === null) {
    return (
      <Notice cause={cause} tone={tone} label="Clone made outside purlis" onDismiss={onDismiss}>
        <p>
          <code>{clone.folder}</code> was not cloned by purlis, and its remote names more than one
          address: <Places places={clone.places} />. purlis can trust it once origin has one
          address.
        </p>
      </Notice>
    );
  }
  return (
    <Notice
      cause={cause}
      tone={tone}
      label="Clone made outside purlis"
      fixes={[{ label: "Trust this clone", onPress: () => onTrust(place), busy }]}
      onDismiss={onDismiss}
    >
      <p>
        <code>{clone.folder}</code> was not cloned by purlis. Its remote is <code>{place}</code>. If
        you made this clone yourself, trust it so chats in this workspace can use your{" "}
        {accessOn(place)} access with it.
        {refused !== undefined && ` ${refused.endsWith(".") ? refused : `${refused}.`}`}
      </p>
    </Notice>
  );
}

/**
 * **The project's clones made outside purlis**: read from the core when the project opens, when
 * the window comes back into focus and when the project's git standing moves, so a Notice goes
 * once its clone is trusted, here or by any other way, and comes back when its remote changes.
 */
export function CloneVouchNotices({
  plane,
  dismissed,
  dismiss,
  settle,
}: {
  plane: PlaneId;
  /** The causes dismissed in this project, and the ways to change them (`useDismissals`). */
  dismissed: ReadonlySet<string>;
  dismiss: (cause: string) => void;
  settle: (family: string, present: readonly string[]) => void;
}) {
  const [clones, setClones] = useState<readonly CloneToVouch[]>([]);
  /** What the last Trust said, by the folder it was pressed on, where it was refused. */
  const [refused, setRefused] = useState<Readonly<Record<string, string>>>({});
  const [busy, setBusy] = useState<string>();
  const [asked, setAsked] = useState(0);
  const focused = useWindowFocused();
  const moved = usePlaneChanged([plane], GIT);

  useEffect(() => {
    let live = true;
    void commands
      .clonesToVouch(plane)
      .then((answer) => {
        if (live && answer.status === "ok" && Array.isArray(answer.data)) {
          setClones(answer.data);
          // A dismissal stands while its clone and remote do, and a change shows again.
          settle(CLONE_VOUCH, answer.data.map(cloneVouchCause));
        }
      })
      // A project whose clones cannot be read says nothing here.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, focused, moved, asked, settle]);

  const trust = useCallback(
    (clone: CloneToVouch, place: string) => {
      if (busy !== undefined) return;
      setBusy(clone.folder);
      void commands
        .vouchClone(plane, clone.folder, place)
        .then((answer) => {
          setRefused((was) => {
            const rest = Object.fromEntries(
              Object.entries(was).filter(([folder]) => folder !== clone.folder),
            );
            return answer.status === "ok" ? rest : { ...rest, [clone.folder]: answer.error };
          });
        })
        .catch((err: unknown) => setRefused((was) => ({ ...was, [clone.folder]: String(err) })))
        .finally(() => {
          setBusy(undefined);
          setAsked((n) => n + 1);
        });
    },
    [plane, busy],
  );

  return (
    <>
      {clones
        .filter((clone) => !dismissed.has(cloneVouchCause(clone)))
        .map((clone) => (
          <CloneVouchNotice
            key={cloneVouchCause(clone)}
            clone={clone}
            busy={busy === clone.folder}
            refused={refused[clone.folder]}
            onTrust={(place) => trust(clone, place)}
            onDismiss={() => dismiss(cloneVouchCause(clone))}
          />
        ))}
    </>
  );
}

/** `places` as code, in a sentence: `a`, `a and b`, `a, b and c`. */
function Places({ places }: { places: readonly string[] }) {
  return places.map((place, at) => (
    <Fragment key={place}>
      {at === 0 ? "" : at === places.length - 1 ? " and " : ", "}
      <code>{place}</code>
    </Fragment>
  ));
}
