#!/usr/bin/env python3
"""Copy closed local recorder captures into ignored workspace storage."""

import argparse
import hashlib
import logging
import re
import shutil
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOG = logging.getLogger("gr2.recorder.archive")


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
			if not (process / "comm").read_text().startswith("Ghostrunner"):
				continue
			if b"Ghostrunner2-Win64-Shipping.exe" in (process / "cmdline").read_bytes():
				raise ValueError("Close GR2 before archiving its captures")
		except (FileNotFoundError, ProcessLookupError, PermissionError):
			continue


def archive(source: Path, destination: Path) -> int:
	require_stopped()
	if not source.is_dir():
		raise ValueError(f"Capture source missing: {source}")
	if not destination.resolve().is_relative_to(ROOT / ".work"):
		raise ValueError("Raw archived captures must remain in workspace .work")
	destination.mkdir(parents=True, exist_ok=True)
	copied = 0
	for path in sorted(source.glob("gr2-*.jsonl")):
		if not re.fullmatch(r"gr2-\d{8}T\d{6}Z-\d{4,}\.jsonl", path.name):
			continue
		target = destination / path.name
		expected = digest(path)
		if target.exists():
			if digest(target) != expected:
				raise ValueError(f"Preserve conflicting archive: {target}")
			continue
		temporary = target.with_suffix(f".{uuid.uuid4().hex}.tmp")
		try:
			shutil.copy2(path, temporary)
			if digest(temporary) != expected:
				raise ValueError(f"Copy verification failed: {path}")
			temporary.rename(target)
		finally:
			temporary.unlink(missing_ok=True)
		copied += 1
		LOG.info("Archived %s", target)
	return copied


def main() -> None:
	parser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument(
		"--source",
		type=Path,
		default=Path.home() / ".local/state/gr2_bevy/traces/movement",
	)
	parser.add_argument(
		"--destination", type=Path, default=ROOT / ".work/traces/movement"
	)
	args = parser.parse_args()
	logging.basicConfig(level=logging.INFO, format="%(message)s")
	try:
		count = archive(args.source, args.destination)
		LOG.info("Copied %d captures; local originals retained", count)
	except (OSError, ValueError) as error:
		parser.exit(1, f"Capture archive failed: {error}\n")


if __name__ == "__main__":
	main()
