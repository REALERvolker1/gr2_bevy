"""Report observed dash-clock transitions without inventing physics ticks."""

import argparse
import json
import math
import struct
from collections.abc import Iterable, Mapping
from dataclasses import asdict, dataclass
from pathlib import Path

from movement_trace import MovementSample, iter_samples


def _mapping(value: object) -> dict[str, object]:
	if not isinstance(value, Mapping):
		raise TypeError("expected object")
	result: dict[str, object] = {}
	for key, item in value.items():
		if not isinstance(key, str):
			raise TypeError("expected string object keys")
		result[key] = item
	return result


def _number(value: object) -> float | None:
	if value is None or value is False:
		return None
	if isinstance(value, bool) or not isinstance(value, (int, float)):
		raise TypeError("expected finite number or unavailable clock")
	result = float(value)
	if not math.isfinite(result):
		raise ValueError("nonfinite channel")
	return result


def _float32(value: float) -> float:
	# Native admission subtracts two float32 audio timestamps before comparing.
	return float(struct.unpack("<f", struct.pack("<f", value))[0])


@dataclass(frozen=True)
class Observation:
	session: str
	sequence: int
	generation: int
	poll: int
	object_id: str
	game_time: float | None
	audio_time: float | None
	stamina: float
	dash_stamp: float
	cooldown: float | None
	upgrade_revision: int | None
	upgrades_status: str


def observation(sample: MovementSample) -> Observation | None:
	data = _mapping(sample.raw.get("sample"))
	candidates: list[tuple[str, dict[str, object]]] = []
	for object_id, value in _mapping(data.get("objects")).items():
		fields = _mapping(value)
		if "@GetStamina" in fields and "DashLastTimeUsed" in fields:
			candidates.append((object_id, fields))
	if not candidates:
		return None
	if len(candidates) != 1:
		raise ValueError("ambiguous stamina subsystem")
	object_id, fields = candidates[0]
	stamina = _number(fields["@GetStamina"])
	stamp = _number(fields["DashLastTimeUsed"])
	if stamina is None or stamp is None:
		return None
	poll = data.get("poll")
	if not isinstance(poll, int) or isinstance(poll, bool):
		raise TypeError("expected integer poll identity")
	return Observation(
		sample.session,
		sample.sequence,
		sample.generation,
		poll,
		object_id,
		_number(data.get("world_time")),
		_number(data.get("world_audio_time")),
		stamina,
		stamp,
		_number(fields.get("DashCooldown")),
		sample.upgrades.revision,
		sample.upgrades.status,
	)


@dataclass(frozen=True)
class DashTransition:
	before: Observation
	after: Observation
	kind: str
	audio_bracket_contains_stamp: bool | None
	elapsed_float32: float | None
	observed_cooldown_compatible: bool | None
	observed_stamina_drop: float
	same_upgrade_revision: bool


def transition(before: Observation, after: Observation) -> DashTransition:
	advanced = after.dash_stamp > before.dash_stamp
	bracket: bool | None = None
	compatible: bool | None = None
	elapsed: float | None = None
	if advanced:
		if before.audio_time is not None and after.audio_time is not None:
			bracket = before.audio_time <= after.dash_stamp <= after.audio_time
		elapsed = _float32(after.dash_stamp - before.dash_stamp)
		# A changed sampled parameter cannot identify the parameter at action time.
		if after.cooldown is not None and before.cooldown == after.cooldown:
			compatible = after.cooldown <= elapsed
	return DashTransition(
		before,
		after,
		"timestamp_advance" if advanced else "timestamp_reset",
		bracket,
		elapsed,
		compatible,
		before.stamina - after.stamina,
		before.upgrades_status == after.upgrades_status == "known"
		and before.upgrade_revision == after.upgrade_revision,
	)


def summarize(samples: Iterable[MovementSample]) -> dict[str, object]:
	previous: Observation | None = None
	transitions: list[DashTransition] = []
	retained = missing = historical = 0
	for sample in samples:
		current = observation(sample)
		if current is None:
			# Do not bridge a missing channel and label its full gap one action.
			previous = None
			missing += 1
			continue
		if previous is not None:
			if (current.session, current.generation, current.object_id) != (
				previous.session,
				previous.generation,
				previous.object_id,
			):
				previous = None
			elif current.poll <= previous.poll:
				# Pre-roll may repeat old observations after a stationary event.
				historical += 1
				continue
		if previous is not None and current.dash_stamp != previous.dash_stamp:
			transitions.append(transition(previous, current))
		retained += 1
		previous = current
	return {
		"retained_observations": retained,
		"missing_stamina_observations": missing,
		"historical_or_duplicate_observations": historical,
		"transitions": [asdict(item) for item in transitions],
		"limits": [
			"Timestamp changes are observations; multiple actions can occur between polls.",
			"Stamina drop is net sampled change, not final dash cost; regeneration and other spending may intervene.",
			"A sampled board revision is not the complete native active-upgrade predicate.",
			"Clock/cooldown compatibility is not physics-tick replay or host parity.",
		],
	}


def main() -> None:
	parser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("segments", nargs="+", type=Path)
	parser.add_argument("--output", required=True, type=Path)
	arguments = parser.parse_args()
	output = Path(arguments.output).resolve()
	private_root = Path(__file__).resolve().parents[1] / ".work"
	if not output.is_relative_to(private_root):
		parser.error("output must be below the workspace .work directory")
	report = summarize(iter_samples(arguments.segments))
	output.parent.mkdir(parents=True, exist_ok=True)
	# Preserve prior reports and their evidence hashes.
	with output.open("x", encoding="utf-8") as destination:
		json.dump(report, destination, indent=2, allow_nan=False)
		destination.write("\n")


if __name__ == "__main__":
	main()
