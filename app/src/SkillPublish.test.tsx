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
import { EXPORT, GITHUB, LOCAL, REVIEW, STOPPED } from "./skillPublish.testkit";

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

  it.each([
    ["a secret", STOPPED],
    ["a skill kept on this machine", LOCAL],
  ])("keeps the button out of reach for %s", async (_what, review) => {
    const asked = core(review);
    page();

    const button = await opener();
    await waitFor(() => expect(button).toHaveAttribute("aria-disabled", "true"));
    await userEvent.click(button);

    expect(asked.some((one) => one.cmd === "skill_publish")).toBe(false);
  });

  it("offers Export when the source refuses the push", async () => {
    core(REVIEW, { kind: "cannot-push", why: "denied" });
    const onExport = vi.fn();
    page({ onExport });

    await userEvent.click(await opener());
    await userEvent.click(await screen.findByRole("button", { name: "Export…" }));

    expect(onExport).toHaveBeenCalled();
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
