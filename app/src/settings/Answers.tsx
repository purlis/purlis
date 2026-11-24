// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type AnswersShown, type PlaneId } from "../bindings";
import { Button } from "../ui/button/Button";
import type { RowIds } from "./components";
import type { Setting } from "./groups";

/** The address of the setting that shows the sealed answers, in the Sandbox group (M75-07a). */
export const SEALED_ANSWERS = "project.sandbox.sealedAnswers";

/** What the row says of each state of the answers. */
export function stateSaid(shown: AnswersShown): string {
  switch (shown.state) {
    case "holds":
      return "Your answers for this project are sealed, and every one holds its seal.";
    case "fresh":
      return "Nothing is sealed yet for this project, so the shipped default applies.";
    case "no-keyring":
      return "This machine has no keyring purlis can use, so it can't seal your answers. The shipped default applies, and nothing you answer is kept.";
    case "unverifiable":
      return "Only purlis's app can check the sealed answers, and this isn't the app.";
    default:
      return `Your answers for this project are in the deny state, so nothing they allow holds: ${shown.deny.join("; ")}.`;
  }
}

/** What the row says of the device id: a label to read, never a name to change. */
export function deviceSaid(shown: AnswersShown): string {
  const { state, id } = shown.device;
  if (state === "confirmed") return `This machine's id is ${id ?? ""}. Rules can name it.`;
  if (state === "unconfirmed") {
    return `This machine's id is ${id ?? ""}, and it isn't confirmed yet, so no rule can name this machine.`;
  }
  return "This machine has no device id yet. It is sealed in the keyring when purlis first opens a project, and without a working keyring it stays unset.";
}

/** What the row says of the Windows Hello opt-in; nothing where this machine has none. */
export function helloSaid(shown: AnswersShown): string | undefined {
  const { state, gap } = shown.hello;
  if (state === "unavailable") return undefined;
  const lead =
    state === "on"
      ? "Windows Hello is confirmed each time purlis starts. Without it, the seal key stays locked."
      : "Windows Hello isn't asked for when purlis starts.";
  return gap === null ? lead : `${lead} ${gap[0]?.toUpperCase() ?? ""}${gap.slice(1)}`;
}

/**
 * **The sealed answers and the device id** (M75-07a, #7015; spec §3.2, §8.10): where this
 * project's answers stand, and this machine's id as a label, with **Confirm** while it is
 * waiting. The id has no rename. Confirming is the person's press, and names the id shown here,
 * so an id that changed since is never confirmed unseen. On Windows it also offers **Windows
 * Hello at each start** (M75-43, #7052), off by default, saying the gap it leaves: narrowed with
 * it on, never closed. Turning it off is a loosening, so it is the person's press and Windows
 * Hello asks again.
 */
export function AnswersView({
  shown,
  busy,
  refused,
  onConfirm,
  onHello,
  ids,
}: {
  shown: AnswersShown;
  busy: boolean;
  /** What the core said to the last Confirm, where it refused. */
  refused?: string;
  onConfirm: (id: string) => void;
  /** The person's press on the Windows Hello button: turn it `on` or off. */
  onHello: (on: boolean) => void;
  ids: RowIds;
}) {
  const id = shown.device.id;
  const hello = helloSaid(shown);
  return (
    <div id={ids.id} aria-labelledby={ids.labelledBy}>
      <p>{stateSaid(shown)}</p>
      {shown.notices.map((line) => (
        <p key={line}>{line}</p>
      ))}
      <p>{deviceSaid(shown)}</p>
      {shown.device.state === "unconfirmed" && id !== null && (
        <Button variant="secondary" isDisabled={busy} onPress={() => onConfirm(id)}>
          Confirm device id
        </Button>
      )}
      {hello !== undefined && <p>{hello}</p>}
      {shown.hello.state !== "unavailable" && (
        <Button
          variant="secondary"
          isDisabled={busy}
          onPress={() => onHello(shown.hello.state !== "on")}
        >
          {shown.hello.state === "on" ? "Turn off Windows Hello…" : "Turn on Windows Hello…"}
        </Button>
      )}
      {refused !== undefined && <p>{refused}</p>}
    </div>
  );
}

/** The Sandbox group's row for the sealed answers: read from the core when it opens. */
export function Answers({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const [shown, setShown] = useState<AnswersShown>();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  useEffect(() => {
    let gone = false;
    void commands
      .answersShown(plane)
      .then((done) => {
        if (gone) return;
        if (done.status === "error") setSaid(done.error);
        else if (done.data?.device !== undefined) setShown(done.data);
      })
      .catch((err: unknown) => {
        if (!gone) setSaid(`purlis could not read the sealed answers: ${String(err)}`);
      });
    return () => {
      gone = true;
    };
  }, [plane]);
  const confirm = useCallback(
    (id: string) => {
      setBusy(true);
      setRefused(undefined);
      void commands
        .confirmDeviceId(plane, id)
        .then((done) => {
          if (done.status === "error") setRefused(done.error);
          else setShown((was) => (was === undefined ? was : { ...was, device: done.data }));
        })
        .catch((err: unknown) => setRefused(`purlis could not confirm the id: ${String(err)}`))
        .finally(() => setBusy(false));
    },
    [plane],
  );
  const hello = useCallback((on: boolean) => {
    setBusy(true);
    setRefused(undefined);
    void commands
      .setWindowsHello(on)
      .then((done) => {
        if (done.status === "error") setRefused(done.error);
        else setShown((was) => (was === undefined ? was : { ...was, hello: done.data }));
      })
      .catch((err: unknown) => setRefused(`purlis could not change Windows Hello: ${String(err)}`))
      .finally(() => setBusy(false));
  }, []);
  if (shown === undefined) return <p id={ids.id}>{said ?? "Reading the sealed answers…"}</p>;
  return (
    <AnswersView
      shown={shown}
      busy={busy}
      refused={refused}
      onConfirm={confirm}
      onHello={hello}
      ids={ids}
    />
  );
}

/** The Sandbox group's row for them. Nothing here writes a file: the answers are the core's. */
export function answersSetting(plane: PlaneId): Setting {
  return {
    id: SEALED_ANSWERS,
    label: "Sealed answers and device id",
    writes: "nothing",
    help: "Where the answers you gave on this machine stand, and this machine's id, which rules can name once you confirm it. The id is a label: it has no rename.",
    useControl: function useAnswers() {
      return { control: (ids) => <Answers plane={plane} ids={ids} /> };
    },
  };
}
