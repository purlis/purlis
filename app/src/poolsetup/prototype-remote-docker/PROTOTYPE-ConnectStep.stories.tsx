// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch `prototype/pool-setup-remote-docker`, #6265, ADR 0123 §6). Never
 * merges to main. Each story is one state of Connect, or of Where beside it; every one is live,
 * so its buttons walk on from there. Switch light and dark with the toolbar's Appearance.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ConnectPrototype, type State } from "./ConnectPrototype";
import type { FindingId } from "./data";

const meta = {
  title: "PROTOTYPE/Pool setup, another machine",
  component: ConnectPrototype,
  args: { start: {} },
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof ConnectPrototype>;

export default meta;
type Story = StoryObj<typeof meta>;

const at = (start: Partial<State>): Story => ({
  args: { start },
  render: ({ start: from }) => <ConnectPrototype key={JSON.stringify(from)} start={from} />,
});

const connect = (more: Partial<State>): Story => at({ screen: "connect", kind: "remote", ...more });

/** Start here and click through: every button walks on. */
export const ClickThrough = at({});

// --- Where ---
export const WhereDockerHere = at({ where: "docker-here" });
export const WhereNoDockerHere = at({ where: "no-docker", kind: "remote" });
export const WhereDockerPointsElsewhere = at({ where: "elsewhere" });

// --- The route and the machine ---
export const RouteSshPreselected = connect({ route: "ssh", machine: "build-box" });
export const RouteTlsApi = connect({
  route: "tls",
  machine: "typed",
  typed: "lab.example.internal:2376",
});
export const RouteDockerContext = connect({ route: "context", machine: "build-box-ssh" });
export const RoutePodmanConnection = connect({ route: "podman", machine: "nas" });
export const MachineTyped = connect({ route: "ssh", machine: "typed", typed: "sam@200.0.0.8" });
export const MachineTypedNotAnAddress = connect({ route: "ssh", machine: "typed", typed: "@@" });
export const PlainTcpRefused = connect({ route: "context", machine: "old-lab", phase: "refused" });

// --- First contact ---
export const AskingForTheKey = connect({ phase: "reading", hold: true });
export const FirstContactKeyYourSshKnows = connect({ phase: "confirm", known: true });
export const FirstContactNewKey = connect({ phase: "confirm", known: false });
export const FirstContactTlsAuthority = connect({
  route: "context",
  machine: "lab-tls",
  phase: "confirm",
});

// --- The engine check ---
export const CheckingDocker = connect({ phase: "checking", hold: true });

const finding = (id: FindingId): Story =>
  connect({
    phase: "found",
    finding: id,
    hold: true,
    ...(id.startsWith("tls-") ? { route: "context" as const, machine: "lab-tls" } : {}),
  });
export const FindsNoSuchName = finding("name");
export const FindsNoAnswer = finding("answers");
export const FindsSignInRefused = finding("sign-in");
export const FindsNoDockerRunning = finding("no-engine");
export const FindsAccountNotAllowed = finding("not-allowed");
export const FindsCertificateFromAnotherAuthority = finding("tls-ca");
export const FindsCertificateExpired = finding("tls-expired");
export const FindsCertificateForAnotherName = finding("tls-name");
export const FindsDockerCannotKeepRunnersApart = finding("daemon");
export const FindsNoImageForItsProcessor = finding("arch");
export const FindsNoRoom = finding("disk");
export const FindsCannotDownloadTheImage = finding("pull");

// --- The owned mark ---
export const EveryCheckPassedOwnedQuestion = connect({ phase: "owned" });

// --- The pool reopened ---
export const PinChangedMachineKey = at({ screen: "reopened", kind: "remote", reopened: "machine" });
export const PinChangedAuthority = at({ screen: "reopened", kind: "remote", reopened: "ca" });
export const PinChangedDockerEngine = at({
  screen: "reopened",
  kind: "remote",
  reopened: "engine",
});
export const ContextNowPointsElsewhere = at({
  screen: "reopened",
  kind: "remote",
  reopened: "context",
});
