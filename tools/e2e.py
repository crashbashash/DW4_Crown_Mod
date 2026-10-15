#!/usr/bin/env python3
"""Exercise the real desktop UI with a supplied clean ISO; writes only new test files."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

AGENT = Path(__file__).with_name("agent.py")


def md5(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "md5").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--session", required=True)
    parser.add_argument("--iso", required=True, type=Path)
    parser.add_argument("--expected-md5", required=True)
    parser.add_argument("--region", choices=("usa", "europe", "japan"), default="usa")
    parser.add_argument("--timeout", type=float, default=300)
    args = parser.parse_args()
    iso = args.iso.resolve()
    folder = Path(tempfile.mkdtemp(prefix="dw4vhp-e2e-"))
    output = folder / "patched.iso"
    failures = folder / "invalid.iso"
    failures.write_bytes(b"not a PS2 disc image")
    checks = []

    def drive(*command):
        return json.loads(subprocess.check_output(
            [sys.executable, str(AGENT), "--session", args.session, *command], text=True))

    def wait(predicate):
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            snapshot = drive("snapshot")
            if predicate(snapshot):
                return snapshot
            time.sleep(0.25)
        raise AssertionError(f"UI timed out: {snapshot}")

    def text_is(fragment):
        return wait(lambda s: fragment in s["text"])

    def enabled(s):
        return any(b["text"] == "Patch ISO" and not b["disabled"] for b in s["buttons"])

    def record(name):
        checks.append(name)
        print(f"PASS: {name}", flush=True)

    print(f"Artifacts: {folder}", flush=True)
    before = md5(iso)
    try:
        initial = drive("snapshot")
        assert not enabled(initial)
        assert not any(name in initial["text"] for name in ("Extreme", "Brutal", "Custom", "Advanced"))
        assert not any(i["type"] == "checkbox" for i in initial["inputs"])
        record("base mode only; patch disabled without a disc")

        drive("fill", "disc path", str(failures))
        text_is("this is not a supported clean PS2 disc")
        assert not enabled(drive("snapshot"))
        record("invalid disc refused")

        drive("fill", "disc path", str(iso))
        text_is({"usa": "clean Digimon World 4 (USA)",
                 "europe": "clean Digimon World 4 (Europe)",
                 "japan": "clean Digimon World X (Japan)"}[args.region])
        wait(enabled)
        assert f"{iso.stat().st_size / 1e9:.2f} GB of free space" in drive("snapshot")["text"]
        assert "pinned" not in drive("snapshot")["text"]
        drive("fill", "output path", str(output))
        drive("click", "Patch ISO")
        done = text_is("GB, md5 " + args.expected_md5.lower())
        wait(enabled)
        assert output.stat().st_size == iso.stat().st_size
        assert md5(output) == args.expected_md5.lower()
        assert md5(iso) == before
        assert not Path(str(output) + ".part").exists()
        drive("screenshot", str(folder / "success.png"))
        (folder / "success-ui.json").write_text(json.dumps(done, indent=2))
        record("native UI patch matches reference MD5; original unchanged; no partial file")

        drive("click", "Patch ISO")
        text_is("Overwrite existing file?")
        assert "Replace it with the patched ISO?" in drive("snapshot")["text"]
        drive("screenshot", str(folder / "overwrite-question.png"))
        drive("click", "Cancel")
        assert "Overwrite existing file?" not in drive("snapshot")["text"]
        assert md5(output) == args.expected_md5.lower()
        record("existing output asks; cancelling preserves the file")

        drive("click", "Patch ISO")
        text_is("Overwrite existing file?")
        before_logs = drive("eval", "document.querySelectorAll('.log-line').length")
        drive("click", "Overwrite and patch")
        wait(lambda s: enabled(s) and drive("eval", "document.querySelectorAll('.log-line').length") > before_logs)
        latest = drive("eval", "document.querySelector('.log-line:last-child').innerText")
        assert latest.startswith("Patched "), latest
        assert md5(output) == args.expected_md5.lower()
        record("confirmed overwrite patches successfully")

        drive("fill", "output path", str(iso))
        drive("click", "Patch ISO")
        text_is("Overwrite existing file?")
        drive("click", "Overwrite and patch")
        text_is("the output path is the same file as the input")
        wait(enabled)
        assert md5(iso) == before
        record("output equal to input refused; original unchanged")

        drive("fill", "disc path", str(output))
        text_is("already")
        assert not enabled(drive("snapshot"))
        record("already-modded ISO refused")

        report = {"passed": checks, "originalMd5": before, "outputMd5": md5(output),
                  "output": str(output), "finalUi": drive("snapshot")}
        (folder / "report.json").write_text(json.dumps(report, indent=2))
        print(f"Artifacts: {folder}", flush=True)
    except Exception:
        (folder / "failure-ui.json").write_text(json.dumps(drive("snapshot"), indent=2))
        print(f"Failure artifacts: {folder}", flush=True)
        raise


if __name__ == "__main__":
    main()
