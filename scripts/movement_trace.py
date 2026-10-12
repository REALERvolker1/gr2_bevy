"""Stream and causally reconstruct GR2 movement recorder JSONL captures."""

import math
from collections.abc import Iterator, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from types import MappingProxyType
from typing import Literal

from json_data import object_map, parse_object, string


@dataclass(frozen=True)
class ObservedValue:
    value: object
    world_time: float | None


@dataclass(frozen=True)
class ContextSnapshot:
    generation: int
    revision: int
    fields_by_object: Mapping[str, Mapping[str, ObservedValue]]


@dataclass(frozen=True)
class UpgradeSnapshot:
    status: Literal["known", "unknown", "not_recorded"]
    revision: int | None
    world_time: float | None
    state: Mapping[str, object] | None


@dataclass(frozen=True)
class MovementSample:
    session: str
    recorder_version: str
    manifest: Mapping[str, object]
    sequence: int
    generation: int
    record_kind: str
    raw: Mapping[str, object]
    context: ContextSnapshot
    upgrades: UpgradeSnapshot


@dataclass
class _GenerationState:
    context_revision: int
    context_objects: dict[str, Mapping[str, ObservedValue]]
    context_history: dict[int, ContextSnapshot]
    upgrade_revision: int
    upgrade_history: dict[int, UpgradeSnapshot]


def _integer(value: object, name: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise TypeError(f"expected integer {name}")
    return value


def _world_time(value: object) -> float | None:
    if value is False or value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError("expected numeric world_time or false")
    result = float(value)
    if not math.isfinite(result):
        raise ValueError("world_time must be finite")
    return result


def _freeze(value: object) -> object:
    """Freeze parsed JSON so snapshots cannot be changed through nested values."""
    if isinstance(value, dict):
        return MappingProxyType({str(key): _freeze(item) for key, item in value.items()})
    if isinstance(value, list):
        return tuple(_freeze(item) for item in value)
    return value


def _freeze_map(value: Mapping[str, object]) -> Mapping[str, object]:
    return MappingProxyType({key: _freeze(item) for key, item in value.items()})


def _version(value: object) -> tuple[str, int]:
    name = string(value)
    prefix = "GR2MovementRecorder/"
    if not name.startswith(prefix):
        raise ValueError(f"unsupported recorder version {name!r}")
    parts = name[len(prefix) :].split(".")
    if len(parts) != 2 or not all(part.isdecimal() for part in parts):
        raise ValueError(f"unsupported recorder version {name!r}")
    major, minor = int(parts[0]), int(parts[1])
    if major != 2 or minor not in (0, 1, 2, 3):
        raise ValueError(f"unsupported recorder version {name!r}")
    return name, minor


def iter_samples(paths: Sequence[Path]) -> Iterator[MovementSample]:
    """Yield sample and stationary-event rows from ordered segment paths.

    Context and upgrade revisions are retained per possession generation so
    buffered samples can resolve the revision they name. Snapshots share
    unchanged object maps; later deltas replace changed maps instead of
    mutating prior snapshots.
    """
    if not paths:
        raise ValueError("at least one movement segment is required")

    session: str | None = None
    recorder_version: str | None = None
    capture_manifest: Mapping[str, object] | None = None
    version_minor: int | None = None
    expected_sequence = 1
    current_generation: int | None = None
    generations: dict[int, _GenerationState] = {}
    manifest_seen = False

    for segment_index, path in enumerate(paths, 1):
        with path.open(encoding="utf-8") as source:
            for line_number, line in enumerate(source, 1):
                try:
                    record = parse_object(line)
                    kind = string(record.get("kind"))

                    if line_number == 1:
                        if kind != "segment":
                            raise ValueError("segment header must be the first record")
                        if string(record.get("schema")) != "gr2-movement-v2":
                            raise ValueError("unsupported movement trace schema")
                        if _integer(record.get("segment"), "segment") != segment_index:
                            raise ValueError("segments are missing or out of order")
                        record_session = string(record.get("session"))
                        if session is None:
                            session = record_session
                        elif record_session != session:
                            raise ValueError("segment session does not match manifest session")
                        continue

                    record_session_value = record.get("session")
                    if record_session_value is not None and string(record_session_value) != session:
                        raise ValueError("record session does not match segment session")

                    if kind == "manifest":
                        if segment_index != 1 or manifest_seen:
                            raise ValueError("manifest must appear once in the first segment")
                        if string(record.get("session")) != session:
                            raise ValueError("manifest session does not match segment session")
                        recorder_version, version_minor = _version(record.get("recorder"))
                        capture_manifest = _freeze_map(record)
                        manifest_seen = True
                    elif not manifest_seen:
                        raise ValueError("manifest must precede capture records")

                    if "sequence" not in record:
                        raise ValueError("record is missing sequence")
                    sequence = _integer(record["sequence"], "sequence")
                    if sequence != expected_sequence:
                        raise ValueError(
                            f"sequence discontinuity: expected {expected_sequence}, got {sequence}"
                        )
                    expected_sequence += 1

                    if kind == "writer_loss":
                        raise ValueError("writer_loss invalidates trace continuity")

                    if kind == "player":
                        generation = _integer(record.get("generation"), "generation")
                        if current_generation is not None and generation <= current_generation:
                            raise ValueError("player generation did not advance")
                        current_generation = generation
                        if generation in generations:
                            raise ValueError("player generation was acquired more than once")
                        empty_context = ContextSnapshot(
                            generation,
                            0,
                            MappingProxyType({}),
                        )
                        generations[generation] = _GenerationState(
                            0,
                            {},
                            {0: empty_context},
                            0,
                            {},
                        )
                    elif kind == "context_delta":
                        generation = _integer(record.get("generation"), "generation")
                        state = generations.get(generation)
                        if state is None:
                            raise ValueError("context delta has no matching player generation")
                        revision = _integer(record.get("revision"), "context revision")
                        if revision != state.context_revision + 1:
                            raise ValueError("context revision is missing or out of order")
                        object_id = string(record.get("id"))
                        values = object_map(record.get("values"))
                        removed = record.get("removed")
                        # The Lua encoder writes empty tables as JSON objects;
                        # a nonempty removal array is encoded as a JSON array.
                        if isinstance(removed, dict) and not removed:
                            removed_fields: list[str] = []
                        elif isinstance(removed, list):
                            removed_fields = []
                            for item in removed:
                                if not isinstance(item, str):
                                    raise TypeError("context removed must contain strings")
                                removed_fields.append(item)
                        else:
                            raise ValueError("context removed must be a string array")
                        timestamp = _world_time(record.get("world_time"))
                        fields = dict(state.context_objects.get(object_id, {}))
                        for field in removed_fields:
                            fields.pop(field, None)
                        for field, value in values.items():
                            fields[field] = ObservedValue(_freeze(value), timestamp)
                        state.context_objects = dict(state.context_objects)
                        state.context_objects[object_id] = MappingProxyType(fields)
                        state.context_revision = revision
                        state.context_history[revision] = ContextSnapshot(
                            generation,
                            revision,
                            MappingProxyType(dict(state.context_objects)),
                        )
                    elif kind == "upgrades":
                        generation = _integer(record.get("generation"), "generation")
                        state_for_generation = generations.get(generation)
                        if state_for_generation is None:
                            raise ValueError("upgrade snapshot has no matching player generation")
                        revision = _integer(record.get("revision"), "upgrade revision")
                        if revision != state_for_generation.upgrade_revision + 1:
                            raise ValueError("upgrade revision is missing or out of order")
                        upgrade_state_data = object_map(record.get("state"))
                        available = upgrade_state_data.get("available")
                        if not isinstance(available, bool):
                            raise ValueError("upgrade state must declare availability")
                        if revision == 1 and record.get("initial") is not True:
                            raise ValueError("first upgrade revision must be marked initial")
                        upgrade_snapshot = UpgradeSnapshot(
                            "known" if available else "unknown",
                            revision,
                            _world_time(record.get("world_time")),
                            _freeze_map(upgrade_state_data),
                        )
                        state_for_generation.upgrade_revision = revision
                        state_for_generation.upgrade_history[revision] = upgrade_snapshot
                    elif kind in ("sample", "stationary_event"):
                        if not generations:
                            raise ValueError("sample appeared before player acquisition")
                        sequence = _integer(record.get("sequence"), "sequence")
                        sample_data = object_map(record.get("sample"))
                        generation = _integer(sample_data.get("generation"), "sample generation")
                        state_for_generation = generations.get(generation)
                        if state_for_generation is None:
                            raise ValueError("sample references unknown player generation")
                        sample_context_revision = _integer(
                            sample_data.get("context_revision"), "sample context_revision"
                        )
                        context = state_for_generation.context_history.get(sample_context_revision)
                        if context is None:
                            raise ValueError(
                                f"sample references unknown context revision {sample_context_revision}"
                            )
                        lost_events = _integer(sample_data.get("lost_events"), "lost_events")
                        if lost_events != 0:
                            raise ValueError("sample reports lost events")

                        if version_minor == 0:
                            sample_upgrade_revision = sample_data.get("upgrade_revision")
                            if sample_upgrade_revision is not None:
                                raise ValueError("v2.0 samples cannot claim an upgrade revision")
                            sample_upgrades = UpgradeSnapshot("not_recorded", None, None, None)
                        else:
                            if not state_for_generation.upgrade_history:
                                raise ValueError("v2.1 sample has no upgrade baseline")
                            sample_upgrade_revision = _integer(
                                sample_data.get("upgrade_revision"), "sample upgrade_revision"
                            )
                            known = sample_data.get("upgrades_known")
                            candidate_upgrade = state_for_generation.upgrade_history.get(
                                sample_upgrade_revision
                            )
                            if candidate_upgrade is None:
                                raise ValueError("sample references unknown upgrade revision")
                            sample_upgrades = candidate_upgrade
                            if not isinstance(known, bool) or known != (sample_upgrades.status == "known"):
                                raise ValueError("sample upgrade-known flag disagrees with baseline")

                        if recorder_version is None or session is None or capture_manifest is None:
                            raise ValueError("missing manifest identity")
                        yield MovementSample(
                            session,
                            recorder_version,
                            capture_manifest,
                            sequence,
                            generation,
                            kind,
                            _freeze_map(record),
                            context,
                            sample_upgrades,
                        )
                except (KeyError, TypeError, ValueError) as error:
                    raise ValueError(f"{path}:{line_number}: {error}") from error

    if not manifest_seen:
        raise ValueError(f"{paths[0]}: missing manifest")
