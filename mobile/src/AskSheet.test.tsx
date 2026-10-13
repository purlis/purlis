import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { AskSheet } from "./AskSheet.tsx";
import type { OpenAsk } from "./timeline.ts";

const ask: OpenAsk = {
  ask: "ask-1",
  summary: "Run the tests?",
  choices: [
    { id: "yes", label: "Yes", lasts: false },
    { id: "no", label: "No, thanks", lasts: false },
  ],
  elicitsSecret: false,
};

describe("AskSheet", () => {
  it("shows the summary and the choices in the source's order and words", () => {
    render(<AskSheet ask={ask} closed={null} connected onAnswer={vi.fn()} onDismiss={vi.fn()} />);
    expect(screen.getByText("Run the tests?")).toBeInTheDocument();
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Yes", "No, thanks"]);
  });

  it("answers with the ask and the choice", async () => {
    const onAnswer = vi.fn().mockResolvedValue(undefined);
    render(<AskSheet ask={ask} closed={null} connected onAnswer={onAnswer} onDismiss={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "No, thanks" }));
    expect(onAnswer).toHaveBeenCalledWith("ask-1", "no");
  });

  it("disables every choice while the desktop is away", () => {
    render(
      <AskSheet ask={ask} closed={null} connected={false} onAnswer={vi.fn()} onDismiss={vi.fn()} />,
    );
    for (const button of screen.getAllByRole("button")) expect(button).toBeDisabled();
  });

  it("sends a secret ask to the desktop", () => {
    render(
      <AskSheet
        ask={{ ...ask, elicitsSecret: true }}
        closed={null}
        connected
        onAnswer={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    expect(
      screen.getByText("This ask wants a secret. Answer it on the desktop."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Yes" })).toBeNull();
  });

  it("says who answered an ask that closed under it, and dismisses on OK", async () => {
    const onDismiss = vi.fn();
    render(
      <AskSheet
        ask={null}
        closed={{ ask: "ask-1", by: "desktop" }}
        connected
        onAnswer={vi.fn()}
        onDismiss={onDismiss}
      />,
    );
    expect(screen.getByText("Answered on the desktop")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "OK" }));
    expect(onDismiss).toHaveBeenCalled();
  });

  it("shows the refusal when the host says no", async () => {
    const onAnswer = vi.fn().mockRejectedValue(new Error("no such ask"));
    render(<AskSheet ask={ask} closed={null} connected onAnswer={onAnswer} onDismiss={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "Yes" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("no such ask");
  });
});
