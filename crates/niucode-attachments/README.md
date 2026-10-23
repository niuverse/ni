# niucode-attachments

Content-addressed storage for user image and text attachments.

Files are hashed with SHA-256, written atomically at `0600` under a `0700`
directory, and referenced from events by metadata rather than base64. Budget
checks enforce per-file and per-turn limits before a request is sent.
