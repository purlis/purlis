### Fixed

- **A secret fill you take the browser over from keeps its mark.** When you take a chat's browser
  over just as purlis fills a secret, and the value is already in the field, the browser's view now
  marks the field `secret: <key>`, and the chat is told the value went in. Before, the chat was told
  the fill was interrupted and the field had no mark.

### Security

- **A download that holds a secret purlis filled is not brought into a chat's folder.**
  `browser_save_download` refuses a file that holds a value purlis filled into that browser, or
  signed in with, in any form a chat's reads already have it taken out in. A download whose name
  Chrome may have shortened by its length in bytes is named `download`, like a name purlis
  shortens itself.
