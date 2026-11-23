// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Answers, AnswersView, deviceSaid, stateSaid } from "./Answers";
import { AnswersNotices } from "../AnswersNotices";
import { SETTINGS_LINK, type SettingsLinkAsk } from "./links";
import {
  DENIED,
  HELLO_OFF,
  HELLO_ON,
  HOLDS,
  IMPORTED,
  IMPORT_FAILED,
  WAITING_FOR_CONFIRM,
} from "./answersFixture";

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/op/project";
const IDS = { id: "sealed-answers", labelledBy: undefined };

describe("the device id in Settings", () => {
  it("the_person_confirming_the_id_in_the_window_names_the_id_it_showed", async () => {
    const sent: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "answers_shown") return WAITING_FOR_CONFIRM;
      if (cmd === "confirm_device_id") {
        sent.push(args);
        return { state: "confirmed", id: WAITING_FOR_CONFIRM.device.id };
      }
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Answers plane={PLANE} ids={IDS} />);
    const confirm = await screen.findByRole("button", { name: "Confirm device id" });
    expect(screen.getByText(/isn't confirmed yet/)).toBeTruthy();
    fireEvent.click(confirm);
    await waitFor(() => expect(screen.getByText(/Rules can name it/)).toBeTruthy());
    expect(sent).toEqual([{ plane: PLANE, id: WAITING_FOR_CONFIRM.device.id }]);
    expect(screen.queryByRole("button", { name: "Confirm device id" })).toBeNull();
  });

  it("the_id_is_a_label_with_no_rename", () => {
    render(<AnswersView shown={WAITING_FOR_CONFIRM} busy={false} onConfirm={() => {}} onHello={() => {}} ids={IDS} />);
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByRole("button", { name: /rename/i })).toBeNull();
    expect(screen.getByText(/01JABCDEFGHJKMNPQRSTVWXYZ0/)).toBeTruthy();
  });

  it("an_unset_id_has_nothing_to_confirm_and_says_why", () => {
    render(<AnswersView shown={DENIED} busy={false} onConfirm={() => {}} onHello={() => {}} ids={IDS} />);
    expect(screen.queryByRole("button", { name: "Confirm device id" })).toBeNull();
    expect(screen.getByText(/no device id yet/)).toBeTruthy();
  });

  it("a_refusal_of_the_confirm_is_said", async () => {
    mockIPC((cmd) => {
      if (cmd === "answers_shown") return WAITING_FOR_CONFIRM;
      if (cmd === "confirm_device_id") throw "the device id is not the one that was shown";
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Answers plane={PLANE} ids={IDS} />);
    fireEvent.click(await screen.findByRole("button", { name: "Confirm device id" }));
    await screen.findByText(/not the one that was shown/);
    expect(screen.getByRole("button", { name: "Confirm device id" })).toBeTruthy();
  });

  it("says_each_state_in_its_own_words", () => {
    const said = [
      HOLDS,
      DENIED,
      { ...HOLDS, state: "no-keyring" },
      { ...HOLDS, state: "fresh" },
    ].map(stateSaid);
    expect(new Set(said).size).toBe(4);
    expect(deviceSaid(HOLDS)).toContain("Rules can name it");
  });
});

describe("Windows Hello at each start in Settings", () => {
  it("a_machine_without_windows_hello_offers_nothing", () => {
    render(<AnswersView shown={HOLDS} busy={false} onConfirm={() => {}} onHello={() => {}} ids={IDS} />);
    expect(screen.queryByRole("button", { name: /Windows Hello/ })).toBeNull();
  });

  it("with_the_opt_in_off_the_full_gap_is_declared_and_turning_it_on_is_a_press", async () => {
    const sent: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "answers_shown") return HELLO_OFF;
      if (cmd === "set_windows_hello") {
        sent.push(args);
        return HELLO_ON.hello;
      }
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Answers plane={PLANE} ids={IDS} />);
    const turnOn = await screen.findByRole("button", { name: "Turn on Windows Hello…" });
    expect(screen.getByText(/isn't asked for when purlis starts/)).toBeTruthy();
    expect(screen.getByText(/Declared gap: on Windows any program/)).toBeTruthy();
    fireEvent.click(turnOn);
    await screen.findByRole("button", { name: "Turn off Windows Hello…" });
    expect(sent).toEqual([{ on: true }]);
    expect(screen.getByText(/Declared gap, narrowed/)).toBeTruthy();
  });

  it("a_hello_that_does_not_confirm_is_said_and_leaves_the_opt_in_as_it_was", async () => {
    mockIPC((cmd) => {
      if (cmd === "answers_shown") return HELLO_ON;
      if (cmd === "set_windows_hello") throw "Windows Hello was not confirmed";
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Answers plane={PLANE} ids={IDS} />);
    fireEvent.click(await screen.findByRole("button", { name: "Turn off Windows Hello…" }));
    await screen.findByText(/was not confirmed/);
    expect(screen.getByRole("button", { name: "Turn off Windows Hello…" })).toBeTruthy();
  });
});

describe("the sealed answers' Notices", () => {
  it("imported_nevers_are_listed_in_a_notice_that_can_be_dismissed", async () => {
    mockIPC((cmd) => {
      if (cmd === "answers_shown") return IMPORTED;
      throw new Error(`unexpected ${cmd}`);
    });
    render(<AnswersNotices plane={PLANE} />);
    const notice = await waitFor(() => {
      const found = document.querySelector('[data-cause^="answers-imported"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(notice).toHaveTextContent("steward -> devops, qa -> prod");
    expect(notice).toHaveTextContent("safe to delete");
    fireEvent.click(screen.getByRole("button", { name: /Dismiss/ }));
    await waitFor(() =>
      expect(document.querySelector('[data-cause^="answers-imported"]')).toBeNull(),
    );
  });

  it("a_deny_state_is_a_notice_that_links_to_the_sealed_answers", async () => {
    mockIPC((cmd) => {
      if (cmd === "answers_shown") return DENIED;
      throw new Error(`unexpected ${cmd}`);
    });
    const links: SettingsLinkAsk[] = [];
    const hear = (event: Event) => links.push((event as CustomEvent<SettingsLinkAsk>).detail);
    window.addEventListener(SETTINGS_LINK, hear);
    render(<AnswersNotices plane={PLANE} />);
    const notice = await waitFor(() => {
      const found = document.querySelector('[data-cause^="answers-deny"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(notice).toHaveTextContent("nothing they allow holds");
    expect(notice).toHaveTextContent("anchored at /old/place");
    fireEvent.click(screen.getByRole("button", { name: "Show sealed answers" }));
    window.removeEventListener(SETTINGS_LINK, hear);
    expect(links[0]?.link).toEqual({
      group: "project.sandbox",
      setting: "project.sandbox.sealedAnswers",
    });
  });

  it("an_import_that_failed_says_so_beside_the_deny_state", async () => {
    mockIPC((cmd) => {
      if (cmd === "answers_shown") return IMPORT_FAILED;
      throw new Error(`unexpected ${cmd}`);
    });
    render(<AnswersNotices plane={PLANE} />);
    await waitFor(() =>
      expect(document.querySelector('[data-cause="answers-import-failed"]')).not.toBeNull(),
    );
    expect(document.querySelector('[data-cause^="answers-deny"]')).not.toBeNull();
  });

  it("answers_that_hold_say_nothing", async () => {
    mockIPC((cmd) => {
      if (cmd === "answers_shown") return HOLDS;
      throw new Error(`unexpected ${cmd}`);
    });
    render(<AnswersNotices plane={PLANE} />);
    await waitFor(() => expect(document.body).toBeTruthy());
    expect(document.querySelector("[data-cause]")).toBeNull();
  });
});
