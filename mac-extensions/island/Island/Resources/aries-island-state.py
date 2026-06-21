"""
Aries Island Hook
- Sends session state to Aries Island.app via Unix socket
"""

import json
import os
import socket
import sys

SOCKET_PATH = "/tmp/aries-island.sock"


def get_tty():
    """Get the TTY of the aries process (parent)"""
    import subprocess

    ppid = os.getppid()

    try:
        result = subprocess.run(
            ["ps", "-p", str(ppid), "-o", "tty="],
            capture_output=True,
            text=True,
            timeout=2,
        )
        tty = result.stdout.strip()
        if tty and tty != "??" and tty != "-":
            if not tty.startswith("/dev/"):
                tty = "/dev/" + tty
            return tty
    except Exception:
        pass

    try:
        return os.ttyname(sys.stdin.fileno())
    except (OSError, AttributeError):
        pass
    try:
        return os.ttyname(sys.stdout.fileno())
    except (OSError, AttributeError):
        pass
    return None


def send_event(state):
    """Send event to app (fire and forget)"""
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(5)
        sock.connect(SOCKET_PATH)
        sock.sendall(json.dumps(state).encode())
        sock.close()
    except (socket.error, OSError):
        pass


def main():
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError:
        sys.exit(1)

    session_id = data.get("session_id", "unknown")
    event = data.get("hook_event_name", "")
    cwd = data.get("cwd", "")
    tool_input = data.get("tool_input", {})

    aries_pid = os.getppid()
    tty = get_tty()

    state = {
        "session_id": session_id,
        "cwd": cwd,
        "event": event,
        "pid": aries_pid,
        "tty": tty,
    }

    if event == "UserPromptSubmit":
        state["status"] = "processing"
        prompt = data.get("prompt") or ""
        if prompt:
            state["prompt"] = prompt

    elif event == "PreToolUse":
        state["status"] = "running_tool"
        state["tool"] = data.get("tool_name")
        state["tool_input"] = tool_input
        tool_use_id_from_event = data.get("tool_use_id")
        if tool_use_id_from_event:
            state["tool_use_id"] = tool_use_id_from_event

    elif event == "PostToolUse":
        state["status"] = "processing"
        state["tool"] = data.get("tool_name")
        state["tool_input"] = tool_input
        tool_use_id_from_event = data.get("tool_use_id")
        if tool_use_id_from_event:
            state["tool_use_id"] = tool_use_id_from_event

    elif event == "PostToolUseFailure":
        state["status"] = "processing"
        state["tool"] = data.get("tool_name")
        state["tool_input"] = tool_input
        state["tool_error"] = data.get("error") or data.get("message")
        tool_use_id_from_event = data.get("tool_use_id")
        if tool_use_id_from_event:
            state["tool_use_id"] = tool_use_id_from_event

    elif event == "Stop":
        state["status"] = "waiting_for_input"
        last_msg = data.get("last_assistant_message") or ""
        if last_msg:
            state["agent_response"] = last_msg

    elif event == "StopFailure":
        state["status"] = "waiting_for_input"
        last_msg = data.get("last_assistant_message") or ""
        if last_msg:
            state["agent_response"] = last_msg
        state["stop_error"] = data.get("error") or data.get("message")

    elif event == "SubagentStart":
        state["status"] = "processing"

    elif event == "SubagentStop":
        state["status"] = "processing"

    elif event == "SessionStart":
        state["status"] = "waiting_for_input"

    elif event == "SessionEnd":
        state["status"] = "ended"

    elif event == "PreCompact":
        state["status"] = "compacting"

    elif event == "PostCompact":
        state["status"] = "processing"

    else:
        state["status"] = "unknown"

    send_event(state)


if __name__ == "__main__":
    try:
        main()
    except Exception:
        exit(0)
