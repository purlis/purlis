// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, expect, it } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { answerReports, forgetReportConsent, UNANSWERED, useReportConsent } from "./reportAnswers";

/**
 * **Answers the core fails to keep** (#5683): `answerReports` is called and left (`void`) by
 * every control that answers, so a failed call is its trouble, never a rejection.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetReportConsent();
});

it("settles, holding the failure as its trouble, when the core fails to keep the answers", async () => {
  forgetReportConsent({ consent: UNANSWERED, known: true, trouble: undefined });
  mockIPC((cmd) => {
    if (cmd === "keep_report_consent") throw new Error("the settings file is read-only");
    return null;
  });
  const { result } = renderHook(() => useReportConsent());

  await act(() => expect(answerReports({ asked: true })).resolves.toBeUndefined());
  expect(result.current.trouble).toContain("the settings file is read-only");
});
