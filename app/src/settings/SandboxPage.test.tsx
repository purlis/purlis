// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsTab } from "./SettingsTab";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";
import { forgetGroups } from "./links";
import { personaHostsNotes, whatAChatCanDo } from "./sandbox";
import { namedIn } from "./project";
import { sandboxCommandReturned } from "../sandboxAsked";
import type {
  SandboxPolicy,
  SandboxPreset,
  SandboxState,
  SettingsEdit,
  SettingsField,
  SettingsFile,
  SettingsWhich,
} from "../bindings";
import { querySelectNamed } from "../ui/select/select.testkit";

/**
 * **Settings › Project › Sandbox explains what a chat here can do** (#1340): one sentence at the
 * top, Internet access as the core's presets, each opening to its hosts, a status line in place
 * of the mode, and what chats can change and what is always protected, each with why. Every
 * preset and every host is the core's (`sandbox_state`): the presets here stand in for its table.
 */

const PLANE = "/home/dev/plane";

const AI: SandboxPreset = {
  word: "model-providers",
  title: "AI providers",
  hosts: ["api.anthropic.com", "api.openai.com"],
  widens_caches: false,
};
const CODE: SandboxPreset = {
  word: "forge",
  title: "Code hosting",
  hosts: ["github.com", "gitlab.com", "git.example.org"],
  widens_caches: false,
};
const PACKAGES: SandboxPreset = {
  word: "toolchains",
  title: "Package registries",
  hosts: ["registry.npmjs.org", "crates.io"],
  widens_caches: true,
};
const PRESETS = [AI, CODE, PACKAGES];

const ON = {
  on: true,
  projectHosts: 0,
  yourHosts: 0,
  folders: 0,
  certificateService: false,
};

describe("the sentence at the top", () => {
  const changes = "A chat here can change files in the folder it works in";
  const caches = `${changes} and in the project's package caches`;
  it.each<[string, readonly SandboxPreset[], string]>([
    ["none", [], `${changes}, and reach no host on the internet.`],
    ["AI providers", [AI], `${changes}, and reach AI providers.`],
    ["code hosting", [CODE], `${changes}, and reach code hosting.`],
    ["package registries", [PACKAGES], `${caches}, and reach package registries.`],
    [
      "AI providers and code hosting",
      [AI, CODE],
      `${changes}, and reach AI providers and code hosting.`,
    ],
    [
      "AI providers and package registries",
      [AI, PACKAGES],
      `${caches}, and reach AI providers and package registries.`,
    ],
    [
      "code hosting and package registries",
      [CODE, PACKAGES],
      `${caches}, and reach code hosting and package registries.`,
    ],
    [
      "every preset",
      PRESETS,
      `${caches}, and reach AI providers, code hosting and package registries.`,
    ],
  ])("says what a chat can do with %s on", (_, presets, said) => {
    expect(whatAChatCanDo({ ...ON, presets })).toBe(said);
  });

  it("counts the project's hosts and yours, and the certificate service once it is on", () => {
    expect(
      whatAChatCanDo({
        ...ON,
        presets: [AI],
        projectHosts: 2,
        yourHosts: 1,
        certificateService: true,
      }),
    ).toBe(
      "A chat here can change files in the folder it works in, and reach AI providers, 2 project hosts, 1 host of yours and the system's certificate service.",
    );
    expect(whatAChatCanDo({ ...ON, presets: [], projectHosts: 1, yourHosts: 2 })).toBe(
      "A chat here can change files in the folder it works in, and reach 1 project host and 2 hosts of yours.",
    );
  });

  it("counts the folders you allowed every chat here on this machine", () => {
    expect(whatAChatCanDo({ ...ON, presets: [PACKAGES], folders: 2 })).toBe(
      "A chat here can change files in the folder it works in, in the project's package caches and in 2 folders you allowed on this machine, and reach package registries.",
    );
    expect(whatAChatCanDo({ ...ON, presets: [], folders: 1 })).toBe(
      "A chat here can change files in the folder it works in and in 1 folder you allowed on this machine, and reach no host on the internet.",
    );
  });

  it("says a project without the sandbox limits nothing", () => {
    expect(whatAChatCanDo({ ...ON, on: false, presets: PRESETS })).toBe(
      "Chats here run without a sandbox, so a chat can change any file you can and reach any host.",
    );
  });
});

describe("the files' names in the window", () => {
  const PURLIS = { shared: "purlis.toml", local: "purlis.local.toml" };

  it("renames a file's name, and leaves a host or path that only holds it as written", () => {
    expect(
      namedIn(
        "charter.toml names charter.toml.example.com; /opt/mycharter.toml and x-charter.local.toml stay. See charter.local.toml.",
        PURLIS,
      ),
    ).toBe(
      "purlis.toml names charter.toml.example.com; /opt/mycharter.toml and x-charter.local.toml stay. See purlis.local.toml.",
    );
    expect(namedIn("/home/dev/plane/charter.toml, which your team sees", PURLIS)).toBe(
      "/home/dev/plane/purlis.toml, which your team sees",
    );
  });

  it("says a sentence written with purlis's names as a project not yet renamed names its files", () => {
    const OLD = { shared: "charter.toml", local: "charter.local.toml" };
    expect(namedIn("one block per [[forge]] in purlis.toml; see purlis.local.toml.", OLD)).toBe(
      "one block per [[forge]] in charter.toml; see charter.local.toml.",
    );
    expect(namedIn("purlis.toml and charter.toml", PURLIS)).toBe("purlis.toml and purlis.toml");
  });
});

/** What a field of the Shared file holds. */
function field(path: string[], value: SettingsField["value"]): SettingsField {
  return { path: path.map((key) => ({ key })), value };
}

/** A file as the core lists it: its fields, and its hosts as the `hosts` collection. */
function fileOf(
  which: SettingsWhich,
  name: string,
  fields: SettingsField[],
  hosts: readonly string[] = [],
): SettingsFile {
  return {
    which,
    file: name,
    exists: true,
    text: "schema = 1\n",
    refusals: [],
    parsed: true,
    fields,
    entries: hosts.map((host, at) => ({
      collection: which === "shared" ? "hosts" : "myHosts",
      id: `host:${at}:${host}`,
      label: host,
      keys: [{ key: "sandbox" }, { key: "hosts" }],
      values: [
        { field: "host", value: host },
        ...(which === "local" ? [{ field: "confirmed", value: "yes" }] : []),
      ],
    })),
  };
}

const MODE_ON = field(["sandbox", "mode"], { kind: "text", value: "on" });

function state(more: Partial<SandboxState> = {}): SandboxState {
  return {
    on: true,
    offer: false,
    said: "no chat has started under this project's sandbox on this machine yet",
    never: [],
    hosts_changed: null,
    presets_changed: null,
    presets: PRESETS,
    persona_hosts: [],
    besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
    policy: null,
    ...more,
  };
}

/** The core, as the Sandbox page asks it: the two files and the sandbox's state. */
function core({
  shared = [MODE_ON],
  sharedName = "charter.toml",
  localName = "charter.local.toml",
  sandbox = state(),
  hosts = [] as string[],
}: {
  shared?: SettingsField[];
  sharedName?: string;
  localName?: string;
  sandbox?: SandboxState | null;
  hosts?: string[];
} = {}) {
  const sent: { which: SettingsWhich; edits: SettingsEdit[] }[] = [];
  const allowed: unknown[] = [];
  /** What `sandbox_state` answers from now on: the state after a change elsewhere. */
  const now = { sandbox };
  const files: Record<SettingsWhich, SettingsFile> = {
    shared: fileOf("shared", sharedName, shared, hosts),
    local: fileOf("local", localName, []),
  };
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "project_settings":
          return files;
        case "project_extensions":
          return { extensions: [], local_left_out: null };
        case "project_harness_plugins":
        case "extensions_on":
        case "sandbox_grants":
          return [];
        case "grantable_folders":
          return { folders: [], dropped: [] };
        case "sandbox_network":
          return { on: true, open: [], blocked: [] };
        case "project_theme":
        case "project_theme_drawn":
          return null;
        case "sandbox_state":
          return now.sandbox;
        case "allow_persona_hosts":
          allowed.push(given);
          return sandbox;
        case "allow_project_expects":
          allowed.push({ ...given, expects: true });
          // What waits is kept, so nothing waits when the state is read again.
          now.sandbox = sandbox === null ? null : { ...sandbox, expects: null };
          return now.sandbox;
        case "save_project_settings": {
          const which = given.which as SettingsWhich;
          const change = given.change as { kind: "edits"; edits: SettingsEdit[] };
          sent.push({ which, edits: change.edits });
          const kept = files[which].fields.filter(
            (one) =>
              !change.edits.some((edit) => JSON.stringify(edit.path) === JSON.stringify(one.path)),
          );
          const added = change.edits.flatMap((edit) =>
            edit.value === null ? [] : [{ path: edit.path, value: edit.value }],
          );
          files[which] = { ...files[which], fields: [...kept, ...added] };
          return { kind: "saved", file: files[which] };
        }
      }
      return undefined;
    },
    { shouldMockEvents: true },
  );
  return { sent, allowed, now };
}

beforeEach(() => {
  forgetThisLaunch();
  forgetGroups();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/dev/layout.json", found: false, document: null, trouble: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

async function atSandbox(): Promise<HTMLElement> {
  render(<SettingsTab plane={PLANE} level="project" />);
  await userEvent.click(
    await within(await screen.findByRole("navigation", { name: "Groups" })).findByRole("button", {
      name: "Sandbox",
    }),
  );
  return screen.findByRole("region", { name: "Sandbox" });
}

describe("Settings › Project › Sandbox", () => {
  it("opens on the sentence, and it follows a preset as it is turned off", async () => {
    const { sent } = core();
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        "A chat here can change files in the folder it works in and in the project's package caches, and reach AI providers, code hosting and package registries.",
      ),
    );

    await userEvent.click(within(page).getByRole("checkbox", { name: "Package registries" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].edits).toEqual([
      {
        path: [{ key: "sandbox" }, { key: "egress" }],
        value: { kind: "list", value: ["model-providers", "forge"] },
      },
    ]);
    await waitFor(() =>
      expect(page).toHaveTextContent(
        "A chat here can change files in the folder it works in, and reach AI providers and code hosting.",
      ),
    );
  });

  it("ticks the presets the file names, and writes none as no host rather than every preset", async () => {
    const { sent } = core({
      shared: [MODE_ON, field(["sandbox", "egress"], { kind: "list", value: ["forge"] })],
    });
    const page = await atSandbox();
    const access = await within(page).findByRole("group", { name: "Internet access" });

    expect(within(access).getByRole("checkbox", { name: "AI providers" })).not.toBeChecked();
    expect(within(access).getByRole("checkbox", { name: "Code hosting" })).toBeChecked();
    expect(within(access).getByRole("checkbox", { name: "Package registries" })).not.toBeChecked();

    await userEvent.click(within(access).getByRole("checkbox", { name: "Code hosting" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].edits[0].value).toEqual({ kind: "list", value: [] });
  });

  it("says a reset of Internet access reaches every preset, before it is pressed (#1197)", async () => {
    const { sent } = core({
      shared: [MODE_ON, field(["sandbox", "egress"], { kind: "list", value: ["forge"] })],
    });
    const page = await atSandbox();
    const access = await within(page).findByRole("group", { name: "Internet access" });
    expect(access).toHaveAccessibleDescription(
      expect.stringContaining(
        "Reset, or an Undo that takes this out of the file, turns every preset on.",
      ),
    );

    await userEvent.click(within(page).getByRole("button", { name: "Reset: reach every preset" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].edits[0].path).toEqual([{ key: "sandbox" }, { key: "egress" }]);
    expect(sent[0].edits[0].value ?? null).toBeNull();
  });

  it("lists each preset's hosts as the core gives them, behind a disclosure", async () => {
    core();
    const page = await atSandbox();
    const access = await within(page).findByRole("group", { name: "Internet access" });

    const code = within(access).getByText("3 hosts", { selector: "summary" });
    const hosts = code.closest("details");
    expect(hosts).not.toBeNull();
    expect(
      within(hosts as HTMLElement)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual(["github.com", "gitlab.com", "git.example.org"]);
  });

  it("shows the mode as a status, with the way to your own level", async () => {
    core();
    const page = await atSandbox();

    const status = await within(page).findByRole("group", { name: "Sandbox" });
    expect(status).toHaveTextContent("On for everyone in this project.");
    // Where a person turns it down for their own chats (#2071).
    expect(within(status).getByRole("button", { name: "Change your level" })).toBeInTheDocument();
    expect(querySelectNamed("Sandbox mode", page)).toBeNull();
    expect(within(page).queryByRole("button", { name: "Reset" })).toBeNull();
  });

  it("turns the sandbox on from its status line, with no way back from here", async () => {
    const { sent } = core({ shared: [], sandbox: state({ on: false, said: null }) });
    const page = await atSandbox();

    expect(page).toHaveTextContent(
      "Chats here run without a sandbox, so a chat can change any file you can and reach any host.",
    );
    await userEvent.click(within(page).getByRole("button", { name: "Turn the sandbox on" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].edits).toEqual([
      { path: [{ key: "sandbox" }, { key: "mode" }], value: { kind: "text", value: "on" } },
    ]);
    await waitFor(() =>
      expect(within(page).getByRole("group", { name: "Sandbox" })).toHaveTextContent(
        "On for everyone in this project.",
      ),
    );
    expect(within(page).queryByRole("button", { name: "Undo" })).toBeNull();
    expect(within(page).queryByRole("button", { name: "Turn the sandbox on" })).toBeNull();
  });

  // R1 (2026-10-13): on by default, so the one thing the committed file ever says is `false`.
  it("draws certificate checks on while the file does not set them, and writes false to turn them off", async () => {
    const { sent } = core();
    const page = await atSandbox();
    const checks = within(page).getByRole("switch", { name: "Certificate checks" });
    expect(checks).toBeChecked();
    // What it does and what it leaves open, under the one control.
    expect(page).toHaveTextContent(
      "Lets tools such as gh, glab and other Go programs verify a host's certificate",
    );
    expect(page).toHaveTextContent("those do not go through the sandbox's proxy");

    await userEvent.click(checks);

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].edits).toEqual([
      {
        path: [{ key: "sandbox" }, { key: "certificate-checks" }],
        value: { kind: "bool", value: false },
      },
    ]);
  });

  it("draws a project that turned certificate checks off as off, and no row beside it grants anything", async () => {
    core({
      shared: [MODE_ON, field(["sandbox", "certificate-checks"], { kind: "bool", value: false })],
      sandbox: state({ certificate_check: { on: false, locked: null, widens: true } }),
    });
    const page = await atSandbox();

    expect(within(page).getByRole("switch", { name: "Certificate checks" })).not.toBeChecked();
    // One control for it: no per-machine row, and nothing to allow.
    expect(within(page).queryByText("Certificate checks on this machine")).toBeNull();
    expect(within(page).queryByRole("button", { name: "Allow on this machine" })).toBeNull();
    expect(page).not.toHaveTextContent("the system's certificate service.");
  });

  it("says who locked certificate checks off, with nothing to turn on", async () => {
    const { sent } = core({
      sandbox: state({
        certificate_check: {
          on: false,
          locked:
            "Policy turns certificate checks off on this machine. Locked by policy, set by IT in /etc/purlis/policy.json.",
          widens: true,
        },
      }),
    });
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        "Off. Policy turns certificate checks off on this machine. Locked by policy, set by IT in /etc/purlis/policy.json.",
      ),
    );
    expect(within(page).queryByRole("switch", { name: "Certificate checks" })).toBeNull();
    expect(page).not.toHaveTextContent("the system's certificate service.");
    expect(sent).toEqual([]);
  });

  // ADR 0093, ruling 2: the committed file declares and never grants, so the page says what
  // this machine has not allowed and allows it with the window's own call, after a Not now too.
  it("lists what the project expects with each item's own words, and allows it on this machine", async () => {
    const { allowed, sent } = core({
      sandbox: state({
        expects: {
          asked: false,
          locked: null,
          items: [
            {
              what: "socket",
              target: "/Users/op/.docker/run/docker.sock",
              named: "/Users/op/.docker/run/docker.sock",
              says: "Allowing it lets this chat control containers, which can reach your whole machine: about the same as no sandbox.",
              refused: null,
            },
            {
              what: "service",
              target: "com.example.helper",
              named: "com.example.helper",
              says: "It is a service on this machine; purlis can't tell what it does.",
              refused: null,
            },
          ],
        },
      }),
    });
    const page = await atSandbox();
    const here = await within(page).findByRole("group", { name: "This project expects" });
    expect(here).toHaveTextContent("/Users/op/.docker/run/docker.sock");
    expect(here).toHaveTextContent("about the same as no sandbox");
    expect(here).toHaveTextContent("com.example.helper");

    // The same answer as the project's Notice gives, in the same grant's look (ADR 0097 §6).
    expect(within(here).getByRole("button", { name: "Allow on this machine" })).toHaveAttribute(
      "data-variant",
      "grants",
    );
    await userEvent.click(within(here).getByRole("button", { name: "Allow on this machine" }));

    await waitFor(() =>
      expect(allowed).toEqual([
        {
          plane: PLANE,
          shown: [
            {
              target: "/Users/op/.docker/run/docker.sock",
              named: "/Users/op/.docker/run/docker.sock",
            },
            { target: "com.example.helper", named: "com.example.helper" },
          ],
          expects: true,
        },
      ]),
    );
    await waitFor(() => expect(here).toHaveTextContent("Allowed on this machine"));
    expect(within(here).queryByRole("button", { name: "Allow on this machine" })).toBeNull();
    // This machine's file, never the committed one.
    expect(sent).toEqual([]);
  });

  // #1888: an Allow made elsewhere (the project's Notice) left this row offering the same Allow.
  it("reads what the project expects again when an Allow elsewhere returns", async () => {
    const { now } = core({
      sandbox: state({
        expects: {
          asked: true,
          locked: null,
          items: [
            {
              what: "service",
              target: "com.example.helper",
              named: "com.example.helper",
              says: "It is a service on this machine; purlis can't tell what it does.",
              refused: null,
            },
          ],
        },
      }),
    });
    const here = await within(await atSandbox()).findByRole("group", {
      name: "This project expects",
    });
    expect(within(here).getByRole("button", { name: "Allow on this machine" })).toBeVisible();

    now.sandbox = state({ expects: null });
    act(() => sandboxCommandReturned());

    await waitFor(() =>
      expect(within(here).queryByRole("button", { name: "Allow on this machine" })).toBeNull(),
    );
    expect(here).not.toHaveTextContent("com.example.helper");
    expect(here).toHaveTextContent("This machine allows all of it now.");
  });

  it("says who locked what the project expects, and why one cannot be allowed here, with no Allow", async () => {
    core({
      sandbox: state({
        expects: {
          asked: true,
          locked:
            "Policy forbids allowing a chat a local socket or a system service. Locked by policy, set by IT in /etc/purlis/policy.json.",
          items: [
            {
              what: "socket",
              target: "/var/run/docker.sock",
              named: "/var/run/docker.sock",
              says: "Allowing it lets this chat control containers, which can reach your whole machine: about the same as no sandbox.",
              refused:
                "That is not a local socket on this machine now, so there is nothing to allow.",
            },
          ],
        },
      }),
    });
    const here = await within(await atSandbox()).findByRole("group", {
      name: "This project expects",
    });
    expect(here).toHaveTextContent("set by IT");
    expect(here).toHaveTextContent("It cannot be allowed here now");
    expect(within(here).queryByRole("button")).toBeNull();
  });

  it("draws no row where the project expects nothing more than this machine allows", async () => {
    core();
    const page = await atSandbox();
    expect(within(page).queryByText("This project expects")).toBeNull();
  });

  it("names every value a persona's tool is handed", async () => {
    // T-1: a tool handed several values, each named with its variable, key and hand-over.
    const CURL =
      'curl with "kibana-password" as KIBANA_PASS file and "kibana-username" as KIBANA_USER env from vault elastic-logs-master';
    const KUBECTL = "kubectl with kubeconfig from vault devops as KUBECONFIG file";
    core({
      sandbox: state({
        persona_hosts: [
          {
            persona: "logs",
            hosts: [],
            reached: [],
            tools: [CURL, KUBECTL],
            digest: "d1",
            allowed: false,
            default: false,
            waiting: false,
          },
        ],
      }),
      sharedName: "purlis.toml",
      localName: "purlis.local.toml",
    });
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        `A chat as logs may run ${CURL}; and ${KUBECTL} only once you allow them on this machine, below.`,
      ),
    );
  });

  it("says beside a general-purpose tool's Allow that it can show its values, and nothing beside an unlisted one's", async () => {
    // T-4: the core's sentence, said beside the line allowed.
    const CURL = "curl with api-token from vault api as API_TOKEN env";
    const AWS = "aws with aws-token from vault cloud as AWS_TOKEN env";
    const SAID =
      "curl can show the values it is handed in the chat's output; allow it only for a persona you trust with them.";
    const waiting = (persona: string, tool: string, cautions: string[] | null) => ({
      persona,
      hosts: [],
      reached: [],
      tools: [tool],
      digest: "d1",
      allowed: false,
      default: false,
      waiting: false,
      cautions,
    });
    core({
      sandbox: state({
        persona_hosts: [waiting("logs", CURL, [SAID]), waiting("cloud", AWS, null)],
      }),
      sharedName: "purlis.toml",
      localName: "purlis.local.toml",
    });
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        `A chat as logs may run ${CURL} only once you allow them on this machine, below. ${SAID}`,
      ),
    );
    expect(page).toHaveTextContent(
      `A chat as cloud may run ${AWS} only once you allow them on this machine, below.`,
    );
    expect(page).not.toHaveTextContent(
      `A chat as cloud may run ${AWS} only once you allow them on this machine, below. curl`,
    );
    expect(page).not.toHaveTextContent("aws can show the values");
  });

  it("says why a tool runs nothing where the program it runs is one no tool may be", () => {
    const REFUSED =
      "devops's tool fetch runs nothing: fetch runs whatever it is told, so it would hand the vault's credential to anything — a tool is the one program that needs it";
    const devops = {
      persona: "devops",
      hosts: [],
      reached: [],
      tools: [],
      digest: "d1",
      allowed: false,
      default: false,
      waiting: false,
      refused: [REFUSED],
    };
    expect(personaHostsNotes(state({ persona_hosts: [devops] }))).toEqual([`${REFUSED}.`]);
    expect(personaHostsNotes(state({ persona_hosts: [{ ...devops, refused: null }] }))).toEqual([]);
  });

  it("names a persona's own hosts", async () => {
    const { allowed } = core({
      sandbox: state({
        persona_hosts: [
          {
            persona: "devops",
            hosts: ["10.0.0.5:6443", "charter.toml.example.com"],
            reached: ["10.0.0.5:6443", "charter.toml.example.com"],
            tools: [],
            digest: "d1",
            allowed: true,
            default: false,
            waiting: false,
          },
          {
            persona: "qa",
            hosts: ["qa.example"],
            reached: ["qa.example"],
            tools: [],
            digest: "d2",
            allowed: false,
            default: false,
            waiting: false,
          },
        ],
      }),
      sharedName: "purlis.toml",
      localName: "purlis.local.toml",
    });
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        "A chat as devops also reaches 10.0.0.5:6443 and charter.toml.example.com.",
      ),
    );
    // #1362, D-1362-7: a persona's hosts reach nothing here until they are allowed.
    expect(page).toHaveTextContent(
      "A chat as qa reaches qa.example only once you allow them on this machine, below.",
    );
    expect(page).toHaveTextContent(
      "A chat that names no persona reaches its default persona's hosts, once they are allowed on this machine. A chat opened by a handoff may hold the asking chat's hosts, and a Resume the default persona's, until you allow its own on its tab.",
    );
    // Settings has the same Allow as the project's notice: the same call, the same digest.
    await userEvent.click(within(page).getByRole("button", { name: "Allow for qa chats" }));
    await waitFor(() =>
      expect(allowed).toContainEqual({ plane: PLANE, persona: "qa", digest: "d2" }),
    );
    expect(
      await within(page).findByText(
        "Allowed on this machine. Chats started after you allow them reach them; a chat already running takes them when it restarts.",
      ),
    ).toBeVisible();
    expect(within(page).queryByRole("button", { name: "Allow for devops chats" })).toBeNull();
    // The persona stays in its own tab, and its row links to it (#1388).
    expect(within(page).getByRole("button", { name: "Show qa" })).toBeVisible();
  });

  it("says a change applies to a chat from its next start, and how to restart one", async () => {
    core();
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        "A change here applies to a chat from its next start. A chat that is running keeps the sandbox it started with until you restart it, with Restart chat on its tab's menu.",
      ),
    );
  });

  it("says a chat can read what you can, so no login of yours reads as hidden", async () => {
    core();
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        "A chat can read any file you can, except vaults and purlis's own keys, so keep secrets in a vault.",
      ),
    );
    const kept = await within(page).findByRole("list", { name: "Always protected" });
    expect(kept).toHaveTextContent("Your approvals in purlis");
    expect(kept).not.toHaveTextContent(/sign-in/i);
  });

  it("counts what the core grants: the project's hosts, yours and your folders", async () => {
    core({
      hosts: ["api.example.com", "10.0.0.5"],
      sandbox: state({ besides: { project_hosts: 1, your_hosts: 0, folders: 1 } }),
    });
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        "A chat here can change files in the folder it works in, in the project's package caches and in 1 folder you allowed on this machine, and reach AI providers, code hosting, package registries and 1 project host.",
      ),
    );
  });

  it("says it is reading, not that a chat reaches nothing, until the core answers", async () => {
    core({ sandbox: null });
    const page = await atSandbox();

    await waitFor(() => expect(page).toHaveTextContent("Reading this project's sandbox…"));
    expect(page).not.toHaveTextContent("no host on the internet");
  });

  it("names the certificate service only on a Mac, where the setting widens anything", async () => {
    core({ sandbox: state({ certificate_check: { on: true, locked: null, widens: false } }) });
    const page = await atSandbox();

    await waitFor(() => expect(page).toHaveTextContent("A chat here can change files"));
    expect(page).not.toHaveTextContent("the system's certificate service.");
    cleanup();

    // On a Mac, a default project's chats reach it, and the sentence at the top says so.
    core({ sandbox: state({ certificate_check: { on: true, locked: null, widens: true } }) });
    const mac = await atSandbox();
    await waitFor(() => expect(mac).toHaveTextContent("the system's certificate service."));
  });

  it("is found by searching for what it explains", async () => {
    core();
    render(<SettingsTab plane={PLANE} level="project" />);
    await userEvent.type(await screen.findByRole("searchbox"), "Always protected");

    const page = await screen.findByRole("region", { name: "Sandbox" });
    expect(within(page).getByRole("list", { name: "Always protected" })).toBeInTheDocument();
  });

  it("lists what chats can change and what is always protected, each with why", async () => {
    core();
    const page = await atSandbox();

    const changes = await within(page).findByRole("list", { name: "What chats can change" });
    expect(within(changes).getAllByRole("listitem").length).toBeGreaterThan(3);
    expect(changes).toHaveTextContent("The project's package caches");
    const kept = within(page).getByRole("list", { name: "Always protected" });
    expect(kept).toHaveTextContent("Vaults");
    expect(kept).toHaveTextContent("charter.toml and charter.local.toml");
    for (const item of within(kept).getAllByRole("listitem"))
      expect(item.querySelector(".sandbox-why")?.textContent).toMatch(/^[A-Z].+\.$/);
  });

  it("leaves the package caches off what chats can change while Package registries is off", async () => {
    core({ shared: [MODE_ON, field(["sandbox", "egress"], { kind: "list", value: ["forge"] })] });
    const page = await atSandbox();

    const changes = await within(page).findByRole("list", { name: "What chats can change" });
    expect(changes).not.toHaveTextContent("package caches");
  });

  it("names the files the project uses, under their purlis names too", async () => {
    core({ sharedName: "purlis.toml", localName: "purlis.local.toml" });
    const page = await atSandbox();

    await waitFor(() => expect(page).toHaveTextContent("Kept in purlis.toml"));
    expect(page).toHaveTextContent("purlis.toml and purlis.local.toml");
    expect(page).not.toHaveTextContent("charter.toml");
  });

  it("names the files the project uses on every Project group", async () => {
    core({ sharedName: "purlis.toml", localName: "purlis.local.toml" });
    render(<SettingsTab plane={PLANE} level="project" />);
    const nav = await screen.findByRole("navigation", { name: "Groups" });
    for (const name of ["Forges", "Saving", "General"]) {
      await userEvent.click(await within(nav).findByRole("button", { name }));
      const group = await screen.findByRole("region", { name });
      expect(group, name).not.toHaveTextContent("charter.toml");
      expect(group, name).not.toHaveTextContent("charter.local.toml");
    }
  });

  it("starts no sentence on the page with a lowercase letter", async () => {
    core();
    const page = await atSandbox();
    await within(page).findByRole("list", { name: "Always protected" });

    // Each piece of text the page draws, by the element that holds it. A host or a file name is
    // in code font and what a select holds is a value: neither is a sentence.
    const texts = [...page.querySelectorAll("p, span, label, li, summary, h3, button, div")]
      .filter((one) => one.closest('code, [role="combobox"]') === null)
      .map((one) =>
        [...one.childNodes]
          .filter((node) => node.nodeType === Node.TEXT_NODE)
          .map((node) => node.textContent)
          .join("")
          .trim(),
      )
      .filter((one) => one.length > 0);
    expect(texts.length).toBeGreaterThan(10);
    const sentences = texts.flatMap((text) => text.split(/(?<=\.)\s+/));
    for (const sentence of sentences) expect(sentence, sentence).toMatch(/^[^a-z]/);
  });
});

describe("values an administrator's policy locks (#1343)", () => {
  const LOCKED = "Locked by policy, set by Platform team in /etc/purlis/policy.json.";
  const policy = (more: Partial<SandboxPolicy> = {}): SandboxPolicy => ({
    locked_by: LOCKED,
    presets: null,
    hosts: null,
    personal_hosts: false,
    persona_hosts: false,
    opt_out: false,
    required: null,
    write_grants: false,
    ...more,
  });
  const REQUIRED = "On, required by policy, set by Platform team in /etc/purlis/policy.json.";

  it("shows locked presets as locked, with who set them, and no box to tick", async () => {
    const { sent } = core({ sandbox: state({ policy: policy({ presets: ["model-providers"] }) }) });
    const page = await atSandbox();

    const access = await within(page).findByRole("group", { name: "Internet access" });
    expect(access).toHaveTextContent(`AI providers is on. ${LOCKED}`);
    expect(within(page).queryByRole("checkbox")).toBeNull();
    expect(within(access).queryByRole("button")).toBeNull();
    // The strictest wins in the sentence too: only what policy allows is reached.
    await waitFor(() =>
      expect(page).toHaveTextContent(
        "A chat here can change files in the folder it works in, and reach AI providers.",
      ),
    );
    expect(sent).toEqual([]);
  });

  it("says the sandbox is on and required by policy on the mode's status line (#1423)", async () => {
    core({ sandbox: state({ policy: policy({ opt_out: true, required: REQUIRED }) }) });
    const page = await atSandbox();

    const status = await within(page).findByRole("group", { name: "Sandbox" });
    expect(status).toHaveTextContent(REQUIRED);
    expect(status).toHaveTextContent("Counted on this machine only, and never sent.");
    expect(status).not.toHaveTextContent("use that chat's tab");
    // Nothing turns it on or off here; your own level is still yours to see (#2071).
    expect(within(status).queryByRole("button", { name: "Turn the sandbox on" })).toBeNull();
  });

  it("shows a project that has not turned the sandbox on as on, required by policy, with no control (#1423)", async () => {
    // No `mode` in the committed file: policy turns the sandbox on here all the same.
    const { sent } = core({
      shared: [],
      sandbox: state({ said: null, policy: policy({ opt_out: true, required: REQUIRED }) }),
    });
    const page = await atSandbox();

    const status = await within(page).findByRole("group", { name: "Sandbox" });
    expect(status).toHaveTextContent(REQUIRED);
    expect(status).not.toHaveTextContent("Not turned on in this project");
    expect(within(status).queryByRole("button", { name: "Turn the sandbox on" })).toBeNull();
    // The page says what a chat here reaches, as for any sandboxed project.
    await waitFor(() =>
      expect(page).toHaveTextContent(
        "A chat here can change files in the folder it works in and in the project's package caches, and reach AI providers, code hosting and package registries.",
      ),
    );
    expect(page).not.toHaveTextContent("Chats here run without a sandbox");
    expect(page).not.toHaveTextContent("Once the sandbox is on.");
    expect(sent).toEqual([]);
  });

  it("says a project whose manifest is gone refuses its chats, never that the sandbox is off (#1411)", async () => {
    const GONE =
      "This project's charter.toml is missing, so purlis cannot tell whether chats here run sandboxed. Chats here are refused until the manifest is restored.";
    const { sent } = core({
      shared: [],
      sandbox: state({ on: false, said: null, manifest: GONE }),
    });
    const page = await atSandbox();

    const status = await within(page).findByRole("group", { name: "Sandbox" });
    expect(status).toHaveTextContent(GONE);
    expect(status).not.toHaveTextContent("Off in this project");
    // Turning it on would write a manifest in place of the gone one.
    expect(within(status).queryByRole("button", { name: "Turn the sandbox on" })).toBeNull();
    expect(page).not.toHaveTextContent("Chats here run without a sandbox");
    expect(sent).toEqual([]);
  });

  it("offers Turn the sandbox on where a policy requires nothing", async () => {
    core({ shared: [], sandbox: state({ on: false, said: null, policy: policy() }) });
    const page = await atSandbox();

    const status = await within(page).findByRole("group", { name: "Sandbox" });
    expect(status).toHaveTextContent(
      "Not turned on in this project, so chats here start at the floor.",
    );
    expect(within(status).getByRole("button", { name: "Turn the sandbox on" })).toBeInTheDocument();
  });

  it("says which hosts policy allows, and that a persona's own are locked out", async () => {
    core({
      sandbox: state({
        persona_hosts: [
          {
            persona: "devops",
            hosts: ["10.0.0.5:6443"],
            reached: [],
            tools: [],
            digest: "d",
            allowed: false,
            default: false,
            waiting: false,
          },
        ],
        policy: policy({ hosts: ["*.corp.example"], persona_hosts: true }),
      }),
    });
    const page = await atSandbox();

    await waitFor(() =>
      expect(page).toHaveTextContent(
        `Only these hosts may be added, by the project or its forges, you, a persona or a block's Allow: *.corp.example. Any other is not reached. ${LOCKED}`,
      ),
    );
    expect(page).toHaveTextContent(
      `A chat as devops reaches none of its own hosts (10.0.0.5:6443). ${LOCKED}`,
    );
    expect(page).not.toHaveTextContent("A chat as devops also reaches");
  });

  it("shows your own hosts as locked, with no Add", async () => {
    core({ sandbox: state({ policy: policy({ personal_hosts: true }) }) });
    render(<SettingsTab plane={PLANE} level="project" />);
    const nav = await screen.findByRole("navigation", { name: "Groups" });
    await userEvent.click(await within(nav).findByRole("button", { name: "Your hosts" }));
    const page = await screen.findByRole("region", { name: "Your hosts" });

    expect(page).toHaveTextContent(
      `Policy forbids hosts of your own, so none reaches a chat here. ${LOCKED}`,
    );
    expect(within(page).queryByRole("button", { name: /Add/ })).toBeNull();
    expect(within(page).queryByRole("textbox")).toBeNull();
  });

  it("shows the folders a block's Allow may name as locked where policy forbids write grants", async () => {
    core({ sandbox: state({ policy: policy({ write_grants: true }) }) });
    render(<SettingsTab plane={PLANE} level="project" />);
    const nav = await screen.findByRole("navigation", { name: "Groups" });
    await userEvent.click(await within(nav).findByRole("button", { name: "Network" }));
    const page = await screen.findByRole("region", { name: "Network" });

    await waitFor(() =>
      expect(page).toHaveTextContent(`Policy forbids allowing a chat to write a folder. ${LOCKED}`),
    );
    expect(within(page).queryByRole("button", { name: "Add folder" })).toBeNull();
  });

  it("locks nothing without a policy", async () => {
    core();
    const page = await atSandbox();
    await within(page).findByRole("group", { name: "Internet access" });
    expect(page).not.toHaveTextContent("Locked by policy");
    expect(within(page).getAllByRole("checkbox").length).toBe(3);
  });
});
