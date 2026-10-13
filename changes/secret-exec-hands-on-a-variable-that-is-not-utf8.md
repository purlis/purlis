### Fixed

- **A brokered `purlis secret exec` hands on a variable that is not UTF-8.** A chat's variable
  whose name or value is not UTF-8, such as a path in another encoding, was dropped without a
  word on its way to the command the app runs. It is now handed on as its bytes (#1407).
