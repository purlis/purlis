// PROTOTYPE smoke check (#7997): every story renders, every control is named.
import { composeStories } from "@storybook/react-vite";
import { render, cleanup } from "@testing-library/react";
import { describe, expect, it, afterEach } from "vitest";
import * as stories from "./ReposView.prototype.stories";
import { namelessControls } from "./a11y.testkit";

const all = composeStories(stories);
afterEach(cleanup);
describe("Repos view prototype", () => {
  for (const [name, Story] of Object.entries(all))
    it(`draws ${name}`, () => {
      const { container } = render(<Story />);
      expect(container.textContent?.length).toBeGreaterThan(0);
      expect(namelessControls(container)).toEqual([]);
    });
});
