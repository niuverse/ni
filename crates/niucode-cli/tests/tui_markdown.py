#!/usr/bin/env python3
"""PTY regression: Markdown emphasis, headings, and a pipe table render."""
import errno, fcntl, json, os, pty, select, struct, subprocess, sys, tempfile, termios, time
from pathlib import Path


class Pane:
    def __init__(self, binary, root):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
        env = dict(os.environ, HOME=str(root / "home"), TERM="xterm-256color", NO_COLOR="1",
                   NIUCODE_HOME=str(root / "niucode-home"))
        self.child = subprocess.Popen(
            [binary, "tui", "--cwd", str(root / "work"), "--replay", str(root / "fixture.json"),
             "--approval", "never", "--sandbox", "read-only", "--network", "off"],
            stdin=self.slave, stdout=self.slave, stderr=self.slave, env=env, start_new_session=True)
        self.transcript = bytearray(); self.tail = b""

    def pump(self, timeout):
        if not select.select([self.master], [], [], timeout)[0]:
            return
        try:
            chunk = os.read(self.master, 65536)
        except OSError as error:
            if error.errno == errno.EIO:
                return
            raise
        self.transcript.extend(chunk)
        q = self.tail + chunk
        count = q.count(b"\x1b[6n")
        if count:
            os.write(self.master, b"\x1b[30;1R" * count)
        self.tail = q[-3:]

    def wait_for(self, marker, timeout=15):
        deadline = time.monotonic() + timeout
        while marker not in self.transcript:
            if self.child.poll() is not None:
                raise AssertionError(f"child exited before {marker!r}\n{self.transcript[-5000:]!r}")
            if time.monotonic() > deadline:
                raise AssertionError(f"missing {marker!r}\n{self.transcript[-5000:]!r}")
            self.pump(0.05)

    def close(self):
        if self.child.poll() is None:
            self.child.terminate()
        try:
            self.child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.child.kill(); self.child.wait()
        os.close(self.master); os.close(self.slave)


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: tui_markdown.py <niucode-binary>")
    with tempfile.TemporaryDirectory(prefix="niucode-pty-markdown-") as directory:
        root = Path(directory)
        (root / "work").mkdir(); (root / "home").mkdir()
        text = (
            "# Rendered heading\n\n"
            "This is **bold**, *italic*, and `code`.\n\n"
            "| name | value |\n"
            "| :--- | ---: |\n"
            "| alpha | 42 |\n"
            "| beta | 7 |\n\n"
            "Energy $E = mc^2$ and $\\alpha \\le \\beta$.\n\n"
            "- first item\n"
            "- [ ] pending task\n"
            "1. first numbered\n"
            "2. second numbered\n"
            "---\n"
            "> quoted line\n\n"
            "> [!WARNING] careful\n\n"
            "```rust\n"
            "/* start\n"
            "still */ fn main() {}\n"
            "```\n\n"
            "$$\n"
            "\\alpha + \\beta = \\gamma\n"
            "$$\n"
        )
        (root / "fixture.json").write_text(json.dumps({
            "schema_version": 1,
            "turns": [[{"type": "text_delta", "text": text}]],
        }))
        image = root / "shot.png"
        image.write_bytes(b"\x89PNG\r\n\x1a\nimage")
        pane = Pane(sys.argv[1], root)
        try:
            pane.wait_for(b"\x1b[?2004h")
            os.write(pane.master, b"/computer\r")
            pane.wait_for(b"computer: off")
            time.sleep(0.2)
            os.write(pane.master, f"/attach {image}\r".encode())
            pane.wait_for("Image · shot.png".encode())
            time.sleep(0.4)
            os.write(pane.master, b"show markdown\r")
            pane.wait_for(b"Rendered heading")
            pane.wait_for(b"bold")
            pane.wait_for(b"alpha")
            pane.wait_for(b"42")
            pane.wait_for("α".encode())
            pane.wait_for("≤".encode())
            pane.wait_for("• first item".encode())
            pane.wait_for("☐ pending task".encode())
            pane.wait_for("1. first numbered".encode())
            pane.wait_for("2. second numbered".encode())
            pane.wait_for("────".encode())
            pane.wait_for("│ quoted line".encode())
            pane.wait_for("│ warning · careful".encode())
            pane.wait_for(("─" * 10).encode())
            pane.wait_for("▎ rust".encode())
            pane.wait_for(b"still */ fn main")
            pane.wait_for("α + β = γ".encode())
            # Highlight attributes and box drawing are evidence that the
            # table was rendered as a block, not printed as raw pipes.
            pane.wait_for(b"\x1b[")
            decoded = pane.transcript.decode("utf-8", "replace")
            assert "├" in decoded and "│ alpha" in decoded, decoded[-3000:]
            assert "**bold**" not in decoded, "bold markers leaked to the terminal"
            assert "[!WARNING]" not in decoded, "alert marker leaked to the terminal"
            assert "| name | value |" not in decoded, "raw table line leaked"
            assert "```" not in decoded, "code fence leaked to the terminal"
            assert "$$" not in decoded, "display math markers leaked to the terminal"
            sessions = [
                path for path in root.rglob("*.jsonl")
                if '"attachments"' in path.read_text(errors="replace")
            ]
            assert sessions, "the attachment ref did not reach the session log"
            print("PTY markdown: heading, emphasis, code, and table rendered")
        finally:
            pane.close()


if __name__ == "__main__":
    main()
