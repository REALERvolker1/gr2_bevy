"""Load machine-specific paths from the gitignored config/local.toml."""

import tomllib
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PATH = ROOT / "config/local.toml"
EXAMPLE_PATH = ROOT / "config/local.toml.example"


@dataclass(frozen=True)
class LocalConfig:
	game_dir: Path
	trace_dir: Path
	compatdata_dirs: tuple[Path, ...]


def _display(path: Path) -> str:
	try:
		return str(path.relative_to(ROOT))
	except ValueError:
		return str(path)


def _path(value: object, name: str) -> Path:
	if not isinstance(value, str):
		raise TypeError(f"{name} must be a path string")
	if not value:
		raise ValueError(f"{name} must not be empty")
	return Path(value).expanduser().resolve()


def load(path: Path = DEFAULT_PATH) -> LocalConfig:
	try:
		text = path.read_text()
	except FileNotFoundError as error:
		raise ValueError(
			f"missing {_display(path)}; copy "
			f"{_display(EXAMPLE_PATH)} and set the local paths"
		) from error
	raw: object = tomllib.loads(text)
	if not isinstance(raw, dict):
		raise TypeError("local config must be a TOML table")
	compatdata: object = raw.get("compatdata_dirs", [])
	if not isinstance(compatdata, list):
		raise TypeError("compatdata_dirs must be a list of paths")
	return LocalConfig(
		game_dir=_path(raw.get("game_dir"), "game_dir"),
		trace_dir=_path(raw.get("trace_dir"), "trace_dir"),
		compatdata_dirs=tuple(_path(item, "compatdata_dirs entry") for item in compatdata),
	)
