from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate n8n workflow JSON")
    parser.add_argument("--file", required=True, help="workflow json path")
    return parser.parse_args()


def _load_json(path: str) -> Any:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def main() -> None:
    args = parse_args()
    data = _load_json(args.file)
    errors: list[str] = []

    if not isinstance(data, dict):
        errors.append("root_not_object")
        _print(errors)
        return

    if not data.get("name") or not isinstance(data.get("name"), str):
        errors.append("missing_name")

    nodes = data.get("nodes")
    if not isinstance(nodes, list) or not nodes:
        errors.append("missing_nodes")
        nodes = []

    connections = data.get("connections")
    if connections is None or not isinstance(connections, dict):
        errors.append("missing_connections")
        connections = {}

    node_names = {node.get("name") for node in nodes if isinstance(node, dict)}
    node_ids = {node.get("id") for node in nodes if isinstance(node, dict)}
    if len(node_names) != len([n for n in node_names if n]):
        errors.append("duplicate_or_empty_node_names")
    if len(node_ids) != len([n for n in node_ids if n]):
        errors.append("duplicate_or_empty_node_ids")

    # Basic connection validation: keys should match node names
    for src_name, outputs in connections.items():
        if src_name not in node_names:
            errors.append(f"unknown_connection_source:{src_name}")
        if not isinstance(outputs, dict):
            errors.append(f"invalid_connection_block:{src_name}")
            continue
        for output_key, dests in outputs.items():
            if not isinstance(dests, list):
                errors.append(f"invalid_connection_list:{src_name}:{output_key}")
                continue
            for dest in dests:
                if not isinstance(dest, dict):
                    errors.append(f"invalid_connection_entry:{src_name}:{output_key}")
                    continue
                dest_name = dest.get("node")
                if dest_name and dest_name not in node_names:
                    errors.append(f"unknown_connection_target:{dest_name}")

    _print(errors)


def _print(errors: list[str]) -> None:
    if errors:
        print("valid=false")
        print("errors:")
        for err in errors:
            print(f"- {err}")
    else:
        print("valid=true")


if __name__ == "__main__":
    main()
