import json
from pathlib import Path
import tempfile
import unittest

from bench import CASES, ROOT, execute
from live import snapshot_candidate
from report import billing, report
from audit import audit


class RunnerTests(unittest.TestCase):
    def test_timeout_and_output_bound(self):
        with tempfile.TemporaryDirectory() as directory:
            outcome = execute(["sh", "-c", "sleep 30 & wait"], directory, timeout=0.1)
            self.assertTrue(outcome["timeout"])
            self.assertLess(outcome["seconds"], 3)
            outcome = execute(["python3", "-c", "print('x'*50000)"], directory)
            self.assertGreater(outcome["output_bytes"], 24_000)
            self.assertEqual(len(outcome["output_tail"]), 24_000)

    def test_grader_restores_original_tests_and_rejects_links(self):
        case = CASES / "py-config"
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            candidate = directory / "candidate"
            candidate.mkdir()
            (candidate / "config.py").write_text("NEW_SOURCE = True\n")
            (candidate / "tests").mkdir()
            (candidate / "tests/test_smoke.py").write_text("raise SystemExit(0)")
            (candidate / "Cargo.toml").write_text("untrusted")
            work = directory / "grade"
            snapshot_candidate(case, candidate, work)
            self.assertEqual((work / "config.py").read_text(), "NEW_SOURCE = True\n")
            self.assertEqual((work / "tests/test_smoke.py").read_bytes(),
                             (case / "seed/tests/test_smoke.py").read_bytes())
            self.assertFalse((work / "Cargo.toml").exists())
            (candidate / "escape.py").symlink_to("/etc/passwd")
            with self.assertRaises(RuntimeError):
                snapshot_candidate(case, candidate, directory / "grade2")

    def test_missing_usage_keeps_reservation_and_missing_runs_visible(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            events = [
                {"kind": "reserved", "request": 0, "run": "interrupted", "upper_micro_cny": 2000000},
                {"kind": "unknown", "request": 0},
                {"kind": "reserved", "request": 1, "run": "done", "upper_micro_cny": 2000000},
                {"kind": "settled", "request": 1, "upper_micro_cny": 30,
                 "input_tokens": 3, "output_tokens": 3},
            ]
            path = directory / "billing.jsonl"
            path.write_text("".join(json.dumps(event) + "\n" for event in events))
            self.assertEqual(billing(path)["interrupted"]["unknown_usage"], 1)
            summary = report(directory)
            self.assertEqual(summary["billing_without_result"], ["done", "interrupted"])
            self.assertEqual(summary["upper_cny"], 2.00003)
            path.write_text("{")
            with self.assertRaises(ValueError):
                billing(path)

    def test_external_root_level_tests_are_restored(self):
        case = ROOT / "anchors/polyglot-py-transpose"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / "candidate"
            candidate.mkdir()
            (candidate / "transpose.py").write_text("def transpose(text): return text\n")
            (candidate / "transpose_test.py").write_text("raise SystemExit(0)\n")
            snapshot_candidate(case, candidate, root / "graded")
            self.assertEqual((root / "graded/transpose_test.py").read_bytes(),
                             (case / "seed/transpose_test.py").read_bytes())

    def test_audit_does_not_drop_missing_pairs(self):
        from bench import digest_tree
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = {"suite": "synthetic", "smoke_only": False, "cases": {
                "py-config": digest_tree(CASES / "py-config")},
                "harnesses": ["niucode", "pi"], "tracks": ["basic"], "repeats": 1}
            (root / "frozen-plan-synthetic.json").write_text(json.dumps(plan))
            result = audit(root)
            self.assertFalse(result["passed"])
            self.assertEqual(result["expected_formal"], 2)
            self.assertEqual(result["completed_formal"], 0)
            self.assertEqual(len(result["errors"]), 2)


if __name__ == "__main__":
    unittest.main()

