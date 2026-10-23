### Added

- **Draft skills in the Skill studio preview** (#2772).
  - **Ask a chat to draft a skill** opens a chat with the request already typed. You finish the sentence and send it.
  - **A chat can't write a skill's files any more.** What it writes becomes a draft skill in the studio, kept on this machine and never committed. The chat's tab only says the draft is ready in the Skill studio, and nothing else asks you about it.
  - **Changes** shows each file the draft writes, against what the project has now.
  - **Try it** opens a scratch chat that has purlis's built-in skills and the draft, and no other skill from the project. It starts in a throwaway folder, on the harness profile the draft's workspace uses or the one you pick. Your own harness skills and plugins are off in it. Closing it removes the folder. All that's left is the skill's run.
  - **Accept** is one commit of only the draft's files. It's refused if the project's copy changed since the chat drafted it. **Reject** deletes the draft.
