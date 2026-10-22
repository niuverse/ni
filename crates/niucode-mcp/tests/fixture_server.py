#!/usr/bin/env python3
"""Tiny MCP stdio server used by the offline client tests."""
import json
import os
import sys
import time


def send(payload):
    print(json.dumps(payload), flush=True)


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    request_id = message.get("id")
    params = message.get("params") or {}
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "fixture", "version": "1"},
            },
        })
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        send({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "tools": [
                    {"name": "echo", "description": "Echo text", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}},
                    {"name": "env", "description": "Report env", "inputSchema": {"type": "object"}},
                    {"name": "fail", "description": "Fail", "inputSchema": {"type": "object"}},
                    {"name": "hang", "description": "Hang", "inputSchema": {"type": "object"}},
                ]
            },
        })
    elif method == "tools/call":
        name = params.get("name")
        arguments = params.get("arguments") or {}
        if name == "echo":
            result = {"content": [{"type": "text", "text": arguments.get("text", "")}], "isError": False}
        elif name == "env":
            text = f"{os.environ.get('LEAK', '')}|{os.environ.get('ALLOWED', '')}"
            result = {"content": [{"type": "text", "text": text}], "isError": False}
        elif name == "fail":
            result = {"content": [{"type": "text", "text": "boom"}], "isError": True}
        elif name == "hang":
            time.sleep(30)
            result = {"content": [{"type": "text", "text": "late"}], "isError": False}
        else:
            send({"jsonrpc": "2.0", "id": request_id, "error": {"code": -32602, "message": "unknown tool"}})
            continue
        send({"jsonrpc": "2.0", "id": request_id, "result": result})
    elif request_id is not None:
        send({"jsonrpc": "2.0", "id": request_id, "error": {"code": -32601, "message": "unknown method"}})
