# Amendment, 2026-10-14: the clipboard's text, read for a Paste the person pressed

A terminal's menu has Paste (a right-click on a chat's or a shell's terminal). The keyboard's
paste is the engine handing the terminal a `paste` event with the text in it; a menu's row gets
no such event, and the page is not given a way to read the clipboard. The operator's ruling of
2026-10-14 is that the core reads it: one command, `clipboard_text`, answers the clipboard's
text to the window, which hands it to the terminal's own paste, the path the keyboard's paste
takes (bracketed when the program asked).

- **It joins `vault_values`.** What is on the clipboard may be a vault's value: a vault's Copy
  puts it there so that it never crosses to the window, and this command is a way for it to. So
  it is granted to purlis's own windows by the class's capability and to nothing else, and the
  class is now "a secret's value, a vault's sign-in, or the clipboard's text, crossing between
  the window and purlis". `ipc.rs` pins the list.
- **No link serves it.** It is on the session protocol's `WINDOW_ONLY` list, so a host built
  with the app's whole command list refuses it on the UI RPC and the generated `uiRpc.ts`
  leaves it out. Nothing that speaks over a link reads the clipboard: no chat, no tool a chat
  calls, no hook, no command-line word.
- **It takes no argument**, so nothing the caller says decides where the text goes: back to
  the window that asked.
- **It reads when asked and at no other time.** Nothing polls the clipboard, nothing keeps what
  was read, and nothing of it is written down, not the text and not its length.
- **It answers whatever text the clipboard holds, a vault's copied value included.** A
  refusal for that value was built and taken out before this merged (the security review's
  recommendation and the dispatcher's decision, following the ruling that Paste pastes). It
  protected nothing: code in the window can already ask for a vault's reveal, and the
  keyboard's paste hands the same value to the same terminal. It would have turned the digest
  a vault's Copy keeps for sixty seconds into a way to test a guess at the value. And it put a
  refusal in the way of the commonest use of a vault's Copy, pasting it into a terminal.
- **Text only, and bounded.** A clipboard holding a picture or files answers as one holding
  nothing. More than 1 MiB of text is refused in a sentence. A read that fails says it failed.

What this does not change: the keyboard's paste already brings the clipboard's text into the
window, in the engine's `paste` event. The command makes the same crossing on a press of a
menu's row and on nothing else. What it adds is that the window's own code can now ask for the
clipboard without the engine's event, which is why it is held to purlis's windows and why an
independent security review reads it before it merges.
