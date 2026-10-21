"""Offline output/log-volume and cancellation regression; synthetic workspace only."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time
from terminal import Terminal


def run(binary, rounds=3):
    with tempfile.TemporaryDirectory(prefix="niucode-volume-") as directory:
        root = Path(directory)
        work = root / "work"
        work.mkdir()
        subprocess.run(["git", "init", "-q", str(work)], check=True)
        # 264 readable bodies and 130 deliberately out-of-budget bodies.
        for i in range(264):
            (work / f"small-{i:03}").write_text((f"line {i:03} abcdefghijklmnop\n" * 600))
        for i in range(130):
            with (work / f"large-{i:03}").open("wb") as f:
                f.truncate(512 * 1024 + 1)
        cancels = 10 if rounds >= 100 else 2
        fixture = root / "fixture.json"
        fixture.write_text(json.dumps({"schema_version": 1, "turns": [
            [{"type": "text_delta", "text": f"ANSWER_{i}\n"}] for i in range(rounds)
        ] + [[{"type": "tool_call_delta", "index": 0, "id": f"write-{i}", "name": "write",
                "arguments": json.dumps({"path": f"cancelled-{i}", "content": "never written"})}]
             for i in range(cancels)] + [[{"type":"text_delta", "text":"partial response"},
                {"type":"done", "finish_reason":"length"}]]}))
        terminal = Terminal(binary, root, args=["--replay", str(fixture), "--approval", "ask"])
        sizes, times, cancellation, task_stop, memory = [], [], [], [], []
        try:
            terminal.wait_for(b"\x1b[?2004h")
            for i in range(rounds):
                terminal.transcript.clear()
                started = time.monotonic()
                os.write(terminal.master, b"hello\r")
                terminal.wait_for(b"done")
                terminal.wait_for(b"\x1b[?2004h", terminal.transcript.index(b"done"))
                times.append((time.monotonic()-started)*1000)
                raw = terminal.transcript
                assert raw.count(b"done") == 1, raw[-1000:]
                assert b"origin unknown" not in raw and b"unverified" not in raw
                assert b"small-" not in raw and b"large-" not in raw
                assert f"ANSWER_{i}".encode() in raw
                path = next((root / "sessions").glob("*.jsonl"))
                sizes.append(path.stat().st_size)
                status = Path(f"/proc/{terminal.child.pid}/status")
                if status.exists():
                    memory.append(int(next(line for line in status.read_text().splitlines() if line.startswith("VmRSS:")).split()[1]))
            events = terminal.events()
            starts={e["id"]:e["ts"] for e in events if e["type"]=="task_started"}
            capture_before=[e["ts"]-starts[e["task_id"]] for e in events if e["type"]=="task_snapshot" and e["phase"]=="before"]
            replay_started=time.monotonic()
            subprocess.run([binary,"replay",str(path)],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,check=True,timeout=10)
            replay_ms=(time.monotonic()-replay_started)*1000
            snapshots = [e["snapshot"] for e in events if e["type"] == "task_snapshot"]
            assert all(len(s["files"]) == 394 for s in snapshots)
            assert all(sum(f["digest"] is None for f in s["files"]) == 130 for s in snapshots)
            bodies = [f["text"] for s in snapshots for f in s["files"] if f["text"] is not None]
            assert len(bodies) == 264, len(bodies)
            incremental = [sizes[i]-sizes[i-1] for i in range(1,len(sizes))]
            # Old implementation records both complete snapshot bodies every round.
            body_map = {hashlib.sha256(b.encode()).hexdigest():b for b in bodies}
            old_bytes = sum(len(json.dumps({**s, "files": [{**f, "text": body_map.get(f["digest"])} for f in s["files"]]}).encode()) for s in snapshots[:2])
            assert max(incremental) < old_bytes * 0.1, (incremental,old_bytes)
            for i in range(cancels):
                terminal.transcript.clear()
                os.write(terminal.master,b"write a file\r")
                terminal.wait_for(b"a allow / d deny")
                terminal.transcript.clear()
                started=time.monotonic()
                wall_started=time.time()*1000
                os.write(terminal.master,b"\x03")
                terminal.wait_for(b"cancelled")
                terminal.wait_for(b"\x1b[?2004h",terminal.transcript.index(b"cancelled"))
                cancellation.append((time.monotonic()-started)*1000)
                finished=[e for e in terminal.events() if e["type"]=="task_finished"][-1]
                task_stop.append(max(0,finished["ts"]-wall_started))
                assert b"done" not in terminal.transcript
                assert not (work / f"cancelled-{i}").exists()
            assert all(e["status"]=="cancelled" for e in terminal.events() if e["type"]=="task_finished" and e["status"]!="complete")
            terminal.transcript.clear()
            os.write(terminal.master,b"truncated response\r")
            terminal.wait_for(b"failed")
            terminal.wait_for(b"\x1b[?2004h",terminal.transcript.index(b"failed"))
            assert b"done" not in terminal.transcript
            assert b"truncated" in terminal.transcript, "error reason must remain visible after terminal exit"
            assert any(e["type"]=="turn_end" and e.get("finish_reason")=="length" for e in terminal.events())
            os.write(terminal.master,b"\x04")
            terminal.child.wait(timeout=5)
            assert terminal.child.returncode == 0
            percentile=lambda values: sorted(values)[min(len(values)-1,int(len(values)*.95))]
            result={"rounds":rounds,"files":394,"unknown":130,"first_round_bytes":sizes[0],
                    "subsequent_round_bytes_max":max(incremental),"old_snapshot_bytes_per_round":old_bytes,
                    "reduction_percent":round((1-max(incremental)/old_bytes)*100,2),
                    "capture_before_ms_median":statistics.median(capture_before),
                    "capture_before_ms_p95":percentile(capture_before),"replay_cli_ms":round(replay_ms,2),
                    "turn_ms_median":round(statistics.median(times),2),"turn_ms_p95":round(percentile(times),2),
                    "cancel_to_idle_ms_p95":round(percentile(cancellation),2),
                    "cancel_task_finished_ms_p95":round(percentile(task_stop),2),"rss_kib_first":memory[0] if memory else None,
                    "rss_kib_last":memory[-1] if memory else None,"rss_kib_peak":max(memory) if memory else None}
            # Keep timing assertions out of routine CI; measurements are release evidence.
            print(json.dumps(result,indent=2))
            return result
        finally:
            terminal.close()


def bulk(binary, count=100):
    with tempfile.TemporaryDirectory(prefix="niucode-bulk-") as directory:
        root=Path(directory);work=root/"work";work.mkdir()
        subprocess.run(["git","init","-q",str(work)],check=True)
        calls=[{"type":"tool_call_delta","index":i,"id":f"write-{i}","name":"write",
                "arguments":json.dumps({"path":f"file-{i:03}","content":f"body {i}\n"})} for i in range(count)]
        fixture=root/"fixture.json"
        fixture.write_text(json.dumps({"schema_version":1,"turns":[calls,[{"type":"text_delta","text":"BULK_SENTINEL\n"}]]}))
        terminal=Terminal(binary,root,args=["--replay",str(fixture),"--sandbox","full-access","--approval","never"])
        try:
            terminal.wait_for(b"\x1b[?2004h")
            terminal.transcript.clear();os.write(terminal.master,b"create fixture files\r")
            terminal.wait_for(b"done")
            terminal.wait_for(b"\x1b[?2004h",terminal.transcript.index(b"done"))
            assert terminal.transcript.count(b"done")==1
            assert b"100 confirmed changes" in terminal.transcript
            assert b"file-" not in terminal.transcript
            assert b"BULK_SENTINEL" in terminal.transcript
            for i in range(count):
                assert (work/f"file-{i:03}").read_text()==f"body {i}\n"
            terminal.transcript.clear();os.write(terminal.master,b"\x0f")
            terminal.wait_for(b"file-000")
            terminal.transcript.clear();os.write(terminal.master,b"q")
            terminal.wait_for(b"\x1b[?1049l")
            terminal.wait_for(b"\x1b[?2004h",terminal.transcript.index(b"\x1b[?1049l"))
            os.write(terminal.master,b"\x04");terminal.child.wait(timeout=5)
            assert terminal.child.returncode==0
            print("Bulk: 100 recorded mutations, one bounded completion, details preserved",file=sys.stderr)
        finally:
            terminal.close()


if __name__ == "__main__":
    run(str(Path(sys.argv[1]).resolve()),int(sys.argv[2]) if len(sys.argv)>2 else 3)
    if len(sys.argv)<=2:
        bulk(str(Path(sys.argv[1]).resolve()))
