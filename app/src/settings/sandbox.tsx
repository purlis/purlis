// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import {
  commands,
  type PersonaHosts,
  type PlaneId,
  type SandboxPolicy,
  type SandboxPreset,
  type SandboxState,
  type SettingsEdit,
  type SettingsFile,
  type YourLevel,
} from "../bindings";
import {
  allowLabel,
  allowPersonaHosts,
  cautionsSaid,
  FROM_NEXT_START,
  mayAllowPersonaHosts,
  whatItLets,
  whatWaits,
} from "../personaHostsAllow";
import {
  ALLOW_EXPECTS,
  allowable,
  asDrawn,
  ExpectedLines,
  mayAnswerExpects,
  useProjectExpects,
} from "../ProjectExpectsNotice";
import { sandboxCommandReturned } from "../sandboxAsked";
import { key, onOffAt, textAt, valueAt, type Control, type Shown } from "./fileControls";
import type { RowIds } from "./components";
import type { Collection, LiveSetting } from "./groups";
import { showPersona } from "./links";
import { OutLink } from "./OutLink";

/**
 * **Settings › Project › Sandbox explains what a chat here can do** (#1340, spec #1330): one
 * sentence at the top ({@link whatAChatCanDo}), the mode as a status line, Internet access as
 * the core's presets — each opening to the hosts the core lists for it — the certificate check,
 * and what chats can change and what is always protected, each with why.
 *
 * **Every preset and host is the core's** (`sandbox_state`'s `presets`, from
 * `purlis_core::sandbox::Preset` and `hosts`): nothing here lists a host. Which presets are on is
 * read from the committed file as it stands, so the sentence follows a box as it is ticked.
 *
 * **An administrator's policy has the last word** (#1343, ADR 0067 §1): each value it locks is
 * shown as "Locked by policy", with who set it ({@link SandboxPolicy}'s `locked_by`), and offers
 * no control. The presets in force are the project's within the policy's.
 *
 * **A policy that forbids the opt-out requires the sandbox** (#1423, ADR 0067 §4 as amended): the
 * mode then reads "On, required by policy" with who set it (`required`), in a project that has
 * not turned it on too, and offers no control.
 */

/** `[sandbox] mode`, which this page turns on and never back off (D-SE17g). */
export const SANDBOX_MODE = key("sandbox", "mode");

/** `[sandbox] egress`: the presets turned on. */
const EGRESS = key("sandbox", "egress");

/** `[sandbox] certificate-checks` (#1337): on unless the file says `false` (R1, 2026-10-13). */
const CERTIFICATE_CHECKS = key("sandbox", "certificate-checks");

/**
 * The presets in force as `shared` stands: those `egress` names, in the core's order; every
 * preset where the file names none, as the core reads a missing `egress` (ADR 0067 §3).
 */
export function presetsOn(
  shared: Shown,
  table: readonly SandboxPreset[],
  policy: SandboxPolicy | null = null,
): SandboxPreset[] {
  const value = valueAt(shared, EGRESS);
  const asked =
    value?.kind !== "list"
      ? [...table]
      : table.filter((preset) => value.value.includes(preset.word));
  // The strictest wins: a preset the policy does not allow is off, whatever the file says.
  return policy?.presets == null
    ? asked
    : asked.filter((preset) => policy.presets?.includes(preset.word));
}

/** What the sentence at the top is made from. */
export type Reach = {
  /** Whether the project turned the sandbox on. */
  on: boolean;
  /** The presets turned on, in the core's order. */
  presets: readonly SandboxPreset[];
  /** The project's own hosts every chat is granted, as the core counts them. */
  projectHosts: number;
  /** Your own hosts every chat here is granted on this machine, as the core counts them. */
  yourHosts: number;
  /** The folders you let every chat here write on this machine, as the core counts them. */
  folders: number;
  /** Whether a chat may ask the system's certificate service: certificate checks on (the
   *  default), on a Mac, the only system where the setting widens anything (D-1337-7). */
  certificateService: boolean;
};

/** `a`, `a and b`, `a, b and c`. */
function listed(items: readonly string[]): string {
  if (items.length < 2) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}

/** A preset's name inside a sentence: "Code hosting" as "code hosting", "AI providers" kept. */
function midSentence(title: string): string {
  return /^[A-Z][a-z]/.test(title) ? title[0].toLowerCase() + title.slice(1) : title;
}

/** `1 thing`, `2 things`. */
function counted(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** **The sentence at the top**: what a chat here can change and reach, as the files stand. */
export function whatAChatCanDo(reach: Reach): string {
  if (!reach.on)
    return "Chats here run without a sandbox, so a chat can change any file you can and reach any host.";
  const caches = reach.presets.some((preset) => preset.widens_caches);
  const places = [
    "the folder it works in",
    ...(caches ? ["in the project's package caches"] : []),
    ...(reach.folders > 0
      ? [
          `in ${counted(reach.folders, "folder you allowed on this machine", "folders you allowed on this machine")}`,
        ]
      : []),
  ];
  const reached = [
    ...reach.presets.map((preset) => midSentence(preset.title)),
    ...(reach.projectHosts > 0
      ? [counted(reach.projectHosts, "project host", "project hosts")]
      : []),
    ...(reach.yourHosts > 0 ? [counted(reach.yourHosts, "host of yours", "hosts of yours")] : []),
    ...(reach.certificateService ? ["the system's certificate service"] : []),
  ];
  return `A chat here can change files in ${listed(places)}, and reach ${
    reached.length > 0 ? listed(reached) : "no host on the internet"
  }.`;
}

/** A sentence that starts with a capital: the core's opt-out count starts lowercase. */
function capitalised(said: string): string {
  return said.length > 0 ? said[0].toUpperCase() + said.slice(1) : said;
}

/**
 * **The mode as a status line** (#1340): a project's sandbox is on for everyone or not set, and
 * the line says the level your chats start at (#2071). Turn the sandbox
 * on writes `mode = "on"`, which has no Undo (D-SE17g); a person turns it down for their own
 * chats with their level (Change your level, ADR 0096), and a team with a commit of its own.
 */
function modeStatus(sandbox: SandboxState | undefined, yourLevel?: () => void): Control {
  // A project whose manifest is gone or unreadable refuses every chat (#1411, D-1410e): that is
  // its state, never "off", and turning the sandbox on would write a manifest in its place.
  const manifest = sandbox?.manifest ?? null;
  if (manifest !== null)
    return {
      ...textAt(SANDBOX_MODE, "Sandbox"),
      kind: "status",
      hint: "Restore the manifest to change the sandbox here.",
      status: () => manifest,
    };
  // Where policy requires the sandbox (#1423), that is the mode, whatever the file says, and
  // there is nothing to turn on.
  const required = sandbox?.policy?.required ?? null;
  return {
    ...textAt(SANDBOX_MODE, "Sandbox"),
    kind: "status",
    hint:
      required === null
        ? "Settings turns it on for everyone here. Your own level turns it down for your chats alone."
        : "Policy on this machine turns it on for every project.",
    // Any `mode` reads as on: one that is not "on" is refused and read as on (ADR 0067).
    status: (value) => {
      const on = required !== null || value !== "";
      return [
        on
          ? (required ?? "On for everyone in this project.")
          : "Not turned on in this project, so chats here start at the floor.",
        // Your own level, where it starts your chats here otherwise (#2071).
        ...yoursSaid(sandbox?.yours ?? null, on),
        // The opt-out count is this machine's, and is never sent (ADR 0067 §7, V78 d).
        ...(on && sandbox?.on === true && sandbox.said !== null
          ? [`${capitalised(sandbox.said)}. Counted on this machine only, and never sent.`]
          : []),
      ].join(" ");
    },
    ...(required === null ? { turnOn: { label: "Turn the sandbox on", value: "on" } } : {}),
    // Straight to the page where you set your own level (#2071).
    ...(yourLevel === undefined
      ? {}
      : { goTo: { label: "Change your level", onPress: yourLevel } }),
  };
}

/** What your own level makes of the project's for your chats, where it changes it (#2071). */
function yoursSaid(yours: YourLevel | null, on: boolean): string[] {
  if (yours === null) return [];
  const how = yours.confirmed ? "your level, confirmed on this machine" : "your level";
  switch (yours.level) {
    case "off":
      return [`Off for you: ${how}.`];
    case "floor":
      return on ? [`Floor for you: ${how}.`] : [];
    case "on":
      return on ? [] : [`On for you: ${how}.`];
    default:
      return [];
  }
}

/**
 * **Internet access** (#1340, `CONTEXT.md`): one box per preset, named as the core names it, each
 * opening to the hosts the core lists for it. Unticking every box writes `[]`, which reaches no
 * host, rather than taking the key out, which would reach every preset.
 *
 * Taking the key out — the row's reset, or an Undo of the change that first wrote it — is
 * allowed, since it is the same as ticking every box, but it widens what a chat may reach. So
 * the reset says so on its face, and the line under the row says it of both (#1197).
 */
function internetAccess(table: readonly SandboxPreset[], policy: SandboxPolicy | null): Control {
  const allowed = policy?.presets ?? null;
  const lockedBy = policy?.locked_by ?? "";
  if (allowed !== null)
    // Locked: what is on, said, and no box to tick (#1343).
    return {
      ...textAt(EGRESS, "Internet access"),
      kind: "status",
      hint: "The hosts every chat here may reach, besides the project's own below.",
      read: (file) =>
        presetsOn(file, table, policy)
          .map((preset) => preset.word)
          .join("\n"),
      status: (value) => {
        const on = table.filter((preset) => value.split("\n").includes(preset.word));
        return `${
          on.length > 0
            ? `${listed(on.map((preset) => preset.title))} ${on.length === 1 ? "is" : "are"} on.`
            : "No preset is on."
        } ${lockedBy}`;
      },
    };
  return {
    id: JSON.stringify(EGRESS),
    label: "Internet access",
    hint: "The hosts every chat here may reach, besides the project's own below. A host nothing allows is blocked, and the chat's tab says so. Reset, or an Undo that takes this out of the file, turns every preset on.",
    kind: "checks",
    resets: "Reset: reach every preset",
    options: table.map((preset) => ({
      value: preset.word,
      label: preset.title,
      says: (
        <details className="sandbox-hosts">
          <summary>{counted(preset.hosts.length, "host", "hosts")}</summary>
          <ul>
            {preset.hosts.map((host) => (
              <li key={host}>
                <code>{host}</code>
              </li>
            ))}
          </ul>
        </details>
      ),
    })),
    read: (file) => {
      const value = valueAt(file, EGRESS);
      return value?.kind === "list"
        ? value.value.join("\n")
        : table.map((preset) => preset.word).join("\n");
    },
    edits: (draft) => [
      {
        path: EGRESS,
        value: {
          kind: "list",
          value: draft
            .split("\n")
            .map((one) => one.trim())
            .filter((one) => one !== ""),
        },
      },
    ],
  };
}

/** What the Certificate checks switch says it does, and what it leaves open. */
export const CERTIFICATE_CHECKS_HELP =
  "Lets tools such as gh, glab and other Go programs verify a host's certificate through the system's certificate service on macOS. On by default. That service makes its own requests, for revoked certificates and the certificates a certificate names, and those do not go through the sandbox's proxy. Turn it off for a strict project: those tools then fail in a sandboxed chat with a certificate error.";

/**
 * **`[sandbox] certificate-checks`, one control, on by default** (#1337; the operator's ruling
 * R1 of 2026-10-13, amending ADR 0093). The file only ever turns it off: `false` is off for
 * everyone in the project, and `true` says what a file that does not set it says. Where an
 * administrator's policy turns it off, the row is a status line that says who set it, and
 * nothing here changes it (#1343).
 */
function certificateChecks(sandbox: SandboxState | undefined): Control {
  const locked = sandbox?.certificate_check?.locked ?? null;
  if (locked !== null)
    return {
      ...textAt(CERTIFICATE_CHECKS, "Certificate checks"),
      kind: "status",
      hint: CERTIFICATE_CHECKS_HELP,
      status: () => `Off. ${locked}`,
    };
  return onOffAt(
    CERTIFICATE_CHECKS,
    "Certificate checks",
    CERTIFICATE_CHECKS_HELP,
    "not set — on",
    // On while the file does not set it (R1): the control shows that, and Reset goes back.
    true,
  );
}

/** The sandbox's settings kept in the committed file, in the order the page draws them. */
export function sandboxControls(
  sandbox: SandboxState | undefined,
  yourLevel?: () => void,
): Control[] {
  return [
    modeStatus(sandbox, yourLevel),
    internetAccess(sandbox?.presets ?? [], sandbox?.policy ?? null),
    certificateChecks(sandbox),
  ];
}

/** What the top says while the core has not said what the sandbox grants (#1340). */
export const READING = "Reading this project's sandbox…";

/**
 * The sentence at the top, as the committed file and the core say it now. Where the sandbox is
 * on and the core has not answered — or could not — it says it is reading, never a reach it
 * does not know (#1340).
 */
export function sandboxSentence(shared: SettingsFile, sandbox: SandboxState | undefined): string {
  // A gone or unreadable manifest refuses every chat (#1411): the core's sentence says so.
  if (sandbox?.manifest != null) return sandbox.manifest;
  const on = valueAt(shared, SANDBOX_MODE) !== undefined || sandbox?.on === true;
  if (!on) return whatAChatCanDo({ ...NOTHING, on: false });
  if (sandbox === undefined) return READING;
  // As the file stands on this page, so the sentence follows the control: on unless it says
  // `false`, policy does not lock it, and this system has such a service at all.
  const checks = valueAt(shared, CERTIFICATE_CHECKS);
  const check = sandbox.certificate_check ?? null;
  return whatAChatCanDo({
    on,
    presets: presetsOn(shared, sandbox.presets, sandbox.policy),
    projectHosts: sandbox.besides.project_hosts,
    yourHosts: sandbox.besides.your_hosts,
    folders: sandbox.besides.folders,
    certificateService:
      check !== null &&
      check.widens &&
      check.locked === null &&
      !(checks?.kind === "bool" && !checks.value),
  });
}

/** A reach of nothing at all. */
const NOTHING: Reach = {
  on: true,
  presets: [],
  projectHosts: 0,
  yourHosts: 0,
  folders: 0,
  certificateService: false,
};

/**
 * When a change on this page reaches a chat (#1428): a chat's sandbox is compiled as it starts,
 * so one that is running keeps the sandbox it has. Restart chat is the row on its tab's menu.
 */
export const NEXT_START =
  "A change here applies to a chat from its next start. A chat that is running keeps the sandbox it started with until you restart it, with Restart chat on its tab's menu.";

/**
 * The sentences under the top one: what a chat can read; when a change reaches a chat; each
 * persona's own hosts, and who else holds them; and the harnesses never sandboxed on this
 * machine.
 */
export function sandboxNotes(shared: Shown, sandbox: SandboxState | undefined): string[] {
  if (valueAt(shared, SANDBOX_MODE) === undefined && sandbox?.on !== true) return [];
  const policy = sandbox?.policy ?? null;
  const lockedBy = policy?.locked_by ?? "";
  const personaLocked = policy?.persona_hosts === true;
  const allowedHosts = policy?.hosts ?? null;
  return [
    // Reads are confined only by the classes (ADR 0067 §5): your own logins are not hidden.
    "A chat can read any file you can, except vaults and purlis's own keys, so keep secrets in a vault.",
    NEXT_START,
    ...(personaLocked
      ? (sandbox?.persona_hosts ?? []).map(
          (one) =>
            `A chat as ${one.persona} reaches none of its own hosts (${listed(one.hosts)}). ${lockedBy}`,
        )
      : (sandbox?.persona_hosts ?? []).map(personaSaid)),
    ...(allowedHosts !== null
      ? [
          allowedHosts.length > 0
            ? `Only these hosts may be added, by the project or its forges, you, a persona or a block's Allow: ${listed(allowedHosts)}. Any other is not reached. ${lockedBy}`
            : `No host may be added, by the project or its forges, you, a persona or a block's Allow. ${lockedBy}`,
        ]
      : []),
    ...(!personaLocked && (sandbox?.persona_hosts ?? []).length > 0
      ? [
          // start::grants_persona, D-1362-5 and D-1362-6.
          "A chat that names no persona reaches its default persona's hosts, once they are allowed on this machine. A chat opened by a handoff may hold the asking chat's hosts, and a Resume the default persona's, until you allow its own on its tab.",
        ]
      : []),
    ...(sandbox !== undefined && sandbox.never.length > 0
      ? [`On this machine these run without it: ${sandbox.never.join("; ")}.`]
      : []),
  ];
}

/**
 * **What a chat as one persona reaches of its own hosts here** (#1362, D-1362-7): only what the
 * person on this machine allowed as it stands, said with the next step where there is one.
 */
export function personaSaid(one: PersonaHosts): string {
  const noPersona = one.default ? ", and so does every chat that names no persona" : "";
  // Its tools wait for the same Allow (D-PE-ruling-1).
  if (one.tools.length > 0) {
    const lets = whatItLets(one);
    // T-4: the core's sentence for a general-purpose tool, beside what is allowed.
    const cautions = cautionsSaid(one) ? ` ${cautionsSaid(one)}` : "";
    if (one.allowed) return `A chat as ${one.persona} may ${lets}${noPersona}.${cautions}`;
    if (one.waiting)
      return `A chat as ${one.persona} gets none of its own ${whatWaits(one)}: they changed after you allowed them on this machine. Allow them again below.${cautions}`;
    return `A chat as ${one.persona} may ${lets} only once you allow them on this machine, below.${cautions}`;
  }
  if (one.allowed)
    return `A chat as ${one.persona} also reaches ${listed(one.reached)}${noPersona}.`;
  if (one.reached.length === 0)
    return `A chat as ${one.persona} reaches none of its own hosts (${listed(one.hosts)}): policy allows none of them.`;
  if (one.waiting)
    return `A chat as ${one.persona} reaches none of its own hosts (${listed(one.reached)}): they changed after you allowed them on this machine. Allow them again below.`;
  return `A chat as ${one.persona} reaches ${listed(one.reached)} only once you allow them on this machine, below.`;
}

/** **Allow** for one persona's hosts, in Settings: the same window-only call and digest as the
 *  project view's Notice. */
function PersonaHostsAllow({
  plane,
  one,
  ids,
}: {
  plane: PlaneId;
  one: PersonaHosts;
  ids: RowIds;
}) {
  const [said, setSaid] = useState<string>();
  return (
    <div
      id={ids.id}
      className="ui-setting-status"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      {said === undefined ? (
        <button
          type="button"
          onClick={() =>
            void allowPersonaHosts(plane, one).then((answer) =>
              setSaid(
                "state" in answer ? `Allowed on this machine. ${FROM_NEXT_START}` : answer.refused,
              ),
            )
          }
        >
          {allowLabel(one)}
        </button>
      ) : (
        <p>{said}</p>
      )}
      {/* The persona stays in its own tab, and Settings links to it (#1388). */}
      <OutLink plane={plane} action={showPersona(one.persona)}>
        {`Show ${one.persona}`}
      </OutLink>
    </div>
  );
}

/**
 * **A row per persona whose hosts or tools wait for this machine's Allow** (#1362,
 * D-PE-ruling-1): the Allow itself, the window's alone, so a link draws none. None for a persona
 * with no tool and none of whose hosts a chat would reach (none where policy forbids them).
 */
export function personaHostsRows(plane: PlaneId, sandbox: SandboxState | undefined): LiveSetting[] {
  if (!mayAllowPersonaHosts() || sandbox === undefined) return [];
  const hostsLocked = sandbox.policy?.persona_hosts === true;
  return sandbox.persona_hosts
    .filter(
      (one) => !one.allowed && ((!hostsLocked && one.reached.length > 0) || one.tools.length > 0),
    )
    .map((one) => ({
      id: `project.sandbox.persona.${one.persona}`,
      label: `${one.persona}'s ${whatWaits(one, hostsLocked)}`,
      // The Allow is this machine's, never the committed file's (#1759).
      writes: "this machine" as const,
      help: personaSaid(one),
      useControl: () => ({
        grouped: true,
        control: (ids) => <PersonaHostsAllow plane={plane} one={one} ids={ids} />,
      }),
    }));
}

/** The row of what the project expects and this machine has not allowed (ADR 0093). */
export const EXPECTS_HERE = "project.sandbox.expects-here";

/** **Allow on this machine**, or what stands: the window-only call the project's Notice makes
 *  too, kept in this machine's own file. Offered after a "Not now" as before it. What waits is
 *  read again whenever a sandbox command of the window's returns (#1888), so an Allow made on
 *  the project's Notice is not offered here again. */
function ExpectsHere({
  plane,
  expects,
  ids,
}: {
  plane: PlaneId;
  expects: NonNullable<SandboxState["expects"]>;
  ids: RowIds;
}) {
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  /** Asked again after an Allow that was refused: what waits may stand otherwise by now. */
  const [again, setAgain] = useState(0);
  const [now, setNow] = useProjectExpects(plane, expects, again);
  // Nothing waits any more: what is left is the lock, as it was drawn.
  const left = now ?? { ...expects, items: [] };
  const offered = left.locked === null && left.items.some(allowable) && mayAnswerExpects() && !busy;
  const allow = () => {
    setBusy(true);
    setSaid(undefined);
    void commands
      .allowProjectExpects(plane, asDrawn(left.items))
      .then((answer) => {
        if (answer.status !== "ok") {
          setSaid(answer.error);
          // Refused: the list is read again, so what is drawn is what stands now.
          setAgain((n) => n + 1);
          return;
        }
        setNow(answer.data.expects ?? null);
        setSaid(
          "Allowed on this machine: every chat of this project has it from its next start, and a chat already running takes it when it restarts. Remove it under Network, in the Allowed list.",
        );
        // The chats open now keep the sandbox they started with: their Notice asks again.
        sandboxCommandReturned();
      })
      .catch((err: unknown) => setSaid(`purlis could not allow it: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  return (
    <div
      id={ids.id}
      role="group"
      className="ui-setting-status"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      <ExpectedLines items={left.items} />
      {left.items.length === 0 && said === undefined && <p>This machine allows all of it now.</p>}
      {left.locked !== null && <p>{left.locked}</p>}
      {said !== undefined && <p>{said}</p>}
      {offered && (
        <button type="button" className="ui-button" onClick={allow}>
          {ALLOW_EXPECTS}
        </button>
      )}
    </div>
  );
}

/**
 * **What the project expects and this machine has not allowed, as a row** (ADR 0093, ruling 2):
 * the committed `[sandbox] expects` declares, and never grants, so the row says what a chat here
 * lacks and allows it, whether or not the person answered the project's Notice with Not now.
 * None where nothing waits.
 */
export function expectsRows(plane: PlaneId, sandbox: SandboxState | undefined): LiveSetting[] {
  const expects = sandbox?.expects;
  if (expects === undefined || expects === null || expects.items.length === 0) return [];
  return [
    {
      id: EXPECTS_HERE,
      label: "This project expects",
      // The Allow is this machine's, never the committed file's (#1759).
      writes: "this machine" as const,
      help: "What this project's settings say its chats need, and this machine has not allowed. The settings only say so: nothing on the list reaches a chat until you allow it here, and anyone who can change the settings can change the list.",
      useControl: () => ({
        grouped: true,
        control: (ids) => <ExpectsHere plane={plane} expects={expects} ids={ids} />,
      }),
    },
  ];
}

/** One item of a read-only list: what, and why in one sentence. */
type Reason = { what: string; why: string };

/**
 * **What a sandboxed chat may change** (ADR 0067 §2, #1332, #1333, #1335, #1337, #1342): its
 * own folder and temp folder; the project's package caches with Package registries; what it asks
 * purlis to write for it; and what a person granted it.
 */
function canChange(caches: boolean): Reason[] {
  return [
    {
      what: "The folder it works in",
      why: "The project, workspace or clone the chat was opened in, where its work goes.",
    },
    { what: "Its temporary files", why: "Programs need somewhere to put scratch files." },
    {
      what: "The harness's own state",
      why: "What the harness keeps for the chat, such as its session data, so the chat can run.",
    },
    ...(caches
      ? [
          {
            what: "The project's package caches",
            why: "So builds can download packages. They are this project's own, and your caches are only read.",
          },
        ]
      : []),
    {
      what: "Session records, memory and todos",
      why: "The chat asks purlis to write them, so they work from any chat, one in a clone included.",
    },
    {
      what: "Repos and their branch folders",
      why: "The chat asks purlis, which writes the git settings the sandbox keeps from the chat.",
    },
    {
      what: "Folders and hosts you allow",
      why: "From a block's Allow, each listed under Network, where you can remove it. A folder is only ever one inside the project, or one you list there, never a temp folder.",
    },
  ];
}

/** **What no chat may change, whatever is granted** (ADR 0067 §5): the denial classes. */
function alwaysProtected(shared: string, local: string): Reason[] {
  return [
    {
      what: "Vaults",
      why: "A chat cannot read where secrets are kept. A command gets a secret only through purlis.",
    },
    {
      what: "What purlis records about chats",
      why: "Only purlis writes it, so a chat cannot change the record of what it did.",
    },
    {
      what: "Your approvals in purlis",
      why: "Only you approve what runs and what is granted, so a chat cannot approve itself.",
    },
    {
      what: "Files other programs load later",
      why: "Git's config and hooks, shell startup files, and editor and harness settings. A program run outside the sandbox would run what a chat put there.",
    },
    {
      what: `This project's ${shared} and ${local}`,
      why: "At the project root and in each folder from the chat's up to it, because they set the sandbox itself.",
    },
  ];
}

/**
 * **A value an administrator's policy locks** (#1343), as a row of its own: what it says —
 * "Locked by policy" and who set it — and no control.
 */
export function lockedRow(id: string, label: string, said: string): LiveSetting {
  return {
    id,
    label,
    writes: "nothing",
    help: "",
    useControl: () => ({
      grouped: true,
      control: (ids) => (
        <p
          id={ids.id}
          className="ui-setting-status"
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
        >
          {said}
        </p>
      ),
    }),
  };
}

/** A read-only list of reasons, as a row of its own. */
function reasons(id: string, label: string, help: string, items: readonly Reason[]): LiveSetting {
  return {
    id,
    label,
    writes: "nothing",
    help,
    useControl: () => ({
      grouped: true,
      control: (ids) => (
        <ul
          id={ids.id}
          className="sandbox-reasons"
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
        >
          {items.map((one) => (
            <li key={one.what}>
              <span className="sandbox-what">{one.what}</span>
              <span className="sandbox-why">{one.why}</span>
            </li>
          ))}
        </ul>
      ),
    }),
  };
}

/** The two read-only lists, drawn under the project's hosts. */
export function sandboxReasons(
  shared: SettingsFile,
  local: SettingsFile,
  sandbox: SandboxState | undefined,
): LiveSetting[] {
  const when =
    valueAt(shared, SANDBOX_MODE) !== undefined || sandbox?.on === true
      ? ""
      : " Once the sandbox is on.";
  const caches = presetsOn(shared, sandbox?.presets ?? [], sandbox?.policy ?? null).some(
    (one) => one.widens_caches,
  );
  return [
    reasons(
      "project.sandbox.changes",
      "What chats can change",
      `What a sandboxed chat may write.${when}`,
      canChange(caches),
    ),
    reasons(
      "project.sandbox.protected",
      "Always protected",
      `What no chat may change, whatever you allow.${when}`,
      alwaysProtected(shared.file, local.file),
    ),
  ];
}

/** The level's name for the personas' hosts as a collection, which its writes are sent as. */
export const PERSONA_HOSTS_COLLECTION = "personaHosts";

/** `[sandbox.personas.<persona>]`, the persona's table. */
const personaTable = (persona: string) => key("sandbox", "personas", persona);

/** `[sandbox.personas.<persona>] hosts`. */
const personaHostsKey = (persona: string) => key("sandbox", "personas", persona, "hosts");

/** One persona's `hosts` as the committed file holds it: `undefined` where it is not a list of
 *  hosts, which only Edit as TOML can mend (the core says why, with the file's refusals). */
export type PersonaList = { persona: string; hosts: readonly string[] | undefined };

/** **Each persona's hosts in the committed file**, in file order, read from the core's fields:
 *  every `[sandbox.personas.<p>] hosts` there is. */
export function personaLists(shared: Shown): PersonaList[] {
  return shared.fields
    .filter(
      (field) =>
        field.path.length === 4 &&
        field.path[0].key === "sandbox" &&
        field.path[1].key === "personas" &&
        field.path[3].key === "hosts" &&
        field.path[2].key !== undefined,
    )
    .map((field) => ({
      persona: field.path[2].key ?? "",
      hosts: field.value.kind === "list" ? field.value.value : undefined,
    }));
}

/** An entry's identity: the persona, and the host's place in its list as the file was drawn.
 *  A change made elsewhere since is the core's to refuse, by the text it was drawn from. */
const entryId = (persona: string, at: number) => `${persona}\u0000${at}`;

/** What an entry is called: the persona and the host, as the file spells it. */
const entryLabel = (persona: string, host: string) => `${persona}: ${host}`;

/**
 * **The personas' hosts as a collection** (#1407): each host of each `[sandbox.personas.<p>]`
 * with Remove, and an Add form of the persona and one host. The host is checked as it is typed
 * by the core's parser (`check_sandbox_host`, #1405), as the project's own are. The personas
 * offered are the project's (`personas`); until the core has listed them, those the file names.
 *
 * Unlike the project's hosts, these are not a collection the core lists: the entries are read
 * from the file's fields here, and written through the one settings save
 * ({@link personaHostsChange}), which keeps every rule the core reads the file by.
 */
export function personaHostsCollection(
  shared: SettingsFile,
  personas: readonly string[] | undefined,
): Collection {
  const lists = personaLists(shared);
  const names = personas ?? lists.map((one) => one.persona);
  return {
    name: PERSONA_HOSTS_COLLECTION,
    noun: "host",
    base: shared.exists ? shared.text : null,
    entries: lists.flatMap((one) =>
      (one.hosts ?? []).map((host, at) => ({
        id: entryId(one.persona, at),
        label: entryLabel(one.persona, host),
        settings: [],
      })),
    ),
    fields: [
      {
        field: "persona",
        label: "Persona",
        help: "Whose chats reach the host: a persona this project defines.",
        kind: "choice",
        choices: names,
        initial: names[0],
      },
      {
        field: "host",
        label: "Host",
        help: "A domain such as api.example.com, *.example.com for every name under it, or an IP address such as 10.0.0.5. Add :port to reach a port other than HTTPS's.",
        kind: "text",
        // The core's parser, as the field is typed in (#1405).
        check: (typed) => commands.checkSandboxHost(typed),
      },
    ],
  };
}

/** What an add or a remove of a persona's host writes, and what it says once written. */
export type PersonaHostsWrite = {
  edits: SettingsEdit[];
  said: string;
  /** The added host's identity, for its Undo; none for a remove. */
  added?: string;
};

/** Why an add or a remove of a persona's host writes nothing, by field or for the file. */
export type PersonaHostsRefusal = { fields: Record<string, string[]>; reasons: string[] };

/**
 * **What adding or removing one persona's host writes** to `shared` (#1407): the persona's
 * whole `hosts` list, with the host added at its end or taken out; the persona's table taken
 * out with its last host. A host the list already holds, an empty field, a list that is not one
 * and an entry no longer there are refused here; everything else is the core's to refuse.
 */
export function personaHostsChange(
  shared: Shown,
  op: { add: { persona?: string; host?: string } } | { remove: string },
): PersonaHostsWrite | PersonaHostsRefusal {
  const lists = personaLists(shared);
  const notAList = (persona: string): PersonaHostsRefusal => ({
    fields: {},
    reasons: [
      `${persona}'s hosts in ${shared.file} are not a list, so a form cannot change them. Mend them under Edit as TOML.`,
    ],
  });
  if ("add" in op) {
    const persona = (op.add.persona ?? "").trim();
    const host = (op.add.host ?? "").trim();
    const fields: Record<string, string[]> = {};
    if (persona === "") fields.persona = ["Pick the persona whose chats reach it."];
    if (host === "") fields.host = ["Type the host."];
    if (Object.keys(fields).length > 0) return { fields, reasons: [] };
    const listed = lists.find((one) => one.persona === persona);
    if (listed !== undefined && listed.hosts === undefined) return notAList(persona);
    const hosts = listed?.hosts ?? [];
    if (hosts.some((one) => one.toLowerCase() === host.toLowerCase()))
      return { fields: { host: [`${host} is already one of ${persona}'s hosts.`] }, reasons: [] };
    return {
      edits: [{ path: personaHostsKey(persona), value: { kind: "list", value: [...hosts, host] } }],
      said: `Added ${entryLabel(persona, host)}.`,
      added: entryId(persona, hosts.length),
    };
  }
  const [persona, place] = op.remove.split("\u0000");
  const at = Number(place);
  const listed = lists.find((one) => one.persona === persona);
  if (listed !== undefined && listed.hosts === undefined) return notAList(persona);
  const host = listed?.hosts?.[at];
  if (listed?.hosts === undefined || host === undefined)
    return {
      fields: {},
      reasons: [
        `That host is not in ${shared.file} as it was shown, so nothing was removed. Read the file again, then remove it again.`,
      ],
    };
  const left = listed.hosts.filter((_, one) => one !== at);
  return {
    edits: [
      left.length === 0
        ? { path: personaTable(persona), value: null }
        : { path: personaHostsKey(persona), value: { kind: "list", value: left } },
    ],
    said: `Removed ${entryLabel(persona, host)}.`,
  };
}

/**
 * **The page's sentences** (#1407): for each persona, the hosts its list holds that an
 * administrator's policy keeps every chat from, as the core read them (`reached`): written, and
 * reaching nothing; and each of its tools that runs nothing for the program it runs here.
 */
export function personaHostsNotes(sandbox: SandboxState | undefined): string[] {
  return (sandbox?.persona_hosts ?? []).flatMap((one) => {
    const kept = one.hosts.filter((host) => !one.reached.includes(host));
    const locked =
      kept.length === 0 || sandbox?.policy?.persona_hosts === true
        ? []
        : [
            `${one.persona} lists ${listed(kept)}, which policy on this machine keeps every chat from. ${sandbox?.policy?.locked_by ?? ""}`.trim(),
          ];
    // The core's sentence for each tool refused for the program it runs here.
    return [...locked, ...(one.refused ?? []).map((why) => `${why}.`)];
  });
}

/**
 * **What the page says it is for** (#1407, D-1362-7): a persona's hosts are committed, so
 * adding one here asks nobody's Allow and grants nothing by itself. Each machine's person
 * allows the list as it then stands, on Sandbox, and a list changed after that is allowed again.
 */
export function personaHostsHelp(file: string): string {
  return `Hosts a chat as one persona reaches besides the project's, kept in ${file}, which your team sees. Adding one grants nothing by itself: a chat as that persona reaches them only once the person on each machine allows the list as it then stands, on Sandbox, and a list changed after that is allowed again.`;
}
