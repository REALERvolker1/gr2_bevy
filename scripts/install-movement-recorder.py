#!/usr/bin/env python3
"""Install only the authored recorder, preserving the working UE4SS loader."""

import argparse
import hashlib
import json
import logging
import os
import shutil
import uuid
from datetime import UTC, datetime
from pathlib import Path

from local_config import LocalConfig
from local_config import load as load_local_config

ROOT = Path(__file__).resolve().parents[1]
MOD = "GR2MovementRecorder"
GAME_SHA256 = "3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85"
# Non-machine sentinel in the tracked Lua template; install() replaces it.
OUTPUT_PLACEHOLDER = '"Z:/gr2-traces"'
LOG = logging.getLogger("gr2.recorder.install")


def digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def require_stopped() -> None:
    for process in Path("/proc").iterdir():
        if not process.name.isdecimal():
            continue
        try:
            name = (process / "comm").read_text().strip()
            if not name.startswith(("Ghostrunner", "wine")):
                continue
            command = (process / "cmdline").read_bytes()
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        if b"Ghostrunner2-Win64-Shipping.exe" in command:
            raise ValueError(f"Close Ghostrunner before installing; game PID {process.name}")


def verify_existing(destination: Path) -> None:
    # Keep dynamic JSON at the boundary; never overwrite locally edited scripts.
    manifest: object = json.loads((destination / "installation.json").read_text())
    if not isinstance(manifest, dict):
        raise TypeError("Existing installation manifest is not an object")
    files: object = manifest.get("installed_files")
    if not isinstance(files, dict):
        raise TypeError("Existing installation has no file hashes")
    for name, expected in files.items():
        if not isinstance(name, str) or Path(name).name != name or not isinstance(expected, str):
            raise ValueError("Invalid installed-file hash entry")
        if digest(destination / "Scripts" / name) != expected:
            raise ValueError(f"Preserve locally modified recorder script: {name}")


def install(config: LocalConfig, game: Path, output: Path, update: bool = False) -> Path:
    require_stopped()
    executable = game / "Ghostrunner2-Win64-Shipping.exe"
    if digest(executable) != GAME_SHA256:
        raise ValueError("Installed binary differs from the researched build; qualify it before recording")
    for name in ("UE4SS.dll", "UE4SS-settings.ini", "dwmapi.dll", "Mods/mods.txt"):
        if not (game / name).is_file():
            raise ValueError(f"Working flat UE4SS installation missing {name}")
    destination = game / "Mods" / MOD
    if destination.exists() and not update:
        raise ValueError(f"Preserve existing recorder installation: {destination}")
    if update:
        verify_existing(destination)
    mods_path = game / "Mods/mods.txt"
    original = mods_path.read_bytes()
    present = any(line.split(":", 1)[0].strip() == MOD for line in original.decode().splitlines())
    if present and not update:
        raise ValueError("Existing recorder load-order entry needs review")
    if update and not present:
        raise ValueError("Existing recorder load-order entry missing")
    if not any(
        output.resolve().is_relative_to(root)
        for root in (ROOT / ".work", config.trace_dir)
    ):
        raise ValueError("Captures must stay under workspace .work or the configured trace_dir")
    output.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%S.%fZ")
    backup = ROOT / ".work/lab/backups" / f"movement-recorder-{stamp}"
    backup.mkdir(parents=True)
    shutil.copy2(mods_path, backup / "mods.txt")
    shutil.copy2(game / "UE4SS-settings.ini", backup / "UE4SS-settings.ini")
    # Save/config backups exclude regenerated crash reports, shader caches and logs.
    prefixes = config.compatdata_dirs
    for index, prefix in enumerate(prefixes):
        saved = prefix / "pfx/drive_c/users/steamuser/AppData/Local/Ghostrunner2/Saved"
        for name in ("SaveGames", "Config"):
            if (saved / name).is_dir():
                shutil.copytree(saved / name, backup / f"prefix-{index}" / name)
    staging = game / "Mods" / f".{MOD}-{uuid.uuid4().hex}"
    source = ROOT / "config/ue4ss" / MOD / "Scripts"
    if update:
        shutil.copytree(destination, backup / MOD)
        verify_existing(backup / MOD)
        shutil.copytree(destination, staging)
    else:
        staging.mkdir()
        (staging / "Scripts").mkdir()
    for path in source.glob("*.lua"):
        shutil.copy2(path, staging / "Scripts" / path.name)
    settings = staging / "Scripts/gr2_settings.lua"
    text = settings.read_text()
    if text.count(OUTPUT_PLACEHOLDER) != 1:
        raise ValueError("Recorder output setting no longer matches installer template")
    settings.write_text(
        text.replace(OUTPUT_PLACEHOLDER, json.dumps("Z:" + str(output.resolve())))
    )
    manifest = {
        "installed_at": stamp,
        "game": str(game),
        "game_sha256": GAME_SHA256,
        "output": str(output.resolve()),
        "source_files": {path.name: digest(path) for path in source.glob("*.lua")},
        "installed_files": {path.name: digest(path) for path in (staging / "Scripts").glob("*.lua")},
        "loader_files": {name: digest(game / name) for name in ("UE4SS.dll", "dwmapi.dll", "UE4SS-settings.ini")},
        "mods_before_sha256": hashlib.sha256(original).hexdigest(),
        "backup": str(backup),
        "prefix_backups": [str(prefix) for prefix in prefixes],
        "live_capture_verified": False,
    }
    (backup / "installation.json").write_text(json.dumps(manifest, indent=2) + "\n")
    shutil.copy2(backup / "installation.json", staging / "installation.json")
    rollback = game / "Mods" / f".{MOD}-rollback-{uuid.uuid4().hex}"
    if update:
        # Both renames must stay on the game filesystem; the durable backup
        # can live on NFS and was copied/verified before touching the live mod.
        destination.rename(rollback)
    try:
        staging.rename(destination)
    except OSError:
        if update:
            rollback.rename(destination)
        raise
    if update:
        shutil.rmtree(rollback)
    newline = b"\r\n" if b"\r\n" in original else b"\n"
    updated = original if update else original.rstrip(b"\r\n") + newline + f"{MOD} : 1".encode() + newline
    temporary = mods_path.with_name(f".mods-{uuid.uuid4().hex}.txt")
    temporary.write_bytes(updated)
    os.replace(temporary, mods_path)
    LOG.info("Installed %s; output %s; backup %s", destination, output, backup)
    return backup


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--game",
        type=Path,
        help="Win64 binaries directory; defaults to game_dir in config/local.toml",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="capture directory; defaults to trace_dir in config/local.toml",
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="Verify and back up an existing installation before replacing it",
    )
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    try:
        config = load_local_config()
        game = (args.game or config.game_dir).resolve()
        output = (args.output or config.trace_dir).resolve()
        install(config, game, output, args.update)
    except (OSError, ValueError, TypeError) as error:
        parser.exit(1, f"Recorder installation failed: {error}\n")


if __name__ == "__main__":
    main()
