# niucode-computer-x11

Linux X11 observe backend for native computer use.

- Pure Rust `x11rb` + XTEST-ready connection.
- Enumerates viewable top-level windows with application/title/class/geometry.
- Captures a window as PNG and derives an observation hash.
- Classification marks terminal and sensitive/authentication windows.
- No input injection in this slice; click/type/key/scroll follow.

Integration tests start Xvfb when it is installed and otherwise skip.
