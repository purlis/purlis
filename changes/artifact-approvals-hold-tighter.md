### Security

- **What an artifact's page keeps lasts only as long as its approval.** An approved page's local
  storage, IndexedDB, cookies and caches belong to that approval: a changed template, a changed
  pin, or an unpin and a pin again starts it empty, and asks again. Unpinning an artifact now
  forgets your approval of it on this machine.
- **Regenerate runs only the code you approved.** A run reads nothing of the project beyond the
  folder of code its prompt names and the artifact's own folder, so a file it imports from
  elsewhere in the project no longer loads.
- **A pin's id is longer**, 32 hex digits, so no other command can be made to share it. An
  approval given under an older, shorter id asks once more.
