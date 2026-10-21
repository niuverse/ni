"""Offline PTY protocol regression; does not certify a real terminal renderer."""
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time


class Terminal:
    """One PTY owner for offline scenarios; always reaps child and closes FDs."""
    def __init__(self, binary, root, inline=False, args=(), color=False, env_extra=None):
        self.root = root
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        self.original = termios.tcgetattr(self.slave)
        env = dict(os.environ, HOME=str(root / "home"), TERM="xterm-256color",
                   NIUCODE_AUTH_DISABLE_KEYRING="1", NIUCODE_MODEL_CHOICES="first,second")
        env.update(env_extra or {})
        env.pop("NIUCODE_TUI_LINEAR", None)
        env.pop("NIUCODE_TUI_INLINE", None)
        if color:
            env.pop("NO_COLOR", None)
            env.pop("CLICOLOR", None)
        else:
            env["NO_COLOR"] = "1"
        env["NIUCODE_TUI_INLINE" if inline else "NIUCODE_TUI_LINEAR"] = "1"
        # Keep the PTY on stdin without making it the child's controlling terminal.
        # macOS revokes slave descriptors when the controlling session exits,
        # preventing the parent from checking restored terminal attributes.
        self.child = subprocess.Popen([binary, "tui", "--cwd", str(root / "work"),
            "--session-dir", str(root / "sessions"), *args], stdin=self.slave,
            stdout=self.slave, stderr=self.slave, env=env, start_new_session=True)
        self.transcript = bytearray()
        self.query_tail = b""

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
        queries = self.query_tail + chunk
        count = queries.count(b"\x1b[6n")
        if count:
            os.write(self.master, b"\x1b[20;1R" * count)
        self.query_tail = queries[-3:]

    def wait_for(self, marker, start=0):
        deadline = time.monotonic() + 10
        while marker not in self.transcript[start:]:
            if self.child.poll() is not None or time.monotonic() > deadline:
                raise AssertionError(f"missing {marker!r}\n{self.transcript[-6000:]!r}")
            self.pump(0.05)

    def wait_for_cursor(self, visible, start):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            self.pump(0.05)
            output = self.transcript[start:]
            shown, hidden = output.rfind(b"\x1b[?25h"), output.rfind(b"\x1b[?25l")
            if (shown > hidden if visible else hidden > shown) and not select.select([self.master], [], [], 0.1)[0]:
                return
        raise AssertionError(f"cursor visible={visible} not reached")

    def events(self):
        return [json.loads(line) for path in (self.root / "sessions").glob("*.jsonl")
                for line in path.read_text().splitlines() if line]

    def assert_restored(self):
        expected, restored = self.original.copy(), termios.tcgetattr(self.slave)
        if sys.platform == "darwin":
            # XNU sets PENDIN when ICANON is restored. It is pending-input state,
            # not a mode the application failed to restore (M6-019).
            expected[3] &= ~termios.PENDIN
            restored[3] &= ~termios.PENDIN
        assert restored == expected, f"terminal attributes were not restored: {expected!r} -> {restored!r}"

    def close(self):
        if self.child.poll() is None:
            self.child.kill()
        # Release the PTY before reaping: a dying child may be draining output.
        os.close(self.master)
        os.close(self.slave)
        self.child.wait(timeout=5)


def scenario(binary, inline, cancel):
    with tempfile.TemporaryDirectory(prefix="niucode-pty-") as directory:
        root = Path(directory)
        work = root / "work"
        work.mkdir()
        fixture = root / "fixture.json"
        fixture.write_text(json.dumps({"schema_version": 1, "turns": [
            [{"type": "tool_call_delta", "index": 0, "id": "write-1", "name": "write",
              "arguments": json.dumps({"path": "result.txt", "content": "written\n"})}],
            [{"type": "reasoning_delta", "text": "PRIVATE_REASONING_DETAIL"},
             {"type": "text_delta", "text": "ANSWER_SENTINEL\u001b]52;c;malicious\u0007\n"}],
        ]}))
        terminal = Terminal(binary, root, inline, ["--replay", str(fixture), "--approval", "ask"])
        master, slave, child = terminal.master, terminal.slave, terminal.child
        transcript = terminal.transcript
        pump, wait_for = terminal.pump, terminal.wait_for
        wait_for_cursor, events = terminal.wait_for_cursor, terminal.events

        try:
            wait_for(b"\x1b[?2004h")
            # Bare settings commands focus the picker without changing policy.
            # One confirmation then applies exactly once and returns to editing.
            for command, down, event_type, field, value in [
                (b"/model", True, "model_change", "model", "second"),
                (b"/permissions", False, "permission_change", "profile", "ask"),
            ]:
                start = len(transcript)
                os.write(master, command + b"\r")
                wait_for_cursor(False, start)
                assert not any(e["type"] == event_type for e in events())
                assert transcript.rfind(b"\x1b[?25l") > transcript.rfind(b"\x1b[?25h"), "picker left input cursor visible"
                start = len(transcript)
                os.write(master, (b"\x1b[B" if down else b"") + b"\r")
                wait_for(b"\x1b[?2004h", start)
                wait_for_cursor(True, start)
                changes = [e for e in events() if e["type"] == event_type]
                assert len(changes) == 1 and changes[0][field] == value, (changes, transcript[-4000:])
            assert b"\x1b[6 q" in transcript, "editor did not request a steady bar cursor"
            start = len(transcript)
            os.write(master, b"\x1b[200~" + "中文🙂\r\nsecond line".encode() + b"\x1b[201~")
            wait_for(b"second", start)
            assert not any(e["type"] == "user_message" for e in events()), "paste submitted automatically"
            # One undo removes the complete paste. The model must see only FINAL.
            os.write(master, b"\x1aFINAL\r")
            wait_for(b"a allow / d deny")
            assert [e["text"] for e in events() if e["type"] == "user_message"] == ["FINAL"]
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 18, 48, 0, 0))
            os.kill(child.pid, signal.SIGWINCH)
            start = len(transcript)
            os.write(master, b"\x03" if cancel else b"a")
            wait_for(b"\x1b[?2004h", start)  # Back to the idle composer.
            if cancel:
                assert not (work / "result.txt").exists()
                assert any(e["type"] == "task_finished" and e["status"] == "cancelled" for e in events())
            else:
                assert (work / "result.txt").read_text() == "written\n"
                assert b"ANSWER_SENTINEL" in transcript
                assert b"PRIVATE_REASONING_DETAIL" not in transcript
                assert b"\x1b]52;c;malicious" not in transcript
                start = len(transcript)
                os.write(master, b"\x0f")
                wait_for(b"task changes", start)
                os.write(master, b"[")
                wait_for(b"PRIVATE_REASONING_DETAIL", start)
                start = len(transcript)
                os.write(master, b"q")
                wait_for(b"\x1b[?1049l", start)
                wait_for(b"\x1b[?2004h", start)
            os.write(master, b"\x04")
            deadline = time.monotonic() + 5
            while child.poll() is None and time.monotonic() < deadline:
                pump(0.05)
            assert child.poll() == 0, "TUI did not exit cleanly"
            terminal.assert_restored()
            assert b"\x1b[?2004l" in transcript and b"\x1b[?25h" in transcript
            assert b"\x1b[0 q" in transcript, "default cursor shape was not restored"
        finally:
            terminal.close()


if __name__ == "__main__":
    for use_inline in (False, True):
        for should_cancel in (False, True):
            scenario(sys.argv[1], use_inline, should_cancel)
    print("PTY: linear/inline × approval/cancel; paste undo, resize, details, sanitization, mode restoration passed")
