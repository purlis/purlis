### Security

- **The folder that holds dispatch reports is never read, emptied or written through a link.**
  Its folders are opened one at a time without following a link, and every report is listed,
  read, written and removed relative to the folder that was opened. A file in it that does not
  read as a report is left where it is (#1463, #1457).
