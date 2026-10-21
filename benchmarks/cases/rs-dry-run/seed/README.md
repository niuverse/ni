# Config updater
Add optional --dry-run before FILE VALUE. Add update(path,value,dry_run) returning
the same change summary as set_value; retain set_value compatibility. Dry run must
not write the file, including modification time. Normal CLI writes VALUE plus newline.
Both modes print identical OLD -> NEW summaries. Missing file/invalid arguments
exit nonzero without panic. Existing tests: cargo test --offline

Use only the standard library. Do not add dependencies or change build manifests.
