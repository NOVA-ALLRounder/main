from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

import yaml


ROOT = Path(__file__).resolve().parents[1]
CONFIGS = ROOT / "collector" / "Data-Collection-Projection" / "configs"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Set privacy level")
    parser.add_argument(
        "--level",
        choices=["strict", "balanced", "permissive"],
        required=True,
        help="privacy level",
    )
    return parser.parse_args()


def load_yaml(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    data = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    return data if isinstance(data, dict) else {}


def dump_yaml(path: Path, data: dict[str, Any]) -> None:
    path.write_text(
        yaml.safe_dump(data, allow_unicode=True, sort_keys=False, indent=2),
        encoding="utf-8",
    )


def apply_config(level: str, config_path: Path) -> None:
    data = load_yaml(config_path)
    if not data:
        return
    privacy = data.setdefault("privacy", {})
    observability = data.setdefault("observability", {})
    activity_detail = data.setdefault("activity_detail", {})

    if level == "strict":
        privacy["url_mode"] = "domain"
        observability["activity_include_title"] = False
        observability["activity_title_apps"] = []
        activity_detail["enabled"] = False
        activity_detail["full_title_apps"] = []
    elif level == "balanced":
        privacy["url_mode"] = "domain"
        observability["activity_include_title"] = True
        observability["activity_title_apps"] = ["chrome.exe", "code.exe"]
        activity_detail["enabled"] = True
        activity_detail["full_title_apps"] = ["chrome.exe"]
    else:
        privacy["url_mode"] = "full"
        observability["activity_include_title"] = True
        observability["activity_title_apps"] = ["chrome.exe", "code.exe"]
        activity_detail["enabled"] = True
        activity_detail["full_title_apps"] = ["chrome.exe", "code.exe"]

    dump_yaml(config_path, data)


def main() -> None:
    args = parse_args()
    level = args.level

    src = CONFIGS / f"privacy_rules_{level}.yaml"
    if not src.exists():
        raise SystemExit(f"privacy profile not found: {src}")

    dest = CONFIGS / "privacy_rules.yaml"
    dest.write_text(src.read_text(encoding="utf-8"), encoding="utf-8")

    apply_config(level, CONFIGS / "config.yaml")
    apply_config(level, CONFIGS / "config_demo.yaml")

    print(f"privacy_level_set={level}")


if __name__ == "__main__":
    main()
