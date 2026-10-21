"""Local SSE + slow PTY consumer. No API calls; optional sustained release gate."""
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
import time
from terminal import Terminal


def run(binary, duration):
    binary_sha256 = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    stop = threading.Event()
    requests = []
    class Stream(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass
        def do_POST(self):
            # Consume request locally; deliberately never save prompts/headers.
            self.rfile.read(int(self.headers.get("Content-Length", "0")))
            requests.append(time.monotonic())
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            i = 0
            try:
                while not stop.wait(.05):
                    delta = {"content": f"line {i:06} · 中文 · streaming response with bounded consumer delay\n"}
                    payload = json.dumps({"choices": [{"index": 0, "delta": delta, "finish_reason": None}]})
                    self.wfile.write(f"data: {payload}\n\n".encode())
                    self.wfile.flush()
                    i += 1
            except (BrokenPipeError, ConnectionResetError):
                pass
    server = ThreadingHTTPServer(("127.0.0.1", 0), Stream)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    with tempfile.TemporaryDirectory(prefix="niucode-pressure-") as directory:
        root = Path(directory)
        (root / "work").mkdir()
        terminal = Terminal(binary,root,color=True,args=["--preset","openai","--model","local-fixture"],
            env_extra={"OPENAI_API_KEY":"local-test-only", "OPENAI_BASE_URL":f"http://127.0.0.1:{server.server_port}/v1"})
        try:
            terminal.wait_for(b"\x1b[?2004h")
            os.write(terminal.master,b"stream for pressure test\r")
            terminal.wait_for(b"line 000000")
            started = time.monotonic()
            rss, total_bytes, views, second = [],0,0,-1
            details = False
            saw_animation = False
            while time.monotonic()-started < duration:
                # Deliberate bounded consumer delay, with periodic details ownership changes.
                stop.wait(.1)
                terminal.pump(.01)
                saw_animation |= any(frame.encode() in terminal.transcript for frame in ["⠋","⠙","⠹","⠸"])
                elapsed = int(time.monotonic()-started)
                if elapsed != second:
                    second = elapsed
                    total_bytes += len(terminal.transcript)
                    terminal.transcript.clear()
                    path=Path(f"/proc/{terminal.child.pid}/status")
                    if path.exists():
                        rss.append(int(next(s for s in path.read_text().splitlines() if s.startswith("VmRSS:")).split()[1]))
                    if elapsed and elapsed % 60 == 0:
                        print(f"pressure elapsed={elapsed}s rss={rss[-1] if rss else 'n/a'}KiB views={views}",file=sys.stderr,flush=True)
                    if elapsed % 15 == 2 and not details:
                        os.write(terminal.master,b"\x0f")
                        details=True
                        views+=1
                    elif elapsed % 15 == 4 and details:
                        os.write(terminal.master,b"q")
                        details=False
                assert terminal.child.poll() is None, "TUI exited during stream"
            terminal.transcript.clear()
            cancelled = time.monotonic()
            os.write(terminal.master,b"\x03")
            terminal.wait_for(b"cancelled")
            terminal.wait_for(b"\x1b[?2004h",terminal.transcript.index(b"cancelled"))
            cancel_ms=(time.monotonic()-cancelled)*1000
            assert b"done" not in terminal.transcript
            events=terminal.events()
            assert any(e["type"]=="task_finished" and e["status"]=="cancelled" for e in events)
            assert len(requests)==1, "unexpected request/retry"
            assert saw_animation, "active feedback did not animate"
            os.write(terminal.master,b"\x04")
            terminal.child.wait(timeout=5)
            assert terminal.child.returncode==0
            terminal.assert_restored()
            report={"duration_s":round(time.monotonic()-started,2),"cancel_ms":round(cancel_ms,2),
                    "requests_to_local_fixture":len(requests),"detail_visits":views,"output_bytes":total_bytes,
                    "rss_kib_first":rss[0] if rss else None,"rss_kib_last":rss[-1] if rss else None,
                    "rss_kib_peak":max(rss) if rss else None,
                    "binary_sha256":binary_sha256}
            print(json.dumps(report,indent=2),flush=True)
            return report
        finally:
            stop.set()
            terminal.close()
            server.shutdown()
            server.server_close()
            worker.join(timeout=2)


if __name__ == "__main__":
    run(str(Path(sys.argv[1]).resolve()),float(sys.argv[2]) if len(sys.argv)>2 else 5)
