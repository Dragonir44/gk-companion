#!/usr/bin/env python3
"""Generate the typetree schemas embedded in the app.

The game assets ship without typetrees, so the app needs the binary layout of
the few MonoBehaviour classes it reads. This script derives that layout from the
game's managed DLLs (TypeTreeGeneratorAPI) on a developer machine and writes it
to src-tauri/schemas/<game>.json.

A schema holds class *structure* only (field names, types, alignment) — no game
content. Each class also records Unity's per-type layout hash, which the app
compares at runtime to detect a game update that changed the layout.

Usage:
    python gen_schema.py gk1 "/path/to/Graveyard Keeper"
    python gen_schema.py gk2 "/path/to/Graveyard Keeper 2"
"""
import json
import os
import sys

import UnityPy
from UnityPy.helpers.Tpk import get_common_strings, get_typetree_node
from UnityPy.helpers.TypeTreeGenerator import TypeTreeGenerator
from UnityPy.helpers.UnityVersion import UnityVersion

# Objects to read, by m_Name. The script class is resolved from the asset.
TARGETS = {
    "gk1": ["game_data", "lng_en"],
    "gk2": ["GameBalance", "lng_en"],
}

# Built-in Unity classes read for icons, by class id. Their layout depends
# only on the Unity version (gk1 assets have no typetrees; gk2 bundles do,
# but these serve as a fallback).
BUILTIN = {"Texture2D": 28, "Sprite": 213, "SpriteAtlas": 687078895}

OUT_DIR = os.path.join(os.path.dirname(__file__), "..", "..", "src-tauri", "schemas")


class NodeTable:
    """Hash-consed node table: identical subtrees are stored once."""

    def __init__(self):
        self.nodes = []
        self.index = {}

    def add(self, node):
        children = [self.add(c) for c in node.m_Children]
        key = (node.m_Name, node.m_Type, node.m_MetaFlag, tuple(children))
        if key not in self.index:
            self.index[key] = len(self.nodes)
            self.nodes.append([node.m_Name, node.m_Type, node.m_MetaFlag, children])
        return self.index[key]


def data_dir(game_root):
    return os.path.join(game_root, next(d for d in os.listdir(game_root) if d.endswith("_Data")))


def main(game, game_root):
    data = data_dir(game_root)
    env = UnityPy.load(os.path.join(data, "resources.assets"))
    assets = next(iter(env.files.values()))
    wanted = set(TARGETS[game])
    # peek_name() must run before the generator is attached: the generator
    # fails on some unrelated scripts, and peeking would trigger it.
    found = {o.peek_name(): o for o in env.objects
             if o.type.name == "MonoBehaviour" and o.byte_size > 4096 and o.peek_name() in wanted}
    missing = wanted - found.keys()
    if missing:
        sys.exit(f"objects not found: {sorted(missing)}")

    generator = TypeTreeGenerator(assets.unity_version)
    generator.load_local_game(game_root)
    env.typetree_generator = generator

    table = NodeTable()
    classes = {}
    for name, obj in sorted(found.items()):
        node = obj._get_typetree_node()
        st = obj.serialized_type
        classes[name] = {
            "root": table.add(node),
            "type_hash": st.old_type_hash.hex(),
            "script_id": st.script_id.hex(),
        }
        print(f"{name}: {len(node.m_Children)} fields, hash {classes[name]['type_hash']}")

    version = UnityVersion.from_str(assets.unity_version)
    builtin = {str(cid): table.add(get_typetree_node(cid, version)) for cid in BUILTIN.values()}

    schema = {
        "schema_version": 2,
        "game": game,
        "unity_version": assets.unity_version,
        "classes": classes,
        "builtin": builtin,
        # Unity's shared string table, used by embedded typetrees.
        "common_strings": {str(k): v for k, v in get_common_strings(version).items()},
        "nodes": table.nodes,
    }
    os.makedirs(OUT_DIR, exist_ok=True)
    out = os.path.join(OUT_DIR, f"{game}.json")
    with open(out, "w") as f:
        json.dump(schema, f, separators=(",", ":"))
    print(f"wrote {out}: {len(table.nodes)} unique nodes, {os.path.getsize(out)} bytes")


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in TARGETS:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
