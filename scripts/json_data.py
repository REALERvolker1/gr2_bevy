"""Narrow untrusted JSON at the boundary without leaking dynamic types."""

import json
import math


def object_map(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise ValueError("expected a JSON object with string keys")
    return {str(key): item for key, item in value.items()}


def string(value: object) -> str:
    if not isinstance(value, str):
        raise ValueError("expected a string")
    return value


def strings(value: object) -> list[str]:
    if not isinstance(value, list):
        raise ValueError("expected a string array")
    return [string(item) for item in value]


def parse_object(text: str) -> dict[str, object]:
    def reject_nonfinite(token: str) -> float:
        raise ValueError(f"nonfinite JSON number: {token}")

    def finite_float(token: str) -> float:
        value = float(token)
        if not math.isfinite(value):
            raise ValueError(f"nonfinite JSON number: {token}")
        return value

    value: object = json.loads(text, parse_constant=reject_nonfinite, parse_float=finite_float)
    return object_map(value)
