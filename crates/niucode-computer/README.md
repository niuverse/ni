# niucode-computer

Protocol, permission, and observation contracts for native computer use.

This crate has no OS backend. The first backend is Linux X11; macOS is a second
milestone. Keeping the types independent lets the permission matrix, stale
observation checks, and audit metadata stay identical across backends.

See `docs/adr/0037-native-computer-use.md` in the repository.
