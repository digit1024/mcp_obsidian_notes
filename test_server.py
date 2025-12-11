#!/usr/bin/env python3
"""
Test script for MCP server protocol flow.
Tests initialize, initialized notification, tools/list, and tool calls.
"""

import json
import os
import subprocess
import sys
import time
from typing import Dict, Any, Optional


class MCPServerTester:
    def __init__(self, server_path: str, vault_location: str):
        self.server_path = server_path
        self.vault_location = vault_location
        self.process: Optional[subprocess.Popen] = None
        self.request_id = 1

    def start_server(self):
        """Start the MCP server process."""
        env = os.environ.copy()
        env["VAULT_LOCATION"] = self.vault_location
        
        self.process = subprocess.Popen(
            [self.server_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0,  # Unbuffered for better real-time communication
            env=env
        )
        print(f"✓ Started server: {self.server_path}")
        print(f"✓ Using vault: {self.vault_location}")

    def stop_server(self):
        """Stop the server process."""
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            print("✓ Stopped server")

    def send_request(self, method: str, params: Dict[str, Any] = None) -> Dict[str, Any]:
        """
        Send a JSON-RPC request and wait for response.
        
        Note: MCP uses newline-delimited JSON over stdio. Each JSON-RPC message
        must be a single line terminated by \\n. The rmcp library handles this
        automatically, and we use readline() to read one complete message per line.
        """
        request = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
        }
        if params:
            request["params"] = params
        
        self.request_id += 1
        
        # Send as single-line JSON (required by MCP stdio protocol)
        request_json = json.dumps(request) + "\n"
        print(f"\n→ Sending: {method}")
        print(f"  Request: {json.dumps(request, indent=2)}")  # Pretty print for display only
        
        if not self.process or not self.process.stdin:
            raise RuntimeError("Server not started")
        
        self.process.stdin.write(request_json)
        self.process.stdin.flush()
        
        # Read response - wait a bit for server to process
        time.sleep(0.2)
        
        # Try to read from stdout with timeout
        import select
        import sys
        
        # Check if there's data available (non-blocking check)
        if hasattr(self.process.stdout, 'fileno'):
            import select
            ready, _, _ = select.select([self.process.stdout], [], [], 1.0)
            if not ready:
                # Check stderr for errors
                if self.process.stderr:
                    ready_err, _, _ = select.select([self.process.stderr], [], [], 0.1)
                    if ready_err:
                        stderr_line = self.process.stderr.readline()
                        if stderr_line:
                            print(f"  Server stderr: {stderr_line.strip()}")
                raise RuntimeError("No response from server (timeout)")
        
        # Read single-line JSON-RPC response (MCP stdio protocol requirement)
        response_line = self.process.stdout.readline()
        if not response_line:
            raise RuntimeError("No response from server")
        
        # Parse the single-line JSON response
        response = json.loads(response_line.strip())
        print(f"← Response: {json.dumps(response, indent=2)}")  # Pretty print for display only
        
        # Check for error response
        if "error" in response:
            print(f"⚠ Server returned error: {response['error']}")
        
        return response

    def send_notification(self, method: str, params: Dict[str, Any] = None):
        """Send a JSON-RPC notification (no response expected)."""
        notification = {
            "jsonrpc": "2.0",
            "method": method,
        }
        if params:
            notification["params"] = params
        
        notification_json = json.dumps(notification) + "\n"
        print(f"\n→ Sending notification: {method}")
        print(f"  Notification: {json.dumps(notification, indent=2)}")
        
        if not self.process or not self.process.stdin:
            raise RuntimeError("Server not started")
        
        self.process.stdin.write(notification_json)
        self.process.stdin.flush()
        time.sleep(0.1)  # Give server time to process

    def test_initialize(self):
        """Test initialize request."""
        print("\n" + "="*60)
        print("TEST 1: Initialize")
        print("="*60)
        
        response = self.send_request("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        })
        
        assert response.get("jsonrpc") == "2.0", "Invalid JSON-RPC version"
        assert "result" in response, "No result in response"
        assert response["result"].get("protocolVersion") == "2024-11-05", "Wrong protocol version"
        assert "capabilities" in response["result"], "No capabilities in result"
        assert "serverInfo" in response["result"], "No serverInfo in result"
        
        print("✓ Initialize test passed")
        return response["result"]

    def test_initialized_notification(self):
        """Test initialized notification."""
        print("\n" + "="*60)
        print("TEST 2: Initialized Notification")
        print("="*60)
        
        self.send_notification("notifications/initialized")
        time.sleep(0.2)  # Give server time to process notification
        print("✓ Initialized notification sent")

    def test_list_tools(self):
        """Test tools/list request."""
        print("\n" + "="*60)
        print("TEST 3: List Tools")
        print("="*60)
        
        response = self.send_request("tools/list")
        
        assert response.get("jsonrpc") == "2.0", "Invalid JSON-RPC version"
        assert "result" in response, "No result in response"
        assert "tools" in response["result"], "No tools in result"
        
        tools = response["result"]["tools"]
        print(f"\n✓ Found {len(tools)} tools:")
        for tool in tools:
            print(f"  - {tool.get('name')}: {tool.get('description', '')[:60]}...")
        
        assert len(tools) > 0, "No tools available"
        
        # Verify expected tools
        tool_names = [tool["name"] for tool in tools]
        expected_tools = [
            "list_notes_directory",
            "read_notes_file",
            "delete_notes_item",
            "create_or_update_note",
            "get_daily_note",
            "search_vault",
            "find_related_notes",
            "edit_note_text",
            "create_note_from_template",
            "list_notes_templates"
        ]
        
        print(f"\nFound tools: {', '.join(tool_names)}")
        for expected in expected_tools:
            assert expected in tool_names, f"Missing tool: {expected}"
        
        print("✓ All expected tools found")
        return tools

    def test_list_notes_templates(self, tools):
        """Test list_notes_templates tool."""
        print("\n" + "="*60)
        print("TEST 4: List Notes Templates")
        print("="*60)
        
        tool = next((t for t in tools if t["name"] == "list_notes_templates"), None)
        assert tool is not None, "list_notes_templates tool not found"
        
        response = self.send_request("tools/call", {
            "name": "list_notes_templates",
            "arguments": {}
        })
        
        assert response.get("jsonrpc") == "2.0", "Invalid JSON-RPC version"
        assert "result" in response, "No result in response"
        assert "content" in response["result"], "No content in result"
        
        content = response["result"]["content"]
        assert len(content) > 0, "No content items"
        assert content[0].get("type") == "text", "Invalid content type"
        
        # Parse the JSON response
        import json
        templates = json.loads(content[0].get("text", "[]"))
        print(f"\n✓ Found {len(templates)} templates:")
        for template in templates[:5]:  # Show first 5
            print(f"  - {template.get('name')} ({template.get('path')})")
        
        print("✓ List notes templates test passed")
        return response["result"]

    def test_list_notes_directory(self, tools):
        """Test list_notes_directory tool."""
        print("\n" + "="*60)
        print("TEST 5: List Notes Directory")
        print("="*60)
        
        tool = next((t for t in tools if t["name"] == "list_notes_directory"), None)
        assert tool is not None, "list_notes_directory tool not found"
        
        response = self.send_request("tools/call", {
            "name": "list_notes_directory",
            "arguments": {
                "path": ".",
                "limit": 10
            }
        })
        
        assert response.get("jsonrpc") == "2.0", "Invalid JSON-RPC version"
        assert "result" in response, "No result in response"
        assert "content" in response["result"], "No content in result"
        
        content = response["result"]["content"]
        assert len(content) > 0, "No content items"
        assert content[0].get("type") == "text", "Invalid content type"
        
        # Parse the JSON response
        import json
        items = json.loads(content[0].get("text", "[]"))
        print(f"\n✓ Found {len(items)} items in directory:")
        for item in items[:10]:  # Show first 10
            item_type = "file" if item.get("is_file") else "directory"
            size_str = f" ({item.get('size', 0)} bytes)" if item.get("is_file") else ""
            print(f"  - {item.get('name')} ({item_type}){size_str}")
        
        print("✓ List notes directory test passed")
        return response["result"]

    def run_all_tests(self):
        """Run all tests in sequence."""
        try:
            self.start_server()
            time.sleep(0.5)  # Give server time to start
            
            # Test protocol flow
            init_result = self.test_initialize()
            time.sleep(0.1)
            
            # Check if server is still alive
            if self.process.poll() is not None:
                print(f"\n✗ Server terminated after initialize (exit code: {self.process.returncode})")
                return False
            
            # Send initialized notification (required by rmcp)
            self.test_initialized_notification()
            time.sleep(0.2)  # Give server time to process
            
            # Check if server is still alive
            if self.process.poll() is not None:
                print(f"\n✗ Server terminated after initialized notification (exit code: {self.process.returncode})")
                if self.process.stderr:
                    try:
                        stderr_output = self.process.stderr.read()
                        if stderr_output:
                            print("\n--- Server stderr output ---")
                            print(stderr_output.decode('utf-8', errors='ignore'))
                    except:
                        pass
                return False
            
            tools = self.test_list_tools()
            self.test_list_notes_templates(tools)
            self.test_list_notes_directory(tools)
            
            print("\n" + "="*60)
            print("✓ ALL TESTS PASSED!")
            print("="*60)
            return True
            
        except AssertionError as e:
            print(f"\n✗ TEST FAILED: {e}")
            return False
        except BrokenPipeError as e:
            print(f"\n✗ Server connection broken: {e}")
            if self.process and self.process.stderr:
                try:
                    stderr_output = self.process.stderr.read()
                    if stderr_output:
                        print("\n--- Server stderr output ---")
                        print(stderr_output.decode('utf-8', errors='ignore'))
                except:
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
    # Determine server path
    script_dir = os.path.dirname(os.path.abspath(__file__))
    server_path = os.path.join(script_dir, "target", "debug", "mcp_obsidian_notes")
    
    # Check if release build exists, prefer it
    release_path = os.path.join(script_dir, "target", "release", "mcp_obsidian_notes")
    if os.path.exists(release_path):
        server_path = release_path
    
    if not os.path.exists(server_path):
        print(f"Error: Server binary not found at {server_path}")
        print("Please build the server first: cargo build")
        sys.exit(1)
    
    # Vault location - use environment variable or default to a placeholder
    vault_location = os.environ.get("VAULT_LOCATION")
    
    if not vault_location:
        print("Error: VAULT_LOCATION environment variable must be set")
        print("Example: export VAULT_LOCATION=/path/to/your/vault")
        sys.exit(1)
    
    if not os.path.exists(vault_location):
        print(f"Warning: Vault location does not exist: {vault_location}")
        print("The server may fail to initialize.")
    
    tester = MCPServerTester(server_path, vault_location)
    success = tester.run_all_tests()
    
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

