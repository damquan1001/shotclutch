"""Validate the test-only scheduling repair; Windows two-CPU stress is explicit."""
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import time

root = Path(__file__).resolve().parents[5]
out = Path(__file__).resolve().parent
report = {
    "parent_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
    "working_tree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root).strip()),
    "source_file": "crates/export/src/determinism_tests.rs",
    "source_sha256": hashlib.sha256((root / "crates/export/src/determinism_tests.rs").read_bytes()).hexdigest(),
    "os": platform.platform(), "logical_cpus": os.cpu_count(),
    "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
    "commands": [], "result": "failed",
    "qualification_limit": "Local test validation only; full hosted repeats remain pending account unblock.",
}

def run(command, name):
    started = time.monotonic()
    result = subprocess.run(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    (out / name).write_bytes(result.stdout)
    text = result.stdout.decode("utf-8", errors="replace")
    counts = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", text)
    report["commands"].append({"command": command, "log": name, "exit_code": result.returncode,
        "wall_seconds": time.monotonic() - started, "test_results": counts})
    print(name, "exit", result.returncode, flush=True)
    (out / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if result.returncode:
        raise RuntimeError(text[-5000:])
    return text

try:
    run(["cargo", "fmt", "--all", "--", "--check"], "fmt.log")
    text = run(["cargo", "test", "--locked", "--release", "-p", "filmcraft-export", "--lib", "--no-run", "--message-format=json"], "build.log")
    artifacts = [json.loads(line) for line in text.splitlines() if line.startswith("{")]
    executable = next(a["executable"] for a in artifacts if a.get("reason") == "compiler-artifact" and a.get("executable") and a.get("target", {}).get("name") == "filmcraft_export")
    run([executable, "determinism_tests::", "--test-threads=1", "--nocapture"], "determinism.log")
    if os.name != "nt":
        raise RuntimeError("Two-CPU stress requires Windows; do not claim this profile elsewhere")
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.GetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
    kernel.SetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.c_size_t]
    handle = kernel.GetCurrentProcess()
    original, system = ctypes.c_size_t(), ctypes.c_size_t()
    if not kernel.GetProcessAffinityMask(handle, ctypes.byref(original), ctypes.byref(system)):
        raise ctypes.WinError(ctypes.get_last_error())
    bits = [1 << bit for bit in range(ctypes.sizeof(ctypes.c_size_t) * 8) if original.value & (1 << bit)]
    assert len(bits) >= 2
    mask = bits[0] | bits[1]
    report["stress_affinity"] = {"original_mask": hex(original.value), "test_mask": hex(mask), "repetitions": 20,
        "scope": "Validator and its test children only; restored before Clippy"}
    if not kernel.SetProcessAffinityMask(handle, mask):
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        for index in range(20):
            run([executable, "determinism_tests::max_threads", "--test-threads=1", "--nocapture"], f"stress-{index + 1:02}.log")
    finally:
        if not kernel.SetProcessAffinityMask(handle, original.value):
            raise ctypes.WinError(ctypes.get_last_error())
    run(["cargo", "clippy", "--locked", "--release", "-p", "filmcraft-export", "--all-targets", "--", "-D", "warnings"], "clippy.log")
    report["result"] = "passed"
finally:
    (out / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
