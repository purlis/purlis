### Security

- **A generated file is withdrawn through the folders held open on the way down.** When a
  project stops declaring a file purlis generated in a workspace folder, the file is unlinked
  inside the folder the walk down holds, and the folders it leaves empty go the same way, so a
  folder swapped for a link in the meantime can no longer carry the removal outside the project
  (#1583).
