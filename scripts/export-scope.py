#!/usr/bin/env python3
"""Export an authored Ghidra scope with build-qualified private provenance."""

import argparse
import asyncio
import csv
import hashlib
import json
import re
from datetime import datetime, timezone
from pathlib import Path

from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client

from ghidra_result import error_text, result_text
from json_data import object_map, parse_object, string

ROOT = Path(__file__).resolve().parents[1]


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


async def export(url: str, scope: Path, output: Path, build: str, expected_hash: str) -> None:
    output = output.resolve()
    scope = scope.resolve()
    if not scope.is_relative_to(ROOT):
        raise ValueError("authored scope must be inside the repository")
    if not output.is_relative_to(ROOT / ".work/decompiled"):
        raise ValueError("export directory must be below .work/decompiled")
    if (output / "manifest.json").exists():
        raise ValueError("preserve completed export; choose a new private output directory")
    with scope.open(newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["id", "rva", "reason"]:
            raise ValueError("scope columns must be id, rva, reason")
        rows = list(reader)
    ids: set[str] = set()
    for row in rows:
        if None in row or any(value is None for value in row.values()):
            raise ValueError("scope row has the wrong number of cells")
        if not re.fullmatch(r"[a-z0-9-]+", row["id"]) or row["id"] in ids:
            raise ValueError("scope IDs must be unique lowercase names")
        ids.add(row["id"])
        if not re.fullmatch(r"0x[0-9a-f]+", row["rva"]):
            raise ValueError(f"invalid RVA for {row['id']}")
    if not rows:
        raise ValueError("empty scope")
    async with streamablehttp_client(url) as (read, write, _):
        async with ClientSession(read, write) as session:
            initialized = await session.initialize()
            info = parse_object(result_text(await session.call_tool("get_current_program_info", {})))
            program = string(info["name"])
            binary = Path(string(info["executable_path"]))
            options = parse_object(result_text(await session.call_tool("get_program_options", {
                "group": "Program Information", "program": program,
            })))
            raw_options = options["options"]
            if not isinstance(raw_options, list):
                raise ValueError("missing imported program options")
            import_hashes: list[str] = []
            for raw in raw_options:
                option = object_map(raw)
                if option.get("name") == "Executable SHA256":
                    import_hashes.append(string(option["value"]))
            if import_hashes != [expected_hash]:
                raise ValueError("Ghidra import SHA-256 differs from expected build")
            if digest(binary) != expected_hash:
                raise ValueError("loaded program's source binary differs from expected SHA-256")
            base = int(string(info["image_base"]), 16)
            output.mkdir(parents=True, exist_ok=True)
            exports: list[dict[str, str]] = []
            calls: set[str] = set()
            for row in rows:
                address = f"{base + int(row['rva'], 16):x}"
                body = result_text(await session.call_tool("decompile_function", {
                    "address": address, "program": program, "timeout": 60,
                }))
                if "{" not in body or "}" not in body:
                    raise ValueError(f"not a decompiler body for {row['id']}")
                path = output / f"{row['id']}.c"
                temporary = path.with_suffix(".pending")
                temporary.write_text(body, encoding="utf-8")
                temporary.replace(path)
                exports.append({**row, "address": address, "path": str(path.relative_to(ROOT)),
                                "sha256": digest(path)})
                calls.update(re.findall(r"(?:FUN_|func_0x)([0-9a-f]+)\s*\(", body))
            # A manifest is written only after every requested member exports successfully.
            manifest: dict[str, object] = {
                "source_build": build, "module_sha256": expected_hash, "program": program,
                "image_base": hex(base), "scope": str(scope.resolve().relative_to(ROOT)),
                "scope_sha256": digest(scope), "tool": initialized.serverInfo.name,
                "tool_version": initialized.serverInfo.version, "decompile_timeout_seconds": 60,
                "recorded_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                "closure_complete": False, "exports": exports,
                "unresolved_address_name_hints": sorted(calls - {row["address"] for row in exports}),
                "dependency_limits": "Decompiler names are navigation hints; indirect calls and UE boundaries need manual review",
            }
            (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
            print(f"Exported {len(exports)} functions; dependency closure remains open")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("scope", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--build", required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--url", default="http://127.0.0.1:8081/mcp")
    args = parser.parse_args()
    try:
        asyncio.run(export(args.url, args.scope, args.output, args.build, args.sha256))
    except (OSError, ValueError, TimeoutError, ExceptionGroup) as error:
        parser.exit(1, f"Scope export failed: {error_text(error)}\n")


if __name__ == "__main__":
    main()
