"""Writes the size of Noctiluca's look library as JSON, read from the app's
own lists in src/director.rs, for the website (site/library.json).

    python3 .github/scripts/library.py src/director.rs > site/library.json

Counts are what a visitor would call the choices: "none" and "clean" are
left out of objects, atmospheres and effects.
"""
import json
import re
import sys

source = open(sys.argv[1] if len(sys.argv) > 1 else "src/director.rs").read()


def size(name):
    m = re.search(rf"pub const {name}: \[[^;\]]+; (\d+)\]", source)
    if not m:
        sys.exit(f"{name} not found in director.rs")
    return int(m.group(1))


library = {
    "themes": size("THEMES"),
    "backgrounds": size("BACKGROUNDS"),
    "patterns": size("PATTERNS"),
    "objects": size("OBJECTS") - 1,
    "atmospheres": size("ATMOSPHERES") - 1,
    "effects": size("POST_NAMES") - 1,
    "palettes": size("PALETTE_NAMES"),
}
print(json.dumps(library, indent=2))
