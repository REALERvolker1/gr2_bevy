#!/usr/bin/env python3
"""Initialize MCP and read a loaded program; optionally decompile the toy probe."""

import argparse
import asyncio
import json

from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client

from ghidra_result import result_text
from json_data import parse_object, string


async def check(url: str, expected: str | None, probe: bool) -> None:
    async with streamablehttp_client(url) as (read, write, _):
        async with ClientSession(read, write) as session:
            initialized = await session.initialize()
            catalog = await session.list_tools()
            names = {tool.name for tool in catalog.tools}
            if not {"get_current_program_info", "list_functions"}.issubset(names):
                raise ValueError("endpoint does not expose the installed Ghidra tool API")
            info = parse_object(result_text(await session.call_tool("get_current_program_info", {})))
            name = string(info["name"])
            if expected is not None and name != expected:
                raise ValueError(f"wrong program: expected {expected}, got {name}")
            if probe:
                if name != "research_probe":
                    raise ValueError("probe mode requires the authored research_probe binary")
                functions = result_text(await session.call_tool("list_functions", {"program": name}))
                address = next(
                    (line.split(" at ", 1)[1] for line in functions.splitlines() if line.startswith("research_probe at ")),
                    None,
                )
                if address is None:
                    raise ValueError("probe function was not analyzed")
                body = result_text(await session.call_tool("decompile_function", {"address": address, "program": name}))
                if "research_probe" not in body or "7" not in body:
                    raise ValueError("probe decompilation does not match the authored function")
            print(json.dumps({"server": initialized.serverInfo.name, "tools": len(names), "program": name, "probe_decompiled": probe}))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="http://127.0.0.1:8081/mcp")
    parser.add_argument("--expected-program")
    parser.add_argument("--probe", action="store_true")
    args = parser.parse_args()
    try:
        asyncio.run(asyncio.wait_for(check(args.url, args.expected_program, args.probe), timeout=60))
    except (ValueError, OSError, TimeoutError, ExceptionGroup) as error:
        parser.exit(1, f"MCP health failed: {error}\nStart Ghidra and its bridge as documented in docs/setup.md.\n")


if __name__ == "__main__":
    main()
