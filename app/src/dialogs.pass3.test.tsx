import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks } from "@tauri-apps/api/mocks";
import { ChatAsk } from "./ChatAsk";
import { AskFirst } from "./ExtensionAction";
import { NewProject } from "./NewProject";

afterEach(() => {
  cleanup();
  clearMocks();
});

const none = () => undefined;

/**
 * **DS-8's third pass** (#1719, #630): the dialogs neither walk had read in full, each checked
 * against the answer bar's rule, the copy guide's one claim per sentence and the working states
 * (`docs/design-system.md`, `docs/ui-primitives.md`). One case per finding fixed here; the walk
 * itself, dialog by dialog, is in the lane's report.
 */

describe("New project's Advanced form", () => {
  it("ends in a Cancel of its own, after its act, as every form's row does", async () => {
    const onCancel = vi.fn();
    render(
      <NewProject
        making={false}
        opening={false}
        onCreate={none}
        onOpenRepo={none}
        onCancel={onCancel}
      />,
    );
    await userEvent.click(screen.getByText("Advanced"));
    const advanced = screen.getByText("Advanced").closest("details");
    if (advanced === null) throw new Error("no Advanced section");
    const buttons = within(advanced)
      .getAllByRole("button")
      .filter((button) => !button.getAttribute("aria-label")?.startsWith("Browse"))
      .map((button) => button.textContent);
    expect(buttons).toEqual(["Create project", "Cancel"]);

    const cancel = within(advanced).getByRole("button", { name: "Cancel" });
    expect(cancel).toHaveAttribute("type", "button");
    await userEvent.click(cancel);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});

describe("a chat Notice's question", () => {
  it("marks its act busy while it is carried out, as a Notice's fix is", () => {
    render(
      <ChatAsk
        title="Forget ide.1?"
        says="Its record is dropped."
        answer="Forget"
        busy
        onAnswer={none}
        onCancel={none}
      />,
    );
    const act = screen.getByRole("button", { name: "Forget" });
    expect(act).toBeDisabled();
    expect(act).toHaveAttribute("aria-busy", "true");
  });
});

describe("an extension's action that deletes", () => {
  it("says what it does one claim to a sentence", () => {
    render(
      <AskFirst
        extension="tidy"
        action={{ id: "sweep", title: "Sweep", deletes: true } as never}
        onRun={() => Promise.resolve(undefined)}
        onCancel={none}
      />,
    );
    const question = screen.getByRole("alertdialog", { name: "Run “Sweep” from tidy?" });
    expect(question).toHaveAccessibleDescription(
      "This action deletes. purlis asks before every action that deletes. The extension decides what it deletes. purlis does not stop it.",
    );
  });
});
