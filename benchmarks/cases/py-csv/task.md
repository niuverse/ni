# CSV ledger
Add summarize(text,group,value,missing='error') to summary.py. Return group->exact
decimal total strings in encounter order. Honor CSV quoting. Blank/absent amounts:
error raises ValueError, skip omits the row, zero contributes 0. Unknown policies,
missing columns, invalid/nonfinite amounts raise ValueError. Use exact decimal
arithmetic (0.1+0.2 => "0.3"). Existing total(text) and CLI remain compatible.
Existing tests: python -m unittest discover -s tests

Use only the standard library. Do not add dependencies or change build manifests.
