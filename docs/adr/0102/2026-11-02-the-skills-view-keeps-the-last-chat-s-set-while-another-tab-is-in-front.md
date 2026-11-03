# The Skills view keeps the last chat's set while another tab is in front

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-02**, ticket #4351, by the operator's ruling (spec #3679 S7). It narrows what S7 said the sidebar Skills view shows when no chat is in front.

## The decision

- **While a tab that is not a chat is in front, the Skills view keeps the set of the chat last in front.** This covers the Skill studio, a settings page or a shell. The view's header goes on naming that chat, and nothing is read again: not its set, not a new chat's.
- **It keeps that chat until its tab closes.** After that, with no chat tab open to come back to, the view shows what a new chat in the title bar's workspace gets, as S7 says for no chat in front.
- **With no tab in front at all, nothing changes.** The view shows what a new chat in the title bar's workspace gets.
- **A chat put in front is followed at once,** as before.

## Why

Opening the Skill studio from a chat took the chat out of front. The view then read a new chat's set from the core and drew all its cards again. With 200 skills that was about 400 ms of drawing, plus a read that competed with the studio's own, on every opening (#4351's bar, L10). The studio is where the chat's skills are worked on, so the set of the chat the person came from is also the more useful one to keep beside it.

**Built by #4351.**
