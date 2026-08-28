"""
Minimal synchronous MCP client over stdio -- just enough to spawn tarn-mcp, do
the initialize handshake, list tools, call one, and read a resource.

Not a general MCP SDK: no cancellation, no server-initiated requests, no
sampling. If the bench later needs those, reach for the real `mcp` Python
package rather than extending this by hand.
"""

import itertools
import json
import subprocess
import sys
import threading
from typing import Any

Json = dict[str, Any]

# rmcp 1.2 negotiates down to whatever the client asks for; this is the version
# the spec revision tarn was built against.
PROTOCOL_VERSION = "2025-06-18"


class McpError(RuntimeError):
    """A JSON-RPC level failure. Tool-level failures are not this -- see call_tool."""


class McpStdioClient:
    def __init__(
        self, cmd: list[str], cwd: str | None = None, env: dict[str, str] | None = None
    ):
        self.cmd = cmd
        self.proc = subprocess.Popen(
            cmd,
            cwd=cwd,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self._id_counter = itertools.count(1)
        self._stderr_thread = threading.Thread(target=self._drain_stderr, daemon=True)
        self._stderr_thread.start()

    def _drain_stderr(self) -> None:
        # tarn logs to stderr by design -- nothing but MCP frames reach stdout.
        # Surface it so a startup failure is visible rather than a silent hang.
        assert self.proc.stderr is not None
        for line in self.proc.stderr:
            sys.stderr.write(f"[tarn-mcp] {line}")

    def _send(self, obj: Json) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(obj) + "\n")
        self.proc.stdin.flush()

    def _recv(self) -> Json:
        assert self.proc.stdout is not None
        line = self.proc.stdout.readline()
        if not line:
            code = self.proc.poll()
            raise McpError(
                f"tarn-mcp closed stdout unexpectedly (exit {code}) -- check the "
                f"[tarn-mcp] stderr lines above for the actual error"
            )
        result: Json = json.loads(line)
        return result

    def request(self, method: str, params: Json | None = None) -> Any:
        req_id = next(self._id_counter)
        self._send(
            {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params or {}}
        )
        while True:
            msg = self._recv()
            if msg.get("id") == req_id:
                if "error" in msg:
                    raise McpError(f"{method} failed: {msg['error']}")
                return msg.get("result")
            # notification, or a response to an id we're not tracking -- ignore

    def notify(self, method: str, params: Json | None = None) -> None:
        self._send({"jsonrpc": "2.0", "method": method, "params": params or {}})

    def initialize(self) -> Json:
        """Handshake. Returns the initialize result, whose serverInfo carries the
        running binary's own name and version (rmcp fills it from the crate's
        build environment) -- that is the authoritative version for the manifest.

        This call is also the index-ready signal. tarn attaches the stdio
        transport only after start_sync has finished its synchronous review pass
        (ADR-0011), so this response cannot arrive before the index reflects the
        vault. On a cold vault that means it blocks for the whole index build.
        """
        result = self.request(
            "initialize",
            {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "tarn-bench", "version": "0.1"},
            },
        )
        self.notify("notifications/initialized")
        assert isinstance(result, dict)
        return result

    def list_tools(self) -> list[Json]:
        tools: list[Json] = self.request("tools/list")["tools"]
        return tools

    def call_tool(self, name: str, arguments: Json) -> Any:
        """Call a tool. Note that tarn reports domain failures as a *successful*
        call with isError: true and the message in content[0].text, not as a
        JSON-RPC error -- callers that care must check isError themselves."""
        return self.request("tools/call", {"name": name, "arguments": arguments})

    def read_resource_json(self, uri: str) -> Any:
        """Read a resource and parse its text content as JSON.

        tarn serves resources as pretty-printed JSON inside a text content block
        rather than as structured content, so the parse happens here.
        """
        result = self.request("resources/read", {"uri": uri})
        contents = result.get("contents") or []
        if not contents:
            raise McpError(f"{uri} returned no contents")
        return json.loads(contents[0]["text"])

    def close(self) -> None:
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
        finally:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
