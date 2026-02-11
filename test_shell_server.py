#!/usr/bin/env python3
"""
Test script for MCP shell server (uvx mcp-shell-server).
Tests initialize, initialized notification, tools/list, and a shell tool call (ls).

If you see timeouts (e.g. on uvx cold start), increase timeouts via env:
  MCP_SHELL_TEST_INITIAL_SLEEP=5     # seconds to wait after starting server (default 3)
  MCP_SHELL_TEST_FIRST_TIMEOUT=60    # timeout for first request, e.g. initialize (default 30)
  MCP_SHELL_TEST_RESPONSE_TIMEOUT=20 # timeout for later requests (default 15)
"""

import json
import os
import subprocess
import sys
import time
from typing import Dict, Any, Optional

# Default allowed commands (match typical MCP config)
DEFAULT_ALLOW_COMMANDS = (
    "ls,cat,pwd,grep,wc,touch,find,echo,mkdir,rmdir,cp,mv,rm,chmod,chown,"
    "git,ps,df,du,whoami,head,tail,sort,uniq,curl,wget,sqlite3,flatpak,"
    "python3,systemctl,cargo"
)

# Timeouts (uvx cold start can be slow; override via env if needed)
INITIAL_SLEEP = float(os.environ.get("MCP_SHELL_TEST_INITIAL_SLEEP", "3.0"))
RESPONSE_TIMEOUT = float(os.environ.get("MCP_SHELL_TEST_RESPONSE_TIMEOUT", "15.0"))
FIRST_RESPONSE_TIMEOUT = float(os.environ.get("MCP_SHELL_TEST_FIRST_TIMEOUT", "30.0"))


class MCPShellServerTester:
    def __init__(
        self,
        command: str = "uvx",
        args: Optional[list] = None,
        allow_commands: Optional[str] = None,
    ):
        self.command = command
        self.args = args or ["mcp-shell-server"]
        self.allow_commands = allow_commands or os.environ.get("ALLOW_COMMANDS", DEFAULT_ALLOW_COMMANDS)
        self.process: Optional[subprocess.Popen] = None
        self.request_id = 1

    def start_server(self):
        """Start the MCP shell server process."""
        env = os.environ.copy()
        env["ALLOW_COMMANDS"] = self.allow_commands

        cmd = [self.command] + self.args
        self.process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0,
            env=env,
        )
        print(f"✓ Started server: {' '.join(cmd)}")
        print(f"✓ ALLOW_COMMANDS: {self.allow_commands[:60]}...")

    def stop_server(self):
        """Stop the server process."""
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            print("✓ Stopped server")

    def send_request(
        self,
        method: str,
        params: Optional[Dict[str, Any]] = None,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """Send a JSON-RPC request and wait for response (newline-delimited JSON over stdio)."""
        if timeout is None:
            timeout = RESPONSE_TIMEOUT

        request = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
        }
        if params:
            request["params"] = params

        self.request_id += 1
        request_json = json.dumps(request) + "\n"
        print(f"\n→ Sending: {method} (timeout={timeout}s)")
        print(f"  Request: {json.dumps(request, indent=2)}")

        if not self.process or not self.process.stdin:
            raise RuntimeError("Server not started")

        self.process.stdin.write(request_json)
        self.process.stdin.flush()

        time.sleep(0.2)

        import select
        if hasattr(self.process.stdout, "fileno"):
            ready, _, _ = select.select([self.process.stdout], [], [], timeout)
            if not ready:
                if self.process.stderr:
                    ready_err, _, _ = select.select([self.process.stderr], [], [], 0.1)
                    if ready_err:
                        stderr_line = self.process.stderr.readline()
                        if stderr_line:
                            print(f"  Server stderr: {stderr_line.strip()}")
                raise RuntimeError(f"No response from server (timeout after {timeout}s)")

        response_line = self.process.stdout.readline()
        if not response_line:
            raise RuntimeError("No response from server")

        response = json.loads(response_line.strip())
        print(f"← Response: {json.dumps(response, indent=2)}")

        if "error" in response:
            print(f"⚠ Server returned error: {response['error']}")

        return response

    def send_notification(self, method: str, params: Optional[Dict[str, Any]] = None):
        """Send a JSON-RPC notification (no response expected)."""
        notification = {"jsonrpc": "2.0", "method": method}
        if params:
            notification["params"] = params

        notification_json = json.dumps(notification) + "\n"
        print(f"\n→ Sending notification: {method}")

        if not self.process or not self.process.stdin:
            raise RuntimeError("Server not started")

        self.process.stdin.write(notification_json)
        self.process.stdin.flush()
        time.sleep(0.1)

    def test_initialize(self):
        """Test initialize request."""
        print("\n" + "=" * 60)
        print("TEST 1: Initialize")
        print("=" * 60)

        response = self.send_request(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "1.0.0"},
            },
            timeout=FIRST_RESPONSE_TIMEOUT,
        )

        assert response.get("jsonrpc") == "2.0", "Invalid JSON-RPC version"
        assert "result" in response, "No result in response"
        protocol_version = response["result"].get("protocolVersion")
        assert protocol_version in ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"], (
            f"Unexpected protocol version: {protocol_version}"
        )
        assert "capabilities" in response["result"], "No capabilities in result"
        assert "serverInfo" in response["result"], "No serverInfo in result"

        print("✓ Initialize test passed")
        return response["result"]

    def test_initialized_notification(self):
        """Test initialized notification."""
        print("\n" + "=" * 60)
        print("TEST 2: Initialized Notification")
        print("=" * 60)

        self.send_notification("notifications/initialized")
        time.sleep(0.2)
        print("✓ Initialized notification sent")

    def test_list_tools(self):
        """Test tools/list request."""
        print("\n" + "=" * 60)
        print("TEST 3: List Tools")
        print("=" * 60)

        response = self.send_request("tools/list")

        assert response.get("jsonrpc") == "2.0", "Invalid JSON-RPC version"
        assert "result" in response, "No result in response"
        assert "tools" in response["result"], "No tools in result"

        tools = response["result"]["tools"]
        print(f"\n✓ Found {len(tools)} tools:")
        for tool in tools:
            desc = (tool.get("description") or "")[:70]
            print(f"  - {tool.get('name')}: {desc}")

        assert len(tools) > 0, "No tools available"
        return tools

    def test_shell_ls(self, tools):
        """Test shell tool with ls command."""
        print("\n" + "=" * 60)
        print("TEST 4: Shell tool — ls")
        print("=" * 60)

        # mcp-shell-server (uvx) exposes "shell_execute"; others use "shell" / "run_command" / "shell_exec"
        tool = next(
            (t for t in tools if t["name"] in ("shell_execute", "shell", "run_command", "shell_exec")),
            None,
        )
        assert tool is not None, "Shell tool not found (expected shell_execute, shell, run_command, or shell_exec)"

        # Arguments: command as array; directory must be absolute (required by shell_execute)
        work_dir = os.path.abspath(".")
        arguments = {"command": ["ls"], "directory": work_dir}
        response = self.send_request("tools/call", {
            "name": tool["name"],
            "arguments": arguments,
        })

        assert response.get("jsonrpc") == "2.0", "Invalid JSON-RPC version"
        assert "result" in response, "No result in response"
        assert "content" in response["result"], "No content in result"

        result = response["result"]
        assert not result.get("isError"), f"Shell command failed: {result.get('content', [{}])[0].get('text', '')}"

        content = result["content"]
        assert len(content) > 0, "No content items"
        assert content[0].get("type") == "text", "Invalid content type"

        text = content[0].get("text", "")
        print(f"\n✓ Shell output (ls):\n{text}")

        # Basic sanity: output should look like ls (some non-empty output)
        assert len(text.strip()) > 0, "Shell returned empty output"

        print("✓ Shell ls test passed")
        return response["result"]

    def run_all_tests(self):
        """Run all tests in sequence."""
        try:
            self.start_server()
            time.sleep(INITIAL_SLEEP)  # uvx cold start can be slow

            if self.process.poll() is not None:
                stderr = self.process.stderr.read() if self.process.stderr else ""
                print(f"\n✗ Server exited early (code {self.process.returncode})\nstderr: {stderr}")
                return False

            init_result = self.test_initialize()
            time.sleep(0.1)

            if self.process.poll() is not None:
                print(f"\n✗ Server terminated after initialize (exit code: {self.process.returncode})")
                return False

            self.test_initialized_notification()
            time.sleep(0.2)

            if self.process.poll() is not None:
                print(f"\n✗ Server terminated after initialized (exit code: {self.process.returncode})")
                if self.process.stderr:
                    try:
                        print(self.process.stderr.read())
                    except Exception:
                        pass
                return False

            tools = self.test_list_tools()
            self.test_shell_ls(tools)

            print("\n" + "=" * 60)
            print("✓ ALL TESTS PASSED!")
            print("=" * 60)
            return True

        except AssertionError as e:
            print(f"\n✗ TEST FAILED: {e}")
            return False
        except BrokenPipeError as e:
            print(f"\n✗ Server connection broken: {e}")
            if self.process and self.process.stderr:
                try:
                    print(self.process.stderr.read())
                except Exception:
                    pass
            return False
        except Exception as e:
            print(f"\n✗ ERROR: {e}")
            import traceback
            traceback.print_exc()
            return False
        finally:
            self.stop_server()


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    allow_commands = os.environ.get("ALLOW_COMMANDS", DEFAULT_ALLOW_COMMANDS)

    tester = MCPShellServerTester(
        command="uvx",
        args=["mcp-shell-server"],
        allow_commands=allow_commands,
    )
    success = tester.run_all_tests()

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
