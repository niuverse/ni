"""Runs inside the evaluation container; never receives the upstream API key."""
import json
import os
from pathlib import Path
import sys

MODEL = "deepseek-flash"
WORKFLOW = """Additional verification workflow:
Before editing, inspect the relevant code and run existing tests. Identify a small
reproduction or acceptance check. Make the smallest correct change. Exercise edge
cases, rerun the original tests, and review the diff for unrelated changes.
Use delegation only when it helps. Report commands actually executed and unresolved
failures. Do not change tests to manufacture success.
"""


def main():
    harness, track, endpoint, task_file = sys.argv[1:]
    task = Path(task_file).read_text()
    if track == "enhanced":
        task += "\n\n" + WORKFLOW
    home = Path.home()
    home.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    # This is a dummy local gateway credential, never a real upstream credential.
    env.update(DEEPSEEK_API_KEY="benchmark-local", DEEPSEEK_MODEL=MODEL,
               DEEPSEEK_THINKING="enabled", DEEPSEEK_REASONING_EFFORT="high",
               OPENAI_API_KEY="benchmark-local", MSWEA_CONFIGURED="true",
               MSWEA_GLOBAL_CONFIG_DIR=str(home / ".mini"),
               LITELLM_LOCAL_MODEL_COST_MAP="true", TERM="dumb", NO_COLOR="1")
    endpoint = endpoint.rstrip("/")
    if harness == "niucode":
        env["DEEPSEEK_BASE_URL"] = endpoint
        args = ["niucode", "exec", task, "--model", MODEL, "--cwd", "/workspace",
                "--max-steps", "40", "--context-budget", "64000",
                "--sandbox", "full-access", "--network", "on", "--approval", "never",
                "--session-dir", "/artifacts/sessions", "--json"]
        # Outer container supplies isolation equally to every harness.
        if track == "basic":
            args += ["--tools", "read,write,edit,grep,glob,bash"]
    elif harness == "pi":
        directory = home / ".pi" / "agent"
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "models.json").write_text(json.dumps({"providers": {"benchmark": {
            "baseUrl": endpoint + "/v1", "api": "openai-completions",
            "apiKey": "OPENAI_API_KEY", "models": [{
                "id": MODEL, "name": MODEL, "reasoning": True, "input": ["text"],
                "contextWindow": 64000, "maxTokens": 8192,
                "compat": {"supportsDeveloperRole": False, "supportsStore": False},
                "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
            }]}}}))
        args = ["pi", "--provider", "benchmark", "--model", MODEL, "--thinking", "high",
                "--no-mcp", "--no-skills", "--no-prompt-templates", "--no-context-files",
                "--session-dir", "/artifacts/sessions", "--mode", "json", "-p", task]
        if track == "basic":
            args += ["--no-extensions", "--tools", "read,bash,edit,write"]
        else:
            extension = Path("/opt/agents/node_modules/@earendil-works/pi-coding-agent/examples/extensions/subagent/index.ts")
            if not extension.is_file():
                raise RuntimeError("pinned official Pi subagent example unavailable")
            agents = directory / "agents"
            agents.mkdir()
            (agents / "reviewer.md").write_text("---\nname: reviewer\ndescription: Inspect changes and verify edge cases\ntools: read,bash\n---\nReview the task and changes. Run checks and report concrete issues. Do not modify files.\n")
            (agents / "worker.md").write_text("---\nname: worker\ndescription: Implement a bounded coding subtask\ntools: read,bash,edit,write\n---\n" + WORKFLOW)
            args += ["--no-extensions", "-e", str(extension)]
    elif harness == "mini":
        import yaml
        config = {
            "agent": {"step_limit": 40, "cost_limit": 0},
            "environment": {"cwd": "/workspace", "timeout": 120},
            "model": {"model_class": "litellm", "model_name": "openai/" + MODEL,
                      "cost_tracking": "ignore_errors",
                      "model_kwargs": {"api_base": endpoint + "/v1", "api_key": "benchmark-local",
                                       "max_tokens": 8192, "reasoning_effort": "high",
                                       "extra_body": {"thinking": {"type": "enabled"}}}}
        }
        config_path = home / "mini.yaml"
        config_path.write_text(yaml.safe_dump(config))
        args = ["python3", "-m", "minisweagent.run.mini", "-c", "mini.yaml", "-c", str(config_path),
                "-m", "openai/" + MODEL, "-y", "--exit-immediately",
                "-o", "/artifacts/trajectory.json", "-t", task]
    elif harness == "dsh":
        env["DSH_HOME"] = str(home / ".dsh")
        env["DSH_PERMISSION_MODE"] = "danger-full-access"
        env["DEEPSEEK_BASE_URL"] = endpoint + "/anthropic"
        config = [
            {"id": "llm-deepseek", "config": {"baseURL": endpoint + "/anthropic",
                "thinking": "enabled", "reasoningEffort": "high", "maxTokens": 8192,
                "defaultContextWindow": 64000, "models": [{
                    "id": MODEL, "name": MODEL, "contextWindow": 64000,
                    "maxTokens": 8192, "inputModalities": ["text"]}]}},
            {"id": "agent-default-model", "config": {
                "provider": "deepseek-official", "model": MODEL, "reasoningEffort": "high"}},
        ]
        disabled = ["web-search-deepseek", "web-fetch-http"]
        if track == "basic":
            disabled += ["tool-subagent", "tool-subagent-fork", "tool-subagent-control",
                         "tool-subagent-list-agents", "skill-filesystem"]
        config += [{"id": name, "disabled": True} for name in disabled]
        # JSON is a YAML subset, accepted by the profile loader.
        config_path = home / "benchmark.patch.yml"
        config_path.write_text(json.dumps(config, indent=2))
        args = ["dsh", "--profile", "headless", "--patch", str(config_path), "--json", task]
    else:
        raise ValueError("unknown harness")
    Path("/artifacts/adapter.json").write_text(json.dumps({
        "harness": harness, "track": track, "model": MODEL, "argv": args,
        "enhancement": WORKFLOW if track == "enhanced" else None,
    }, indent=2))
    # Replace the adapter, keeping cancellation tied to the container lifecycle.
    os.execvpe(args[0], args, env)


if __name__ == "__main__":
    main()

