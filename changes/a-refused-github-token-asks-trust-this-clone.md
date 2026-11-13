### Changed

- A chat whose GitHub App token can't be minted because purlis has no record of a clone in its
  workspace now raises an ask in Asks, "Trust this clone so chats can use GitHub", with
  **Trust this clone** on its row. Pressing it trusts the clone, as the clone's own notice does,
  and the chat's next `gh` gets its token. The ask comes back even if you dismissed the clone's
  notice. The chat is told the ask was raised, not to "save the clone from the window".
