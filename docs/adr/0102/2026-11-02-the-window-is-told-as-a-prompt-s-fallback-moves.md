# The window is told as a prompt's fallback moves

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-02**, ticket #4445 (SK-15n). The amendment of 2026-11-01 kept a chat's latest prompt routing and left the window to find it at its next read, so the Skills chip on a chat's pane said none of it. This takes back that amendment's "What it does not do".

## What the window is told

As a chat's latest prompt changes whether it fell back, the core sends `chat-prompt-routing`, naming the plane and the chat. The change can go either way: a prompt falls back after none did, a later one is answered, or a router stops reading prompts and leaves none. A prompt that leaves the answer as it was sends nothing, so a router that answers every prompt asks the window to read nothing, and one slow on every prompt asks it once.

The event names the chat and says nothing of how it did. The window reads the routing with the read it already makes, so tellings that land out of order leave it nothing to get wrong. `plane-changed` does not carry this: it says what moved on disk, and this routing is in memory.

## What reads it

- **The Skills chip on a chat's pane** reads that chat's set again, and says a prompt's fallback as it says a start's, with the need's title. It is the warning look the chip already had.
- **The Skills view** reads again where the chat is the one in front.
- **The Skill studio** reads again, for its Routing page.

A window with the studio off listens to nothing.

## What it leaves

The window is still not told of a prompt that was answered, so the timing beside an answered prompt is the one of its last read. A chat's timing is not worth a read per prompt.
