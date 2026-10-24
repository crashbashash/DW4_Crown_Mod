#!/usr/bin/env python3
"""Drive the real desktop UI through the opt-in agent-control build."""
import argparse
import json
import os
from pathlib import Path
import secrets
import subprocess
import urllib.request


def evaluate(session, script):
    info = json.loads(Path(session).read_text())
    request = urllib.request.Request(
        f"http://127.0.0.1:{info['port']}/eval",
        data=json.dumps({"script": script}).encode(),
        headers={"Authorization": f"Bearer {info['token']}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=15) as response:
        result = json.load(response)
    if not result["ok"]:
        raise RuntimeError(result["error"])
    return result.get("value")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--session", required=True, help="private session JSON outside the repository")
    commands = parser.add_subparsers(dest="command", required=True)
    start = commands.add_parser("start")
    start.add_argument("--binary", required=True)
    start.add_argument("--port", type=int, default=17364)
    commands.add_parser("snapshot")
    screenshot = commands.add_parser("screenshot")
    screenshot.add_argument("path")
    fill = commands.add_parser("fill")
    fill.add_argument("label")
    fill.add_argument("value")
    click = commands.add_parser("click")
    click.add_argument("text")
    option = commands.add_parser("option")
    option.add_argument("label")
    option.add_argument("state", choices=("on", "off"))
    script = commands.add_parser("eval")
    script.add_argument("script")
    args = parser.parse_args()
    if args.command == "start":
        path = Path(args.session)
        # Never overwrite another agent's credentials/session.
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        token = secrets.token_urlsafe(32)
        env = dict(os.environ, DW4_AGENT_TOKEN=token, DW4_AGENT_PORT=str(args.port))
        try:
            with open(str(path) + ".log", "wb") as log:
                child = subprocess.Popen([str(Path(args.binary).resolve())], env=env,
                                         stdout=log, stderr=log, start_new_session=True)
            with os.fdopen(fd, "w") as session:
                json.dump({"port": args.port, "token": token, "pid": child.pid}, session)
            print(json.dumps({"pid": child.pid, "session": str(path)}))
        except Exception:
            path.unlink(missing_ok=True)
            raise
        return
    if args.command == "screenshot":
        info = json.loads(Path(args.session).read_text())
        windows = subprocess.check_output(["xdotool", "search", "--onlyvisible", "--pid", str(info["pid"]), "--name", "^DW4 Crown Mod$"], text=True).splitlines()
        if len(windows) != 1:
            raise RuntimeError("expected one visible window for the selected app session")
        subprocess.run(["import", "-window", windows[0], args.path], check=True)
        print(json.dumps({"screenshot": args.path}))
        return
    if args.command == "snapshot":
        script = """({text: document.body.innerText, inputs: [...document.querySelectorAll('input')].map(e =>
          ({label: e.getAttribute('aria-label'), type: e.type, value: e.value, checked: e.checked})),
          buttons: [...document.querySelectorAll('button')].map(e => ({text:e.innerText, disabled:e.disabled}))})"""
    elif args.command == "fill":
        script = """(() => {
          const e = [...document.querySelectorAll('input')].find(e => e.getAttribute('aria-label') === LABEL);
          if (!e) throw Error('input missing');
          e.focus();
          Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(e, VALUE);
          e.dispatchEvent(new Event('input', {bubbles:true}));
          e.dispatchEvent(new Event('change', {bubbles:true}));
          e.blur(); return true;
        })()""".replace("LABEL", json.dumps(args.label)).replace("VALUE", json.dumps(args.value))
    elif args.command == "option":
        script = """(() => {
          const e = [...document.querySelectorAll('input[type=checkbox]')].find(e =>
            e.closest('label')?.querySelector('.setting-title')?.textContent.trim() === LABEL);
          if (!e) throw Error('option missing');
          if (e.disabled) throw Error('option disabled');
          if (e.checked !== SELECTED) e.click();
          return true;
        })()""".replace("LABEL", json.dumps(args.label)).replace("SELECTED", json.dumps(args.state == "on"))
    elif args.command == "click":
        script = """(() => {
          const matches = [...document.querySelectorAll('button')].filter(e => e.innerText.trim() === TEXT);
          if (matches.length !== 1) throw Error('expected one matching button');
          if (matches[0].disabled) throw Error('button disabled');
          matches[0].click(); return true;
        })()""".replace("TEXT", json.dumps(args.text))
    else:
        script = args.script
    print(json.dumps(evaluate(args.session, script), indent=2))


if __name__ == "__main__":
    main()
