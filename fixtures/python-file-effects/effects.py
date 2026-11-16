from pathlib import Path


def record(action):
    path = Path("artifact.bin")
    if action == "create":
        path.write_bytes(b"first\x00\xff")
    elif action == "change":
        path.write_bytes(b"second\r\n")
    elif action == "delete":
        path.unlink()
    elif action == "error":
        path.write_bytes(b"failed\n")
        raise ValueError("after write")
    return 1
