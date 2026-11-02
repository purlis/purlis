// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { CLASSIC, DEFAULT_THEME, OLD_PICKS, PURLIS, SYSTEM, THEME_ALIASES } from "../theme/theme";
import { isWorkspaceColour, WORKSPACE_COLOURS } from "../theme/theme";
import type { Option } from "./components";
import type {
  InForce,
  ProjectExtension,
  ProjectExtensions,
  ProjectTheme,
  SavingInForce,
  SettingsEdit,
  SettingsFile,
  SettingsStep,
  SettingsValue,
} from "../bindings";

/**
 * **What a project's settings files hold, as controls** (charter-app#252): each key a form can
 * write, how it reads its value out of the file, and the edits a new value makes. First the old
 * Project settings page's two sections (retired by SE-19); now drawn by the Settings tab's
 * Project level (`project.ts`, SE-17), which regroups the same controls into V89h's groups. A workspace's (its extensions, theme, colour
 * and plugins) are drawn only by the Workspace level (`workspace.tsx`, SE-20).
 */
/**
 * One control: a label, how it is drawn, how it reads its value out of the file, and the edits a
 * new value makes. Every value a control holds is text while it is being typed; `edits` is where
 * it becomes a key.
 */
/** What a section shows of a file: a project's settings file, or a workspace's manifest. */
export type Shown = Pick<
  SettingsFile,
  "file" | "exists" | "text" | "refusals" | "parsed" | "fields"
>;

export type Control = {
  id: string;
  label: string;
  hint?: string;
  /** `text` is one line, `choice` a closed set, `lines` one entry per line, and `colour` a
   *  closed set whose `custom` pick is a `#rrggbb` of the operator's own (charter-app#281);
   *  `checks` any of a few, one entry per line, and `status` a line of text (#1340). */
  kind: "text" | "choice" | "lines" | "colour" | "checks" | "status";
  choices?: readonly string[];
  /** A `checks`' boxes, each with what it says beside it. */
  options?: readonly Option[];
  /** What a `status` says while the value is `value` (empty: not set). */
  status?: (value: string) => string;
  /** A `status`'s one button while no file sets it: its label, and the value it writes. */
  turnOn?: { label: string; value: string };
  /** A `status`'s button to the page that changes what it says, drawn whatever the value. */
  goTo?: { label: string; onPress: () => void };
  /** What a `choice`'s empty option says. */
  unset?: string;
  /** What a `choice` shows for each of its values, when that is not the value itself. */
  labels?: Readonly<Record<string, string>>;
  read: (file: Shown) => string;
  /** The edits `draft` makes to `file`, the file it was typed over. */
  edits: (draft: string, file: Shown) => SettingsEdit[];
  /** A `choice` over one of the project's own collections (ST-1): its choices are what the
   *  project has, read when the level is, and it offers New… beside them. */
  names?: Entry;
  /** What the row's reset is called, where "Reset" alone would hide what it does (#1197). */
  resets?: string;
  /**
   * **An on/off key's value in force** (#1865): what its switch shows while no file at the row's
   * level holds the key — the value beneath, from another file or the default. With it the key
   * is a switch and Reset takes it back to not set; without it (a harness plugin's, which the
   * harness decides from its own settings) the key stays a three-way select of on, off and not
   * set, since a switch would show a value purlis does not know.
   */
  inForce?: boolean;
  /**
   * **What is in force is still being read** (#1875): an on/off key that will be a switch once
   * the core says its value in force. While the file does not set it, the row holds a
   * placeholder, never a select that turns into a switch on load.
   */
  reading?: true;
};

/**
 * **A collection a setting names an entry of** (ST-1, #1225; V91r): the project's personas, its
 * workspaces, or the harness profiles a chat can start on — built in, or declared in
 * `charter.local.toml`.
 */
export type Entry = "persona" | "workspace" | "profile";

/** What each {@link Entry} lists, as the core answered it. */
export type Entries = Readonly<Record<Entry, readonly string[]>>;

/**
 * **A picker over one of the project's collections** (ST-1): a `choice` whose choices are filled
 * in from what the core lists when the level is read, with New… beside them.
 */
export function pickAt(path: SettingsStep[], label: string, names: Entry, hint?: string): Control {
  return { ...textAt(path, label, { kind: "choice", hint }), names, unset: "not set" };
}

/** What `project_extensions` answers before it has answered, or when it could not. */
export const NO_EXTENSIONS: ProjectExtensions = { extensions: [], local_left_out: null };

/** A heading and what is under it. `extensions` is what the core says is in force in this
 *  project, and `theme` what it says the project draws, for the groups that draw them. */
export type Group = {
  title: string;
  note?: string;
  controls: (
    file: Shown,
    extensions: readonly ProjectExtension[],
    theme: ProjectTheme | undefined,
    saving: Saving,
  ) => Control[];
  /** Sentences about this file the group says under its heading. */
  notes?: (
    file: Shown,
    extensions: readonly ProjectExtension[],
    theme: ProjectTheme | undefined,
    saving: Saving,
  ) => string[];
  /** For a group that shows what is in force: why `charter.local.toml` had no say in it, as the
   *  core's answer carries it (charter-app#319), or `null` when it had its say. */
  leftOut?: (
    extensions: ProjectExtensions,
    theme: ProjectTheme | undefined,
    saving: Saving,
  ) => string | null;
  /** What the group says when it has no controls; "None in this file." otherwise. */
  empty?: string | ((saving: Saving) => string);
};

/** What `project_saving_in_force` answered: nothing yet, the answer, or why it could not. */
export type Saving = SavingInForce | { trouble: string } | undefined;

/** The answer in `saving`, when there is one. */
export function answered(saving: Saving): SavingInForce | undefined {
  return saving === undefined || "trouble" in saving ? undefined : saving;
}

export const key = (...keys: string[]): SettingsStep[] => keys.map((one) => ({ key: one }));

function same(a: readonly SettingsStep[], b: readonly SettingsStep[]): boolean {
  return (
    a.length === b.length && a.every((step, at) => JSON.stringify(step) === JSON.stringify(b[at]))
  );
}

export function valueAt(file: Shown, path: readonly SettingsStep[]): SettingsValue | undefined {
  return file.fields.find((field) => same(field.path, path))?.value;
}

/** A value as one line of text: what a text box or a choice shows. */
export function shown(value: SettingsValue | undefined): string {
  if (value === undefined) return "";
  switch (value.kind) {
    case "text":
    case "other":
      return value.value;
    case "list":
      return value.value.join("\n");
    case "bool":
    case "integer":
      return String(value.value);
  }
}

/** One key holding one piece of text: an empty box removes the key. */
export function textAt(
  path: SettingsStep[],
  label: string,
  more: Partial<Pick<Control, "hint" | "kind" | "choices">> = {},
): Control {
  return {
    id: JSON.stringify(path),
    label,
    kind: "text",
    ...more,
    read: (file) => shown(valueAt(file, path)),
    edits: (draft) => [
      {
        path,
        value: draft.trim() === "" ? null : { kind: "text", value: draft.trim() },
      },
    ],
  };
}

/** One key holding a list of text, one entry per line. An empty box removes the key. */
export function listAt(path: SettingsStep[], label: string, hint?: string): Control {
  return {
    id: JSON.stringify(path),
    label,
    hint,
    kind: "lines",
    read: (file) => shown(valueAt(file, path)),
    edits: (draft) => {
      const items = entries(draft);
      return [{ path, value: items.length === 0 ? null : { kind: "list", value: items } }];
    },
  };
}

export function entries(draft: string): string[] {
  return draft
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
}

/** The places `[[forge]]` blocks are at. */
export function forgeBlocks(file: Shown): number[] {
  const at = new Set<number>();
  for (const { path } of file.fields) {
    const [first, second] = path;
    if (first?.key === "forge" && second?.index !== undefined) at.add(second.index);
  }
  return [...at];
}

/** A profile's `env` as `NAME=value` lines — one key per variable, so a line removed is a key
 *  removed and the others are left exactly as they were written. */
export function envAt(name: string): Control {
  const env = key("harness", name, "env");
  const pairs = (file: Shown) =>
    file.fields
      .filter((field) => field.path.length === 4 && same(field.path.slice(0, 3), env))
      .map((field) => [field.path[3].key ?? "", shown(field.value)] as const);
  return {
    id: JSON.stringify(env),
    label: "Environment",
    hint: "NAME=value, one per line. A credential does not belong here: log in inside the harness.",
    kind: "lines",
    read: (file) =>
      pairs(file)
        .map(([n, v]) => `${n}=${v}`)
        .join("\n"),
    edits: (draft, file) => {
      const before = pairs(file);
      const after = new Map(
        entries(draft).map((line) => {
          const cut = line.indexOf("=");
          return cut < 0 ? [line, ""] : [line.slice(0, cut).trim(), line.slice(cut + 1).trim()];
        }),
      );
      const gone: SettingsEdit[] = before
        .filter(([n]) => !after.has(n))
        .map(([n]) => ({ path: [...env, { key: n }], value: null }));
      const set: SettingsEdit[] = [...after]
        .filter(([n, v]) => before.find(([was]) => was === n)?.[1] !== v)
        .map(([n, v]) => ({ path: [...env, { key: n }], value: { kind: "text", value: v } }));
      return [...gone, ...set];
    },
  };
}

/** Where a source is, as the end of a sentence: a file, or this workspace. */
export function where(source: string): string {
  if (source === "local") return "charter.local.toml";
  if (source === "workspace") return "this workspace";
  return "charter.toml";
}

/** What an extension is in this project, and why, in a sentence after its name. */
function standing(it: ProjectExtension): string {
  const file = where(it.source);
  switch (it.state) {
    case "on":
      return it.source === "default"
        ? "on — installed and approved on this machine"
        : `on — enabled in ${file}`;
    case "off":
      return `off — turned off in ${file}`;
    case "needs-approval":
      return `needs approval here — ${it.source === "default" ? "installed" : `enabled in ${file}`}, and this machine has not approved it. Approve it in Extensions.`;
    default:
      return `not installed here — named in ${file}; install it from Extensions to use it`;
  }
}

/**
 * One key holding true or false, as a closed choice: on, off, or not set here. With `inForce`,
 * the value in force while the key is not set here, it is drawn as a switch (#1865); with
 * `"reading"`, it will be once the core says, and holds a placeholder until then (#1875).
 */
export function onOffAt(
  path: SettingsStep[],
  label: string,
  hint: string,
  unset: string,
  inForce?: boolean | "reading",
): Control {
  return {
    id: JSON.stringify(path),
    label,
    hint,
    kind: "choice",
    choices: ["on", "off"],
    unset,
    ...(inForce === undefined ? {} : inForce === "reading" ? { reading: true } : { inForce }),
    read: (file) => {
      const value = valueAt(file, path);
      if (value?.kind !== "bool") return shown(value);
      return value.value ? "on" : "off";
    },
    edits: (draft) => [
      {
        path,
        value: draft === "" ? null : { kind: "bool", value: draft === "on" },
      },
    ],
  };
}

/** Where a resolved setting came from, as the end of a sentence. */
export function from(source: string): string {
  return source === "default" ? "its default" : `from ${where(source)}`;
}

/**
 * **Extensions, in either section** (charter-app#253, ADR 0048). A project turns an extension
 * this machine has installed on or off, and sets what it declares; Local overrides Shared key by
 * key, and neither overrides this machine's approval. The sentence under each is the core's
 * answer for the project as both files stand — which the toggle above it may be about to change.
 */
export const EXTENSIONS: Group = {
  title: "Extensions",
  empty: "No extension is installed on this machine or named by this project.",
  controls: (_file, extensions) =>
    extensions.flatMap((it) => {
      const at = (...rest: string[]) => key("extensions", it.id, ...rest);
      return [
        onOffAt(
          at("enabled"),
          `${it.name}: enabled`,
          `${it.name}: ${standing(it)}`,
          "not set — inherits",
          enabledInForce(it),
        ),
        ...it.settings.map((setting): Control => {
          const label = `${it.name}: ${setting.title}`;
          const hint = `In this project: ${setting.kind === "bool" ? (setting.value === "true" ? "on" : "off") : setting.value || "empty"}, ${from(setting.source)}.`;
          const path = at("settings", setting.key);
          if (setting.kind === "bool")
            return onOffAt(path, label, hint, "not set", setting.value === "true");
          if (setting.kind === "choice")
            return textAt(path, label, { hint, kind: "choice", choices: setting.choices });
          return textAt(path, label, { hint });
        }),
      ];
    }),
  notes: (file, extensions) =>
    extensions.flatMap((it) =>
      it.ignored.filter((one) => one.file === file.file).map((one) => one.why),
    ),
  leftOut: (extensions) => extensions.local_left_out,
};

/**
 * Whether an extension is enabled in this project, as its row's switch shows it while the row's
 * file does not set it: on where it is on or only waits for this machine's approval, off where a
 * file turned it off. Not known for one not installed here, whose row stays a select.
 */
function enabledInForce(it: ProjectExtension): boolean | undefined {
  if (it.state === "on" || it.state === "needs-approval") return true;
  if (it.state === "off") return false;
  return undefined;
}

/** charter's own picks, for when the core could not be asked what else there is: the two
 *  built-in themes and following the system (#3034). */
const BUILT_IN_PICKS = [PURLIS, CLASSIC, SYSTEM];

/** How Settings labels a pick a file may still hold that it no longer offers (#3034): Purlis's
 *  dark or light by `purlis-dark` or `purlis-light`, or an id either had. */
function oldPickLabel(pick: string): string {
  return `Purlis ${pick.endsWith("light") ? "light" : "dark"} (old id)`;
}

/**
 * **Theme, in either section** (charter-app#273, ADR 0048): `[theme] use`. Local's pick wins over
 * Shared's; an extension's theme is drawn only while the project has that extension on and this
 * machine approved it, and the core's sentence says why when the pick in force is not drawn —
 * under the section whose file made it.
 */
export function themeGroup(unset: string, here: "project" | "workspace" = "project"): Group {
  const path = key("theme", "use");
  return {
    title: "Theme",
    controls: (_file, _extensions, theme) => {
      const options = theme?.options ?? BUILT_IN_PICKS.map((value) => ({ value, label: value }));
      const labels = Object.fromEntries(options.map((one) => [one.value, one.label]));
      // A file that still holds an id Settings no longer offers is shown under what it draws
      // (#1838, #3034).
      for (const old of [...OLD_PICKS, ...Object.keys(THEME_ALIASES)])
        labels[old] ??= oldPickLabel(old);
      // What the theme levels draw here, and which level chose it (#2323), when the core said.
      const levels = theme?.drawn == null ? undefined : `${theme.drawn}, from ${theme.from ?? ""}`;
      const drawn =
        levels ??
        (theme?.draws == null
          ? `the window's own theme — your theme.json, else the first theme from an extension this ${here} has on, else ${DEFAULT_THEME.name}`
          : (labels[theme.draws] ?? theme.draws));
      // In a workspace, which of the three files picked the theme drawn here: its own, or the
      // project's that it did not override (charter-app#281). Not when the pick fell back: the
      // file picked something else, and the sentence under the group says what and why.
      const from =
        levels === undefined && here === "workspace" && theme?.file != null && theme.why === null
          ? `, picked in ${theme.file}`
          : "";
      return [
        {
          ...textAt(path, "Theme", {
            kind: "choice",
            choices: options.map((one) => one.value),
            hint: `Drawn in this ${here}: ${drawn}${from}. The terminal follows the window.`,
          }),
          unset,
          labels,
        },
        ...(here === "workspace" ? [COLOUR] : []),
      ];
    },
    notes: (file, _extensions, theme) => {
      if (theme === undefined) return [];
      // A workspace's tab is one section, and what is drawn in the workspace is its answer
      // whichever file made the pick; a project's says it under the file that made it.
      const why = here === "workspace" || theme.file === file.file ? theme.why : null;
      return [
        ...(why === null ? [] : [why]),
        ...theme.ignored.filter((one) => one.file === file.file).map((one) => one.why),
        ...(here === "workspace" && theme.colour !== null && !isWorkspaceColour(theme.colour)
          ? [
              `${theme.colour} is a grey, which has no hue to tint with — so this workspace is drawn without a colour. Pick one of the eight, or a custom colour that is not grey.`,
            ]
          : []),
      ];
    },
    leftOut: (_extensions, theme) => theme?.local_left_out ?? null,
  };
}

/** Which of the project's two files a section is. */
export type Which = "shared" | "local";

/** Which file a group's controls are about: one of the two, or `either` — the Settings tab's one
 *  row per key, whose value may be kept in either file (SE-18) and whose row says which. */
export type Section = Which | "either";

/** The modes `planesave::Mode` reads, in the ladder's order. */
const MODES = ["off", "commit", "push", "pr", "pr-merge"] as const;

/** One save key the Plane or Repos group has a control for: where it is, what it is called,
 *  what it does, and what no value means when no file sets one. */
type SaveKey = {
  key: string;
  label: string;
  kind: "mode" | "text" | "bool";
  hint: string;
  /** What `null` in force means — no file set it, and none is the answer. */
  none?: string;
};

/**
 * What the project has for one save key, and which file decided it — the marker ADR 0048's
 * overlay asks of every place that shows a value — as the end of the control's hint. In the
 * Shared section, a value this file holds that `charter.local.toml` overrides says so.
 */
function decided(
  it: InForce,
  one: SaveKey,
  file: Shown,
  path: SettingsStep[],
  section: Section,
  fromShare = false,
): string {
  if (it.value === null && one.kind === "mode") return `In this project: ${one.none}.`;
  const value = it.value ?? one.none ?? "not set";
  const origin = fromShare
    ? "from [memory] share in charter.toml, the deprecated alias"
    : from(it.source);
  const overridden =
    section === "shared" && it.source === "local" && valueAt(file, path) !== undefined
      ? " — overriding the value here"
      : "";
  return `In this project: ${value}, ${origin}${overridden}.`;
}

/** An on/off save key's value in force, as its switch shows it; `"reading"` before the core
 *  says, and unknown where it said none. */
function onOffInForce(it: InForce | undefined, reading: boolean): boolean | "reading" | undefined {
  if (reading) return "reading";
  if (it?.value === "on") return true;
  if (it?.value === "off") return false;
  return undefined;
}

/** One save key's control, writing to `path` in the section it is in. `it` is what the project
 *  has for the key, once the core has said. */
function saveControl(
  one: SaveKey,
  path: SettingsStep[],
  label: string,
  section: Section,
  marker: string | null,
  it?: InForce,
  reading = false,
): Control {
  const hint = marker === null ? one.hint : `${one.hint} ${marker}`;
  const unset = section === "local" ? "not set — charter.toml's" : "not set";
  if (one.kind === "bool") return onOffAt(path, label, hint, unset, onOffInForce(it, reading));
  if (one.kind === "mode")
    return { ...textAt(path, label, { hint, kind: "choice", choices: MODES }), unset };
  return textAt(path, label, { hint });
}

const QUIET_HINT = "A whole number of seconds or minutes, like 30s or 2m.";

/** `[plane]`'s save keys, in the order ADR 0051 and `docs/plane-format.md` list them. */
const PLANE_KEYS: readonly SaveKey[] = [
  {
    key: "mode",
    label: "Mode",
    kind: "mode",
    hint: "How far a save of the project goes: off, commit, push, pr (push to the save branch and keep one request open), or pr-merge (and set that request to auto-merge).",
    none: "not set — the Saving view asks once, before anything is pushed",
  },
  {
    key: "branch",
    label: "Target branch",
    kind: "text",
    hint: "The branch a save is meant to end up on.",
    none: "the branch the project has checked out",
  },
  {
    key: "save_branch",
    label: "Save branch",
    kind: "text",
    hint: "The one branch per machine that pr and pr-merge push to.",
    none: "purlis/save/<this machine's name>",
  },
  { key: "sign", label: "Sign commits", kind: "bool", hint: "Sign the commits a save makes." },
  {
    key: "refuse_unsandboxed_commit",
    label: "Refuse unsandboxed commits",
    kind: "bool",
    hint: "Make `purlis worktree commit` refuse a chat with no sandbox and send it to git. Off, it commits there with that chat's own git.",
  },
  {
    key: "autosave",
    label: "Auto-save",
    kind: "bool",
    hint: "Save by itself: after a quiet period, when a session ends, and when the app quits.",
  },
  { key: "autosave_after", label: "Auto-save after", kind: "text", hint: QUIET_HINT },
];

/** `[repos.<name>]`'s keys: `[plane]`'s but `save_branch`, with a repo's defaults. */
const REPO_KEYS: readonly SaveKey[] = [
  {
    key: "mode",
    label: "mode",
    kind: "mode",
    hint: "How far a save of this repo goes: off, commit, push, pr or pr-merge.",
  },
  {
    key: "branch",
    label: "branch",
    kind: "text",
    hint: "The branch a request goes into.",
    none: "the repo's default branch",
  },
  { key: "sign", label: "sign", kind: "bool", hint: "Sign the commits a save makes." },
  { key: "autosave", label: "auto-save", kind: "bool", hint: "Save this repo by itself." },
  { key: "autosave_after", label: "auto-save after", kind: "text", hint: QUIET_HINT },
];

/**
 * **Plane, in either section** (charter-app#300, ADR 0051): `[plane]`'s save keys, each with what
 * the project has and the file that decided it. Shared's also holds `[memory] share`, the
 * deprecated alias of Mode that `docs/plane-format.md` still documents — the only file it is read
 * from.
 */
export function planeGroup(section: Section): Group {
  return {
    title: "Saving",
    note: "How the project is saved. charter.local.toml overrides charter.toml key by key. Where branch folders go is under General.",
    controls: (file, _extensions, _theme, asked) => {
      const saving = answered(asked);
      return [
        ...PLANE_KEYS.map((one) => {
          const path = key("plane", one.key);
          const it = saving?.plane[one.key as keyof typeof saving.plane];
          const marker =
            it === undefined || typeof it === "boolean"
              ? null
              : decided(
                  it,
                  one,
                  file,
                  path,
                  section,
                  one.key === "mode" && saving?.plane.from_share === true,
                );
          return saveControl(
            one,
            path,
            one.label,
            section,
            marker,
            typeof it === "boolean" ? undefined : it,
            // Not answered yet, nor refused: what is in force is on its way (#1875).
            asked === undefined,
          );
        }),
        ...(section !== "local"
          ? [
              textAt(key("memory", "share"), "[memory] share (deprecated)", {
                kind: "choice",
                choices: ["local", "commit", "push"],
                hint: [SHARE_HINT, shareMarker(file, saving)].filter(Boolean).join(" "),
              }),
            ]
          : []),
      ];
    },
    notes: (_file, _extensions, _theme, saving) =>
      saving !== undefined && "trouble" in saving
        ? [`What this project uses could not be read: ${saving.trouble}`]
        : [],
    leftOut: (_extensions, _theme, saving) => answered(saving)?.plane_left_out ?? null,
  };
}

const SHARE_HINT =
  "Deprecated: read as Mode — commit and push carry over, local says nothing — only while neither file sets Mode. Set Mode instead.";

/**
 * Whether `[memory] share` is what the plane's mode is, as the end of its hint: ADR 0051's
 * marker for a Shared value something else overrides. Only a share that says something —
 * `commit` or `push` — can be in force or overridden; `local` says nothing either way.
 */
function shareMarker(file: Shown, saving: SavingInForce | undefined): string {
  if (saving === undefined) return "";
  if (saving.plane.from_share) return "In force as Mode.";
  const share = shown(valueAt(file, key("memory", "share")));
  if ((share === "commit" || share === "push") && saving.plane.mode.value !== null)
    return `Not in force — Mode from ${where(saving.plane.mode.source)} wins.`;
  return "";
}

/**
 * **Repos, in either section** (charter-app#300, ADR 0051): a row per repo — every one
 * `inventory/repos.json` catalogues, then every one only a file's `[repos]` names — with
 * `[repos.<name>]`'s keys, each marked as the Plane group's are.
 */
export function reposGroup(section: Section): Group {
  return {
    title: "Repos",
    note: "How each workspace repo is saved, by its name in inventory/repos.json. A repo's defaults are mode off and auto-save off: purlis saves no repo until its mode says how.",
    empty: (saving) =>
      saving !== undefined && "trouble" in saving
        ? `The repos and how each is saved could not be read: ${saving.trouble}`
        : "No repo is catalogued in inventory/repos.json or named by either file.",
    controls: (file, _extensions, _theme, saving) =>
      (answered(saving)?.repos ?? []).flatMap((repo) =>
        REPO_KEYS.map((one) => {
          const path = key("repos", repo.name, one.key);
          const it = repo[one.key as keyof typeof repo];
          const marker = typeof it === "string" ? null : decided(it, one, file, path, section);
          return saveControl(
            one,
            path,
            `${repo.name}: ${one.label}`,
            section,
            marker,
            typeof it === "string" ? undefined : it,
          );
        }),
      ),
    leftOut: (_extensions, _theme, saving) => answered(saving)?.repos_left_out ?? null,
  };
}

/** The harness kinds a profile may name — `profiles::KINDS`, in the registry's order. */
export const KINDS = ["claude", "opencode", "codex"] as const;

/** `charter.toml`, as `docs/plane-format.md` documents it. `[frame]` is the tmux frame's, which
 *  this charter does not have and nothing reads (the doctor says so): Edit as TOML has it. */
export const SHARED: Group[] = [
  {
    title: "General",
    controls: () => [
      pickAt(
        key("workspace", "default"),
        "Default workspace",
        "workspace",
        "The workspace a session lands in when nothing else selects one.",
      ),
      pickAt(
        key("persona", "default"),
        "Default persona",
        "persona",
        "The persona a chat starts as when nothing else names one.",
      ),
      pickAt(
        key("harness", "default"),
        "Default harness",
        "profile",
        "A built-in harness, or a profile charter.local.toml declares.",
      ),
      textAt(key("update", "channel"), "Update channel", {
        kind: "choice",
        choices: ["stable", "dev"],
      }),
      textAt(key("charter", "version"), "Version lock", {
        hint: "The purlis version this project is pinned to, as 1.2.3. Empty pins nothing.",
      }),
      textAt(key("plane", "worktrees"), "Branch folders", {
        hint: "Under the project or one folder beside it, such as ../charter.worktrees.",
      }),
    ],
  },
  {
    title: "Forges",
    note: "One block per [[forge]] in the file.",
    controls: (file) =>
      forgeBlocks(file).flatMap((at) => {
        const block = (name: string): SettingsStep[] => [
          { key: "forge" },
          { index: at },
          { key: name },
        ];
        // `group` wins over `owner` when a block has both; edit the one the block uses.
        const owner = valueAt(file, block("group")) !== undefined ? "group" : "owner";
        const n = at + 1;
        return [
          textAt(block("kind"), `Forge ${n}: kind`, {
            kind: "choice",
            choices: ["gitlab", "github"],
          }),
          textAt(block(owner), `Forge ${n}: ${owner}`),
          textAt(block("host"), `Forge ${n}: host`, {
            hint: "A bare host, with a port if it needs one. Empty is the kind's own.",
          }),
          listAt(block("exclude"), `Forge ${n}: repos never listed`, "One repo name per line."),
        ];
      }),
  },
  planeGroup("shared"),
  reposGroup("shared"),
  EXTENSIONS,
  themeGroup("not set — the window's own theme"),
];

/** The pick that says a workspace's colour is its own `#rrggbb` rather than a palette name. */
const CUSTOM = "custom";

/**
 * **A workspace's colour** (charter-app#281): `settings.theme.colour`, one of the eight the core
 * reads (`theme.ts`' `WORKSPACE_COLOURS`, as the engine hands them) or a custom `#rrggbb`, whose
 * hue is taken. A select, and a colour well beside it while the pick is custom.
 */
const COLOUR: Control = {
  ...textAt(key("theme", "colour"), "Colour", {
    kind: "colour",
    choices: [...WORKSPACE_COLOURS, CUSTOM],
    hint: "Tints this workspace's tab, its chat strip, the title bar's mark and the accent while it is in front, as the theme says. The terminal keeps the theme's own colours.",
  }),
  unset: "none — the theme as it is",
  labels: {
    ...Object.fromEntries(
      WORKSPACE_COLOURS.map((name) => [name, name[0].toUpperCase() + name.slice(1)]),
    ),
    [CUSTOM]: "Custom…",
  },
};

/** `charter.local.toml`: `[harness]`, `[plane]`'s save keys, `[repos]`, `[extensions]` and
 *  `[theme]`, which is all its readers read there. Its `[harness.<name>]` profiles are a
 *  collection, each with a page of its own (ST-4, `project.ts`). */
export const LOCAL: Group[] = [
  {
    title: "Harness",
    controls: () => [
      pickAt(
        key("harness", "default"),
        "Default profile",
        "profile",
        "The profile the new-chat picker starts on. Wins over charter.toml's.",
      ),
    ],
  },
  planeGroup("local"),
  reposGroup("local"),
  EXTENSIONS,
  themeGroup("not set — charter.toml's pick"),
];
