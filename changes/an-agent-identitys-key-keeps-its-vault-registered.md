### Fixed

- **An agent identity's vault stays registered until the identity is removed.** `purlis vault
  remove` now refuses a keyring vault that holds an agent identity's key, as `vault destroy`
  already did, and the refusal of a GitHub App's code exchange says to delete that App and close
  the browser tab. Finish allows exactly the lines it wrote to the project.
