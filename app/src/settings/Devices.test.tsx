import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "../App";
import type { PendingOffer, RemoteStatus } from "../bindings";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";
import { forgetPairing, spaced, timeLeft } from "./devices";

/**
 * **Settings › You › Devices** (iOS design §4.2, §6.4), in the whole window: phone access on
 * and off with where it listens, the extra addresses, the paired phones and Unpair, Pair a
 * phone… with its QR code and the window's own Pair/Cancel on an offer, Stop pairing, and
 * the keep-awake setting — each reaching the core as the command it is, and the status read
 * again after it.
 */

vi.mock("../SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PHONE = "01J9PHONE00000000000000000";
const OLD = "01J9OLDPHONE00000000000000";
const QR =
  'purlis-pair:{"version":1,"host_device":"01J9DEVCE00000000000000000","host_key":"ab","addresses":["192.168.9.155:7777"],"code":"cd"}';

let status: RemoteStatus;
/** Every action command the window sent, with its arguments. */
let sent: { cmd: string; args: Record<string, unknown> }[];
/** The refusal the next action is answered with, if any. */
let refuse: string | undefined;
/** The phone offering to pair, as the next status read says it. */
let offer: PendingOffer | null;

function act(cmd: string, args: Record<string, unknown>): unknown {
  sent.push({ cmd, args });
  if (refuse !== undefined) throw refuse;
  if (cmd === "remote_set_on") {
    status.on = args.on as boolean;
    status.running = status.on;
    status.bound = status.on ? ["192.168.9.155:7777"] : [];
  }
  if (cmd === "remote_set_addresses") status.addresses = args.addresses as string[];
  if (cmd === "remote_set_keep_awake") status.keep_awake = args.on as boolean;
  if (cmd === "remote_unpair") status.peers = status.peers.filter((p) => p.device !== args.device);
  if (cmd === "remote_pair_start") return QR;
  if (cmd === "remote_pair_confirm") {
    status.peers.push({
      device: PHONE,
      name: "Noro's iPhone",
      fingerprint: "3f2a9c1e0b7d4455",
      paired: 1_760_000_000,
    });
    offer = null;
    return null;
  }
  if (cmd === "remote_pair_cancel") offer = null;
  if (cmd === "remote_pair_stop") return null;
  return structuredClone(status);
}

beforeEach(() => {
  forgetThisLaunch();
  forgetPairing();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/op/.config/charter/layout.json", found: false, document: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  status = {
    on: false,
    running: false,
    bound: [],
    port: 7777,
    addresses: [],
    device: "01J9DEVCE00000000000000000",
    fingerprint: null,
    peers: [
      { device: OLD, name: "Old phone", fingerprint: "0011223344556677", paired: 1_750_000_000 },
    ],
    keep_awake: false,
    pending: null,
  };
  sent = [];
  refuse = undefined;
  offer = null;
  mockIPC(
    (cmd, args) => {
      if (cmd === "plane_at_launch") return { plane: null, from: null, why: "no plane here" };
      if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
      if (cmd === "planes_to_restore") return { windows: [], dropped: [], gone: [] };
      if (cmd === "remote_status") return { ...structuredClone(status), pending: offer };
      if (cmd.startsWith("remote_")) return act(cmd, (args ?? {}) as Record<string, unknown>);
      return null;
    },
    { shouldMockEvents: true },
  );
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

/** Settings, at You › Devices, from the palette of a window with no project open. */
async function devices() {
  render(<App />);
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard("Settings…");
  await userEvent.keyboard("{Enter}");
  const nav = await screen.findByRole("navigation", { name: "Groups" });
  await userEvent.click(within(nav).getByRole("button", { name: "Devices" }));
  return screen.findByRole("region", { name: "Devices" });
}

const phones = () =>
  within(screen.getByRole("group", { name: "Paired phones" }))
    .queryAllByRole("listitem")
    .map((item) => item.textContent);

const AWAKE = /Keep this (Mac|machine) awake while chats are running/;

describe("You › Devices", () => {
  it("spaces a fingerprint in fours and counts a window down", () => {
    expect(spaced("3f2a9c1e0b7d4455")).toBe("3f2a 9c1e 0b7d 4455");
    expect(timeLeft(0, 0)).toBe("5:00");
    expect(timeLeft(0, 61_000)).toBe("3:59");
    expect(timeLeft(0, 600_000)).toBe("0:00");
  });

  it("lists the paired phones, says access is off, and holds keep-awake off", async () => {
    await devices();
    await waitFor(() => expect(phones()).toEqual(["Old phone0011 2233 4455 6677Unpair"]));
    expect(screen.getByRole("checkbox", { name: "Phone access" })).not.toBeChecked();
    expect(screen.getByText("Not listening.")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: AWAKE })).not.toBeChecked();
  });

  it("turns phone access on and says where it listens", async () => {
    await devices();
    await userEvent.click(await screen.findByRole("checkbox", { name: "Phone access" }));
    await waitFor(() =>
      expect(screen.getByText("Listening on 192.168.9.155:7777.")).toBeInTheDocument(),
    );
    expect(sent).toEqual([{ cmd: "remote_set_on", args: { on: true } }]);
  });

  it("says why access did not start, beside the switch", async () => {
    refuse = "no address could be listened on: 192.168.9.155:7777: address in use";
    await devices();
    await userEvent.click(await screen.findByRole("checkbox", { name: "Phone access" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("address in use");
  });

  it("keeps the extra addresses, one per line", async () => {
    await devices();
    const box = await screen.findByRole("textbox", { name: "Also listen on" });
    await userEvent.type(box, "100.64.0.9{Enter}10.0.0.5:7800");
    await userEvent.tab();
    await waitFor(() =>
      expect(sent).toEqual([
        { cmd: "remote_set_addresses", args: { addresses: ["100.64.0.9", "10.0.0.5:7800"] } },
      ]),
    );
  });

  it("pairs a phone: the QR code, the offer the window confirms, and the phone listed after", async () => {
    status.on = true;
    status.running = true;
    status.bound = ["192.168.9.155:7777"];
    await devices();
    await userEvent.click(await screen.findByRole("button", { name: "Pair a phone" }));
    expect(
      await screen.findByRole("img", { name: "Pairing code for your phone" }),
    ).toBeInTheDocument();
    // The countdown's line, not the row's help, which says the same without the clock.
    expect(screen.getByText(/The code works for 5 minutes, once: 5:0\d left/)).toBeInTheDocument();
    expect(sent.at(-1)).toEqual({ cmd: "remote_pair_start", args: {} });

    offer = { ticket: "7", device: PHONE, name: "Noro's iPhone", fingerprint: "3f2a9c1e0b7d4455" };
    const asking = await screen.findByRole(
      "alertdialog",
      { name: "Pair Noro's iPhone?" },
      { timeout: 3000 },
    );
    expect(asking).toHaveTextContent("3f2a 9c1e 0b7d 4455");
    await userEvent.click(within(asking).getByRole("button", { name: "Pair" }));

    await waitFor(() => expect(phones()).toContain("Noro's iPhone3f2a 9c1e 0b7d 4455Unpair"));
    expect(sent).toContainEqual({ cmd: "remote_pair_confirm", args: { ticket: "7" } });
    expect(
      screen.queryByRole("img", { name: "Pairing code for your phone" }),
    ).not.toBeInTheDocument();
  });

  it("cancels an offer, and stops a pairing", async () => {
    status.on = true;
    status.running = true;
    status.bound = ["192.168.9.155:7777"];
    await devices();
    await userEvent.click(await screen.findByRole("button", { name: "Pair a phone" }));
    await screen.findByRole("img", { name: "Pairing code for your phone" });
    offer = { ticket: "8", device: PHONE, name: "Noro's iPhone", fingerprint: "3f2a9c1e0b7d4455" };
    const asking = await screen.findByRole(
      "alertdialog",
      { name: "Pair Noro's iPhone?" },
      { timeout: 3000 },
    );
    await userEvent.click(within(asking).getByRole("button", { name: "Cancel" }));
    await waitFor(() =>
      expect(sent).toContainEqual({ cmd: "remote_pair_cancel", args: { ticket: "8" } }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Stop pairing" }));
    await waitFor(() => expect(sent).toContainEqual({ cmd: "remote_pair_stop", args: {} }));
    expect(
      screen.queryByRole("img", { name: "Pairing code for your phone" }),
    ).not.toBeInTheDocument();
  });

  it("refuses to pair while access is off, and says so", async () => {
    refuse = "turn on phone access first: nothing is listening";
    await devices();
    await userEvent.click(await screen.findByRole("button", { name: "Pair a phone" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("turn on phone access first");
  });

  it("unpairs a phone", async () => {
    await devices();
    await userEvent.click(await screen.findByRole("button", { name: "Unpair Old phone" }));
    await waitFor(() => expect(phones()).toEqual([]));
    expect(sent).toEqual([{ cmd: "remote_unpair", args: { device: OLD } }]);
  });

  it("keeps the machine awake while chats run, when asked", async () => {
    await devices();
    await userEvent.click(await screen.findByRole("checkbox", { name: AWAKE }));
    await waitFor(() =>
      expect(sent).toEqual([{ cmd: "remote_set_keep_awake", args: { on: true } }]),
    );
    expect(screen.getByRole("checkbox", { name: AWAKE })).toBeChecked();
  });
});
