// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { ClusterPoolKey, PoolAdding, PoolLimitsShown } from "./bindings";
import { Choice, Field, SettingRow } from "./settings/components";
import {
  addClusterPool,
  addPool,
  checkClusterPoolKey,
  choosePoolKind,
  doneWithTheAddedPool,
  readClusterContexts,
  readReleaseImage,
  stopAddingAPool,
  usePools,
  type PoolKind,
  type ReleaseImage,
} from "./pools";
import { PoolProfiles } from "./settings/PoolProfiles";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **Add a pool…** (#2679, spec #2041: a Docker pool in one step): its Kind first, then its name,
 * the runner image by digest, its limits, and how long its runner is left without a link before
 * it suspends itself (#3471), each left empty for purlis's default, which its box shows. The
 * image's default is the runner image this version's release names (the person's ruling of
 * 2026-10-30, {@link readReleaseImage}); a build that names none (a development build, or a
 * release whose runner image was not built) says so under the box, which then needs a digest.
 * Add pool makes the pool's runner from the image on this machine's Docker, confined and held
 * to those limits, and pairs it with this machine, which takes a moment; a refusal stays in the
 * dialog, in the core's words, with what was typed.
 *
 * **Adding a pool ends with signing this machine's profiles in there** (#2049): once the pool
 * is added, the dialog lists each profile as its runner keeps it, with Sign in for each not
 * signed in there, from a shell tab in the project `plane`'s window. Done closes it.
 *
 * **A cluster pool is added from the same dialog** (#3292): its Kind, first, is Docker or
 * Cluster ({@link KindRow}). A cluster pool takes a name, a kubeconfig (kubectl's own default
 * when empty), one of its contexts and the cluster's Pool; **Check its key** reads the cluster
 * operator key that Pool names and shows it whole, to check against the one the cluster's admin
 * gives; **Add pool** then pins that key and claims, pairs and keeps its runner. Changing what
 * names the cluster takes the key away, so only a key checked for what is added is pinned.
 *
 * Drawn wherever the window is, while the palette's row or the Pools group asked for it
 * ({@link AddDockerPoolHost}).
 */
export function AddDockerPoolHost({ plane }: { plane?: string }) {
  const pools = usePools();
  // What was typed is kept while the dialog is open, across a change of Kind, and no longer.
  useEffect(() => {
    if (!pools.adding) drafts.clear();
  }, [pools.adding]);
  // The image this version's release names, asked each time the dialog opens.
  const [releaseImage, setReleaseImage] = useState<ReleaseImage | undefined>();
  useEffect(() => {
    if (!pools.adding) return;
    let gone = false;
    void readReleaseImage().then((read) => {
      if (!gone) setReleaseImage(read);
    });
    return () => {
      gone = true;
      setReleaseImage(undefined);
    };
  }, [pools.adding]);
  if (pools.added !== undefined && !pools.adding) {
    return <SignInOnTheAddedPool pool={pools.added} plane={plane} />;
  }
  if (!pools.adding) return null;
  if (pools.kind === "cluster") return <AddClusterPool />;
  const defaults = pools.read.state === "read" ? pools.read.shown.defaults : undefined;
  const suspendAfter = pools.read.state === "read" ? pools.read.shown.suspend_after : undefined;
  const linkFrames = pools.read.state === "read" ? pools.read.shown : undefined;
  return (
    <AddDockerPool
      releaseImage={releaseImage}
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

/** The dialog's first row: which kind of pool it adds (#3292). */
function KindRow({ kind, disabled }: { kind: PoolKind; disabled: boolean }) {
  return (
    <SettingRow
      label="Kind"
      help="Where its runner runs."
      grouped
      control={(ids) => (
        <Choice
          kind="radio"
          ids={ids}
          value={kind}
          disabled={disabled}
          onValueChange={(to) => choosePoolKind(to === "cluster" ? "cluster" : "docker")}
          options={[
            { value: "docker", label: "Docker", says: "A container on this machine's Docker." },
            {
              value: "cluster",
              label: "Cluster",
              says: "A runner in your team's cluster, claimed through your kubectl.",
            },
          ]}
        />
      )}
    />
  );
}

/** What is typed in each box of the dialog, by kind and box, while it is open (#3292): kept
 *  when the Kind is changed, so the other form shows what was typed in it. */
const drafts = new Map<string, string>();

/** A box's text, kept in {@link drafts}. */
function useDrafted(kind: PoolKind, box: string): [string, (to: string) => void] {
  const id = `${kind}.${box}`;
  const [value, setValue] = useState(() => drafts.get(id) ?? "");
  return [
    value,
    (to: string) => {
      drafts.set(id, to);
      setValue(to);
    },
  ];
}

/** A count typed, or `null` for the default; `undefined` when it is not a count. */
function count(typed: string): number | null | undefined {
  const trimmed = typed.trim();
  if (trimmed === "") return null;
  return /^\d+$/.test(trimmed) ? Number(trimmed) : undefined;
}

/** What the Image box says under it, by what purlis said of this version's runner image. */
function imageHelp(read: ReleaseImage | undefined): string {
  const named = "The runner image, named by its digest, so it cannot move under the pool.";
  if (read === undefined) return named;
  if ("trouble" in read) {
    return `${named} purlis could not read which runner image this version names, so name one by its digest: ${read.trouble}`;
  }
  return read.image === null
    ? `${named} This build of purlis names no runner image: a development build, or a release whose runner image was not built. Name one by its digest.`
    : `${named} Empty is the default: purlis's runner image of this version.`;
}

function AddDockerPool({
  releaseImage,
  defaults,
  suspendAfter,
  linkFrames,
}: {
  /** The runner image this version's release names, once purlis said (the person's ruling of
   *  2026-10-30). */
  releaseImage: ReleaseImage | undefined;
  defaults: PoolLimitsShown | undefined;
  /** How long a new pool's runner is left without a link before it suspends, by default. */
  suspendAfter: string | undefined;
  /** How a new pool's runner's browsers are watched over the link, by default (#3297). */
  linkFrames: { frames: number | undefined; width: number | undefined };
}) {
  const [name, setName] = useDrafted("docker", "name");
  const [image, setImage] = useDrafted("docker", "image");
  const [cpus, setCpus] = useDrafted("docker", "cpus");
  const [memory, setMemory] = useDrafted("docker", "memory");
  const [processes, setProcesses] = useDrafted("docker", "processes");
  const [tmp, setTmp] = useDrafted("docker", "tmp");
  const [after, setAfter] = useDrafted("docker", "after");
  const [frames, setFrames] = useDrafted("docker", "frames");
  const [width, setWidth] = useDrafted("docker", "width");
  const [trouble, setTrouble] = useState<string | undefined>();
  const [adding, setAdding] = useState(false);
  const named =
    releaseImage !== undefined && "image" in releaseImage
      ? (releaseImage.image ?? undefined)
      : undefined;

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
      image: image.trim() === "" ? (named ?? image) : image,
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
      title="Add a pool"
      subtitle="on this machine; its runner is a container of the image, run on your Docker"
      cancel={{ isDisabled: adding, onPress: stopAddingAPool }}
      main={{
        children: adding ? "Making its runner…" : "Add pool",
        reserve: ["Add pool", "Making its runner…"],
        isDisabled: adding,
        onPress: () => void add(),
      }}
    >
      <KindRow kind="docker" disabled={adding} />
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
        help={imageHelp(releaseImage)}
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={image}
            placeholder={named ?? "ghcr.io/purlis/purlis-runner@sha256:…"}
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

/**
 * **Add a pool, of Kind Cluster** (#3292): Kind, Name, Kubeconfig, Context and Pool, then Check
 * its key, which shows the cluster operator key the Pool names, then Add pool, which pins it.
 */
function AddClusterPool() {
  const [name, setName] = useDrafted("cluster", "name");
  const [kubeconfig, setKubeconfig] = useDrafted("cluster", "kubeconfig");
  // The kubeconfig whose contexts are offered: the one typed, a moment after it stops changing.
  const [readFrom, setReadFrom] = useState(() => kubeconfig.trim());
  const [contexts, setContexts] = useState<string[]>([]);
  // Why the contexts could not be read, said under its box rather than as an alert.
  const [unread, setUnread] = useState<string | undefined>();
  const [defaultKubeconfig, setDefaultKubeconfig] = useState<string | undefined>();
  const [context, setContext] = useState<string | undefined>();
  const [pool, setPool] = useDrafted("cluster", "pool");
  const [checked, setChecked] = useState<ClusterPoolKey | undefined>();
  const [trouble, setTrouble] = useState<string | undefined>();
  const [busy, setBusy] = useState<"checking" | "adding" | undefined>();

  useEffect(() => {
    const typed = kubeconfig.trim();
    if (typed === readFrom) return;
    const later = setTimeout(() => setReadFrom(typed), 400);
    return () => clearTimeout(later);
  }, [kubeconfig, readFrom]);

  useEffect(() => {
    let gone = false;
    void readClusterContexts(readFrom).then((read) => {
      if (gone) return;
      if ("trouble" in read) {
        setContexts([]);
        setUnread(read.trouble);
        return;
      }
      setUnread(undefined);
      if (readFrom === "") setDefaultKubeconfig(read.kubeconfig);
      setContexts(read.contexts);
      setContext((was) =>
        was !== undefined && read.contexts.includes(was) ? was : (read.current ?? undefined),
      );
    });
    return () => {
      gone = true;
    };
  }, [readFrom]);

  // What names the cluster changed: the key checked was for something else.
  const changing =
    <T,>(to: (value: T) => void) =>
    (value: T) => {
      setChecked(undefined);
      to(value);
    };

  const check = async () => {
    if (busy !== undefined) return;
    if (context === undefined) {
      setTrouble("Nothing was checked: choose a context of the kubeconfig.");
      return;
    }
    if (pool.trim() === "") {
      setTrouble("Nothing was checked: name the cluster's pool, as your cluster's admin gives it.");
      return;
    }
    setTrouble(undefined);
    setBusy("checking");
    const read = await checkClusterPoolKey(kubeconfig, context, pool);
    setBusy(undefined);
    if ("trouble" in read) setTrouble(read.trouble);
    else setChecked(read);
  };

  const add = async () => {
    if (busy !== undefined || checked === undefined || context === undefined) return;
    setTrouble(undefined);
    setBusy("adding");
    const refused = await addClusterPool({
      name,
      kubeconfig: kubeconfig.trim() === "" ? null : kubeconfig,
      context,
      pool,
      key: checked.key,
    });
    setBusy(undefined);
    if (refused !== undefined) setTrouble(refused);
  };

  const main =
    checked === undefined
      ? busy === "checking"
        ? "Reading its key…"
        : "Check its key"
      : busy === "adding"
        ? "Claiming its runner…"
        : "Add pool";

  return (
    <Dialog
      onClose={() => {
        // Not while its runner is being claimed: the answer would land where nobody is looking.
        if (busy === undefined) stopAddingAPool();
      }}
      title="Add a pool"
      subtitle="in your team's cluster; its runner is claimed from a pool of purlis's cluster operator, through your kubectl"
      cancel={{ isDisabled: busy !== undefined, onPress: stopAddingAPool }}
      main={{
        children: main,
        reserve: ["Check its key", "Reading its key…", "Add pool", "Claiming its runner…"],
        isDisabled: busy !== undefined,
        onPress: () => void (checked === undefined ? check() : add()),
      }}
    >
      <KindRow kind="cluster" disabled={busy !== undefined} />
      <SettingRow
        label="Name"
        help="Lower-case letters, digits and dashes. Its runner is called by it too."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={name}
            placeholder="team"
            maxLength={40}
            disabled={busy !== undefined}
            onChange={setName}
          />
        )}
      />
      <SettingRow
        label="Kubeconfig"
        help="The kubeconfig your kubectl signs in to the cluster with. Empty is kubectl's own. The pool keeps its own copy of the context, taken when it is added; to pick up a changed kubeconfig, add the pool again."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={kubeconfig}
            placeholder={defaultKubeconfig}
            disabled={busy !== undefined}
            onChange={changing(setKubeconfig)}
          />
        )}
      />
      <SettingRow
        label="Context"
        help={
          unread === undefined
            ? "The kubeconfig's context for the cluster. Its namespace is where the runner is claimed."
            : `No context to choose: ${unread}`
        }
        control={(ids) => (
          <Choice
            kind="select"
            ids={ids}
            value={context}
            unset="Choose a context"
            disabled={busy !== undefined || contexts.length === 0}
            onValueChange={changing((to: string) => setContext(to === "" ? undefined : to))}
            options={contexts.map((one) => ({ value: one, label: one }))}
          />
        )}
      />
      <SettingRow
        label="Pool"
        help="The name of the cluster's pool, as your cluster's admin gives it."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={pool}
            disabled={busy !== undefined}
            onChange={changing(setPool)}
          />
        )}
      />
      {checked !== undefined && (
        <SettingRow
          label="Cluster operator key"
          help={`Check it against the key your cluster's admin gives you before you add the pool. Its runner is claimed in namespace ${checked.namespace}.`}
          control={(ids) => (
            <output id={ids.id} aria-describedby={ids.describedBy} className="ui-machine-path">
              <code>{checked.key}</code>
            </output>
          )}
        />
      )}

      {trouble !== undefined && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}
