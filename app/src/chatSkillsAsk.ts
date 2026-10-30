// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **"Show this chat's skills"** (spec #3679 S8, #3694): the one way a chat's own surfaces reach
 * the sidebar Skills view, which is the one answer to what a chat gets and why. The skills chip
 * on its pane, its "started without" notice's Show in Skills and its "skills changed" notice's
 * What changed each ask here; the project's window puts that chat in front, so the view follows
 * it, and shows the view.
 *
 * An event on the window rather than a prop, as the studio's own ask is (`askSkillStudio`): a
 * notice is drawn in its pane and in the Inbox, and neither holds the window's layout.
 */

/** The event a chat's surface asks for its skills by. */
export const CHAT_SKILLS_ASKED = "purlis-chat-skills-asked";

/** What it asks: which project's window, and whose skills. */
export type ChatSkillsAsk = { plane: string; session: number };

/** **Asks the window of `plane` to show chat `session`'s skills** in the sidebar Skills view. */
export function askChatSkills(plane: string, session: number): void {
  window.dispatchEvent(
    new CustomEvent<ChatSkillsAsk>(CHAT_SKILLS_ASKED, { detail: { plane, session } }),
  );
}

/** What the chat's surfaces call the way there. */
export const SHOW_IN_SKILLS = "Show in Skills";
