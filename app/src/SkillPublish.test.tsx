// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The publish review and the export dialog** (#2781), at the window seam: nothing is sent until
 * the button is pressed, the press hands back what the page showed, and anything that stops it
 * keeps the button out of reach. The core is faked at its command boundary.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ExportDialog, SkillPublishPage } from "./SkillPublish";
import type { PublishReviewView, SkillLevel } from "./bindings";
import {
  EXPORT,
  FORK_TAKEN,
  GITHUB,
  LOCAL,
  REVIEW,
  STOPPED,
  THROUGH_A_NEW_FORK,
  THROUGH_THEIR_FORK,
} from "./skillPublish.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
});

const LEVEL: SkillLevel = { at: "project", local: false };

function core(
  review: PublishReviewView,
  done: unknown = { kind: "opened", url: "https://x.invalid/1" },
) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "skill_sources") return [GITHUB];
    if (cmd === "skill_publish_review") return review;
    if (cmd === "skill_publish") return done;
    if (cmd === "skill_export_review") return EXPORT;
    if (cmd === "skill_export") return "/tmp/deploy.zip";
    return null;
  });
  return asked;
}

const page = (over: Partial<Parameters<typeof SkillPublishPage>[0]> = {}) =>
  render(
    <SkillPublishPage
      plane="p"
      name="deploy"
      level={LEVEL}
      onCancel={vi.fn()}
      onExport={vi.fn()}
      onMove={vi.fn()}
      onSources={vi.fn()}
      {...over}
    />,
  );

const opener = () => screen.findByRole("button", { name: "Open draft pull request" });

describe("the publish review", () => {
  it("shows everything that goes and sends nothing until the button is pressed", async () => {
    const asked = core(REVIEW);
    page();

    await screen.findByText("deploy/scripts/release.sh");
    screen.getByText("deploy/.env");
    screen.getByText("purlis/publish/deploy-3fa91c07d2be");
    screen.getByText(/Ada Example/);
    screen.getByText("90%");
    expect(asked.some((one) => one.cmd === "skill_publish")).toBe(false);

    await userEvent.click(await opener());

    await screen.findByTestId("publish-opened");
    const sent = asked.find((one) => one.cmd === "skill_publish");
    expect(sent?.args).toMatchObject({ accepted: REVIEW.accepted, name: "deploy" });
  });

  it("says the grades go beside the skill's folder too, where an install review reads them (#4346)", async () => {
    core(REVIEW);
    page();

    const file = await screen.findByTestId("publish-grade-file");
    expect(file).toHaveTextContent(
      "They go in deploy.grades.json too, beside the skill's folder, where an install review reads them as your grades.",
    );
  });

  it("names no grade file for a skill with no grade (#4346)", async () => {
    core({ ...REVIEW, grades: [], grade_files: [] });
    page();

    await screen.findByText("Not graded.");
    expect(screen.queryByTestId("publish-grade-file")).toBeNull();
  });

  it.each([
    ["a secret", STOPPED],
    ["a skill kept on this machine", LOCAL],
    ["a repo of the person's at the fork's name that is not a fork", FORK_TAKEN],
  ])("keeps the button out of reach for %s", async (_what, review) => {
    const asked = core(review);
    page();

    const button = await opener();
    await waitFor(() => expect(button).toHaveAttribute("aria-disabled", "true"));
    await userEvent.click(button);

    expect(asked.some((one) => one.cmd === "skill_publish")).toBe(false);
  });

  it("says what git said when the push fails, and offers Export and another try", async () => {
    core(REVIEW, { kind: "cannot-push", why: "denied" });
    const onExport = vi.fn();
    page({ onExport });

    await userEvent.click(await opener());
    await screen.findByText(/git said: denied/);
    await userEvent.click(screen.getByRole("button", { name: "Export…" }));
    expect(onExport).toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await opener()).not.toHaveAttribute("aria-disabled", "true");
  });

  it("names the fork it will make and makes it only on the press", async () => {
    const asked = core(THROUGH_A_NEW_FORK);
    const onExport = vi.fn();
    page({ onExport });

    await screen.findByText(
      /GitHub says your account cannot push to this source, so publishing makes your fork ada\/skills/,
    );
    screen.getByText(/public: everyone can read what is sent/);
    screen.getByText("Made when you publish");
    expect(asked.some((one) => one.cmd === "skill_publish")).toBe(false);

    await userEvent.click(screen.getByRole("button", { name: "Export…" }));
    expect(onExport).toHaveBeenCalled();
    await userEvent.click(
      await screen.findByRole("button", { name: "Fork and open draft pull request" }),
    );

    await screen.findByTestId("publish-opened");
    const sent = asked.find((one) => one.cmd === "skill_publish");
    expect(sent?.args).toMatchObject({ accepted: THROUGH_A_NEW_FORK.accepted });
  });

  it("names the fork the person has, and says nothing is made", async () => {
    core(THROUGH_THEIR_FORK);
    page();

    await screen.findByText(/so the branch goes on your fork ada\/skills/);
    expect(screen.queryByText("Made when you publish")).toBeNull();
    expect(await opener()).not.toHaveAttribute("aria-disabled", "true");
  });

  it("says a failed push was to the fork", async () => {
    core(THROUGH_THEIR_FORK, { kind: "cannot-push", why: "denied" });
    page();

    await userEvent.click(await opener());

    await screen.findByText("Nothing was opened. git said: denied");
  });

  it("names a skill it refers to that is kept here, and offers no move of the wrong one", async () => {
    core({ ...REVIEW, local: ["notify"] });
    page();

    await screen.findByText(/notify is kept on this machine/);
    expect(screen.queryByRole("button", { name: "Move…" })).toBeNull();
  });

  it("says a folder is replaced, and a reference that is not there", async () => {
    core(REVIEW);
    page();

    await screen.findByText("Replaces what is there");
    expect(screen.getByTestId("publish-broken")).toHaveTextContent("gone");
  });
});

describe("the export dialog", () => {
  it("names the skills that come along and writes only when asked", async () => {
    const asked = core(REVIEW);
    const onExported = vi.fn();
    render(
      <ExportDialog
        plane="p"
        name="deploy"
        level={LEVEL}
        onClose={vi.fn()}
        onExported={onExported}
      />,
    );

    await screen.findByText(/and notify, the skill it refers to/);
    expect(asked.some((one) => one.cmd === "skill_export")).toBe(false);

    await userEvent.click(screen.getByRole("button", { name: "Choose where to save…" }));

    await waitFor(() => expect(onExported).toHaveBeenCalledWith("/tmp/deploy.zip"));
    const sent = asked.find((one) => one.cmd === "skill_export");
    expect(sent?.args).toMatchObject({ zip: true });
  });
});
