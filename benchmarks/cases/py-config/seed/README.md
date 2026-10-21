# Layered configuration
Fix config.resolve: precedence defaults < file < environment < CLI.
Environment values are JSON strings; CLI values are typed. None means absent
in environment/CLI. False, zero, empty strings/collections are explicit values.
Do not mutate input dictionaries. Invalid environment JSON raises ValueError.
Keep python cli.py FILE printing resolved JSON with defaults from defaults.py.
Existing tests: python -m unittest discover -s tests

Use only the standard library. Do not add dependencies or change build manifests.
