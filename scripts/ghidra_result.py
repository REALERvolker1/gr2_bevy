"""Validate text results from the installed Ghidra MCP boundary."""

from mcp.types import CallToolResult, TextContent

from json_data import parse_object


def error_text(error: BaseException) -> str:
    if isinstance(error, BaseExceptionGroup):
        return "; ".join(error_text(child) for child in error.exceptions)
    return str(error)


def result_text(result: CallToolResult) -> str:
    text = "\n".join(block.text for block in result.content if isinstance(block, TextContent))
    if result.isError or not text:
        raise ValueError(f"MCP operation failed: {text}")
    if text.lstrip().startswith("{") and "error" in parse_object(text):
        raise ValueError(f"Ghidra operation failed: {text}")
    return text
