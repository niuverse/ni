# niucode-computer-macos

macOS backend for niucode computer use.

- CoreGraphics window enumeration and window snapshots.
- PNG encoding from the CGImage bitmap.
- CGEvent mouse, keyboard, Unicode text, scroll.
- Screen Recording preflight; Accessibility check before every input action.
- Emergency stop releases common modifier keys and mouse buttons.
- No shelling out to `screencapture`/AppleScript.

The first slice uses the CoreGraphics window snapshot API. A ScreenCaptureKit
observer is a later upgrade for newer macOS releases. TCC permissions cannot be
granted by the binary; unsigned builds may re-prompt after each rebuild.
