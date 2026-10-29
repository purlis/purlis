// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PoolAdding, PoolLimitsShown } from "./bindings";
import { Field, SettingRow } from "./settings/components";
import { addPool, doneWithTheAddedPool, stopAddingAPool, usePools } from "./pools";
import { PoolProfiles } from "./settings/PoolProfiles";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **Add a Docker pool…** (#2679, spec #2041: a Docker pool in one step): its name, the runner
 * image by digest, its limits, and how long its runner is left without a link before it
 * suspends itself (#3471), each left empty for purlis's default, which its box shows.
 * Add pool makes the pool's runner from the image on this machine's Docker, confined and held
 * to those limits, and pairs it with this machine, which takes a moment; a refusal stays in the
 * dialog, in the core's words, with what was typed.
 *
 * **Adding a pool ends with signing this machine's profiles in there** (#2049): once the pool
 * is added, the dialog lists each profile as its runner keeps it, with Sign in for each not
 * signed in there, from a shell tab in the project `plane`'s window. Done closes it.
 *
 * Drawn wherever the window is, while the palette's row or the Pools group asked for it
 * ({@link AddDockerPoolHost}).
 */
export function AddDockerPoolHost({ plane }: { plane?: string }) {
  const pools = usePools();
  if (pools.added !== undefined && !pools.adding) {
    return <SignInOnTheAddedPool pool={pools.added} plane={plane} />;
  }
  if (!pools.adding) return null;
  const defaults = pools.read.state === "read" ? pools.read.shown.defaults : undefined;
  const suspendAfter = pools.read.state === "read" ? pools.read.shown.suspend_after : undefined;
  const linkFrames = pools.read.state === "read" ? pools.read.shown : undefined;
  return (
    <AddDockerPool
      defaults={defaults}
      suspendAfter={suspendAfter}
      linkFrames={{
        frames: linkFrames?.frames ?? undefined,
        width: linkFrames?.width ?? undefined,
      }}
    />
  );
}

/** What adding pool `pool` ends with: this machine's profiles, to sign in there (#2049). */
function SignInOnTheAddedPool({ pool, plane }: { pool: string; plane: string | undefined }) {
  return (
    <Dialog
      onClose={doneWithTheAddedPool}
      title={`Sign your profiles in on pool ${pool}`}
      subtitle="each once, through its harness's own sign-in on the runner; the runner keeps it, and nothing of this machine's sign-in goes there"
      main={{ children: "Done", onPress: doneWithTheAddedPool }}
    >
      <PoolProfiles pool={pool} plane={plane} />
    </Dialog>
  );
}

/** A count typed, or `null` for the default; `undefined` when it is not a count. */
function count(typed: string): number | null | undefined {
  const trimmed = typed.trim();
  if (trimmed === "") return null;
  return /^\d+$/.test(trimmed) ? Number(trimmed) : undefined;
}

function AddDockerPool({
  defaults,
  suspendAfter,
  linkFrames,
}: {
  defaults: PoolLimitsShown | undefined;
  /** How long a new pool's runner is left without a link before it suspends, by default. */
  suspendAfter: string | undefined;
  /** How a new pool's runner's browsers are watched over the link, by default (#3297). */
  linkFrames: { frames: number | undefined; width: number | undefined };
}) {
  const [name, setName] = useState("");
  const [image, setImage] = useState("");
  const [cpus, setCpus] = useState("");
  const [memory, setMemory] = useState("");
  const [processes, setProcesses] = useState("");
  const [tmp, setTmp] = useState("");
  const [after, setAfter] = useState("");
  const [frames, setFrames] = useState("");
  const [width, setWidth] = useState("");
  const [trouble, setTrouble] = useState<string | undefined>();
  const [adding, setAdding] = useState(false);

  const add = async () => {
    if (adding) return;
    const [memoryMib, processesCount, tmpMib] = [count(memory), count(processes), count(tmp)];
    const [framesCount, widthPixels] = [count(frames), count(width)];
    if (
      memoryMib === undefined ||
      processesCount === undefined ||
      tmpMib === undefined ||
      framesCount === undefined ||
      widthPixels === undefined
    ) {
      setTrouble("Nothing was added: memory, processes, /tmp, frames and width are whole numbers.");
      return;
    }
    const asked: PoolAdding = {
      name,
      image,
      cpus: cpus.trim() === "" ? null : cpus,
      memory_mib: memoryMib,
      processes: processesCount,
      tmp_mib: tmpMib,
      suspend_after: after.trim() === "" ? null : after.trim(),
      frames: framesCount,
      width: widthPixels,
    };
    setTrouble(undefined);
    setAdding(true);
    const refused = await addPool(asked);
    setAdding(false);
    if (refused !== undefined) setTrouble(refused);
  };

  const limit = (
    label: string,
    help: string,
    value: string,
    onChange: (to: string) => void,
    shown: string | undefined,
  ) => (
    <SettingRow
      label={label}
      help={help}
      control={(ids) => (
        <Field
          kind="text"
          ids={ids}
          inputMode="numeric"
          size={8}
          value={value}
          placeholder={shown}
          disabled={adding}
          onChange={onChange}
        />
      )}
    />
  );

  // **Placed as every dialog is** (#2910): it had no place of its own and sat wherever the
  // window drew it.
  return (
    <Dialog
      onClose={() => {
        // Not while the runner is being made: the answer would land where nobody is looking.
        if (!adding) stopAddingAPool();
      }}
      title="Add a Docker pool"
      subtitle="on this machine; its runner is a container of the image, run on your Docker"
      cancel={{ isDisabled: adding, onPress: stopAddingAPool }}
      main={{
        children: adding ? "Making its runner…" : "Add pool",
        reserve: ["Add pool", "Making its runner…"],
        isDisabled: adding,
        onPress: () => void add(),
      }}
    >
      <SettingRow
        label="Name"
        help="Lower-case letters, digits and dashes. Its runner is called by it too."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={name}
            placeholder="builds"
            maxLength={40}
            disabled={adding}
            onChange={setName}
          />
        )}
      />
      <SettingRow
        label="Image"
        help="The runner image, named by its digest, so it cannot move under the pool."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={image}
            placeholder="ghcr.io/purlis/purlis-runner@sha256:…"
            disabled={adding}
            onChange={setImage}
          />
        )}
      />
      {limit(
        "CPUs",
        "How many CPUs its runner may use, to the thousandth. Empty is the default.",
        cpus,
        setCpus,
        defaults?.cpus,
      )}
      {limit(
        "Memory in MiB",
        "The memory its runner may use, with no swap beyond it. Empty is the default.",
        memory,
        setMemory,
        defaults === undefined ? undefined : String(defaults.memory_mib),
      )}
      {limit(
        "Processes",
        "How many processes its runner may run at once. Empty is the default.",
        processes,
        setProcesses,
        defaults === undefined ? undefined : String(defaults.processes),
      )}
      {limit(
        "/tmp in MiB",
        "The size of its runner's /tmp, which counts against its memory. Empty is the default.",
        tmp,
        setTmp,
        defaults === undefined ? undefined : String(defaults.tmp_mib),
      )}
      <SettingRow
        label="Suspends after"
        help="How long its runner is left without a link from this machine before it suspends itself, keeping its home: a whole number of h, m or s, such as 12h. Empty is the default."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            size={8}
            value={after}
            placeholder={suspendAfter}
            disabled={adding}
            onChange={setAfter}
          />
        )}
      />
      {limit(
        "Browser frames a second",
        "How many frames a second its runner's browsers are watched at over the link. Empty is the default.",
        frames,
        setFrames,
        linkFrames.frames === undefined ? undefined : String(linkFrames.frames),
      )}
      {limit(
        "Browser width in pixels",
        "The widest frame its runner's browsers are watched at over the link. Empty is the default.",
        width,
        setWidth,
        linkFrames.width === undefined ? undefined : String(linkFrames.width),
      )}

      {trouble !== undefined && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}
