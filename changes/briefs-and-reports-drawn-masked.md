### Security

- **A secret in a dispatch's brief or report is masked wherever the window shows it.** Activity,
  Dispatches, a finished task's row and the brief reader replace each vault value purlis read or
  stored, each credential-shaped stretch and each forge token it holds with `***`, checked every
  time the text is shown, so a value purlis learns later is masked too. The record keeps the text
  as written (#1872).
- **The app log leaves out an event that holds a vault value purlis read or stored**, whatever
  the value looks like, as it already did for forge tokens (#1872).
