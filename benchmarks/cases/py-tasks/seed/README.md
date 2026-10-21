# Task list
Add --status open|done and --json to python cli.py FILE.
FILE is a JSON array with id, title, status fields. Preserve order and complete
objects. JSON output contains no headings. Without flags preserve text output.
Invalid status exits nonzero. Never modify input. Empty filtered JSON is [].
Existing tests: python -m unittest discover -s tests

Use only the standard library. Do not add dependencies or change build manifests.
