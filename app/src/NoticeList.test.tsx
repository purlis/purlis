import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { Notice, NoticeList } from "./Notice";

/**
 * **A project's Notices, listed in its Inbox** (#1695, V91i): what stood under the tab strip,
 * every one listed with its ways out, the most important first. The summary of a time away
 * (#1514, #1551) leads the news, after only an Undo that lasts seconds and what the person just
 * did; trouble still comes first.
 */

afterEach(cleanup);

const listed = () =>
  [...document.querySelectorAll(".notice-list [data-cause]")].map((one) =>
    one.getAttribute("data-cause"),
  );

const news = (cause: string) => (
  <Notice key={cause} cause={cause} onDismiss={() => undefined}>
    {cause}
  </Notice>
);

describe("the Inbox's Notices", () => {
  it("lists every one, the summary of a time away before news that waits", () => {
    render(
      <NoticeList>
        {news("pin-dormant:able")}
        {news("chat-resumed:1:conv-a")}
        {news("away-summary")}
      </NoticeList>,
    );

    expect(listed()).toEqual(["away-summary", "pin-dormant:able", "chat-resumed:1:conv-a"]);
    expect(screen.queryByRole("button", { name: /more$/ })).toBeNull();
  });

  it("puts trouble first, then an Undo that lasts seconds, then the summary", () => {
    render(
      <NoticeList>
        {news("away-summary")}
        {news("memory-deleted")}
        <Notice cause="window-trouble" tone="trouble" onDismiss={() => undefined}>
          trouble
        </Notice>
      </NoticeList>,
    );

    expect(listed()).toEqual(["window-trouble", "memory-deleted", "away-summary"]);
  });

  it("counts what it lists for the status line, leaving out what the doctor's button counts", () => {
    const onCount = vi.fn();
    render(
      <NoticeList onCount={onCount}>
        {news("pin-dormant:able")}
        {news("doctor-finding:hooks")}
        {news("alert:front door")}
      </NoticeList>,
    );

    // Two the status line counts, of three it lists.
    expect(onCount).toHaveBeenLastCalledWith(2, 3);
  });

  it("says when an answer to the person's own press arrives, and not for one already listed", () => {
    const onAnswer = vi.fn();
    function Pressed() {
      const [forgot, setForgot] = useState(false);
      return (
        <NoticeList onAnswer={onAnswer}>
          {news("pin-dormant:able")}
          {!forgot && (
            <Notice
              cause="pin-dormant:baker"
              fixes={[{ label: "Forget", onPress: () => setForgot(true) }]}
            >
              baker is gone
            </Notice>
          )}
          {forgot && news("pin-forgotten:baker")}
        </NoticeList>
      );
    }
    render(<Pressed />);
    expect(onAnswer).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Forget" }));

    expect(onAnswer).toHaveBeenCalledTimes(1);
    expect(listed()).toEqual(["pin-forgotten:baker", "pin-dormant:able"]);
  });

  it("keeps the focus where it is when a new one arrives above it", () => {
    function Arriving() {
      const [more, setMore] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setMore(true)}>
            arrive
          </button>
          <NoticeList>
            {news("pin-dormant:able")}
            {more && (
              <Notice cause="window-trouble" tone="trouble" onDismiss={() => undefined}>
                trouble
              </Notice>
            )}
          </NoticeList>
        </>
      );
    }
    render(<Arriving />);
    const able = document.querySelector('[data-cause="pin-dormant:able"]') as HTMLElement;
    const theirs = within(able).getByRole("button", { name: "Dismiss" });
    theirs.focus();

    // `fireEvent`, not a person's click: the press must not move the focus itself.
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "arrive" }));
    });

    expect(listed()).toEqual(["window-trouble", "pin-dormant:able"]);
    expect(document.activeElement).toBe(theirs);
  });
});
