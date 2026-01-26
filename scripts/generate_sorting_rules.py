#!/usr/bin/env python3
"""
Parse modorder_dream.txt and generate sorting_rules.json.
Creates rules from consecutive dfmod pairs specifying load order relationships.
"""

import json
import re
import sys
from pathlib import Path

# Map human-readable mod names to FileName format (lowercase, no extension)
def normalize_mod_name(name: str) -> str:
    """Convert display name to FileName format."""
    # Remove special characters but keep spaces and hyphens
    name = name.strip()
    # Convert to lowercase
    name = name.lower()
    # Normalize various characters to match FileName format
    name = name.replace('\u2013', '-')  # en-dash to hyphen
    name = name.replace('\u2014', '-')  # em-dash to hyphen
    name = name.replace('\u2019', "'")  # normalize apostrophes
    name = name.replace("'", "'")       # normalize other apostrophe style
    return name

def is_comment_start(text: str) -> bool:
    """Check if text looks like the start of a comment, not part of a mod name."""
    text = text.strip()
    if not text:
        return True
    # Normalize apostrophes
    text = text.replace('\u2019', "'")  # Right single quotation mark
    # Comments typically start with these words
    comment_starters = [
        'i ', "i'", 'you ', 'this ', 'if ', "it'", 'it ', 'all ', 'mod ', 'power ',
        'turn ', 'use ', 'not ', 'get ', 'place ', 'recently', 'one ', 'for ',
    ]
    lower = text.lower()
    for starter in comment_starters:
        if lower.startswith(starter):
            return True
    return False

def parse_modorder_file(filepath: Path) -> list[str]:
    """Parse the modorder file and extract mod names in order."""
    mods = []

    # Unicode bullet character used in the file
    BULLET = '\uf0b7'  # Private use area bullet
    DASH = '\u2013'    # En-dash used for comments

    with open(filepath, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()

            # Skip empty lines
            if not line:
                continue

            # Skip "Page X of Y" lines
            if re.match(r'^Page \d+ of \d+$', line):
                continue

            # Skip lines that are just commentary (don't start with bullet)
            if not line.startswith(BULLET):
                continue

            # Extract mod name - everything after bullet and before comment markers
            mod_name = line[1:].strip()  # Remove bullet prefix

            # Remove trailing comments (after "–" en-dash or " - " hyphen)
            # Only if what follows looks like a comment, not a mod name part
            for sep in [DASH, ' - ']:
                if sep in mod_name:
                    parts = mod_name.split(sep, 1)
                    if len(parts) > 1 and is_comment_start(parts[1]):
                        mod_name = parts[0].strip()
                        break

            if mod_name:
                mods.append(normalize_mod_name(mod_name))

    return mods

def generate_rules(mods: list[str]) -> list[dict]:
    """Generate consecutive pair rules."""
    rules = []
    for i in range(len(mods) - 1):
        rules.append({
            "first": mods[i],
            "then": mods[i + 1]
        })
    return rules

def main():
    if len(sys.argv) < 2:
        # Default path
        input_path = Path.home() / "Documents" / "modorder_dream.txt"
    else:
        input_path = Path(sys.argv[1])

    if not input_path.exists():
        print(f"Error: Input file not found: {input_path}", file=sys.stderr)
        sys.exit(1)

    output_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("sorting_rules.json")

    print(f"Parsing: {input_path}")
    mods = parse_modorder_file(input_path)
    print(f"Found {len(mods)} mods")

    # Print first 10 for verification
    print("First 10 mods:")
    for i, mod in enumerate(mods[:10]):
        print(f"  {i+1}. {mod}")

    rules = generate_rules(mods)
    print(f"Generated {len(rules)} rules")

    output = {"rules": rules}

    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(output, f, indent=2)

    print(f"Written to: {output_path}")

if __name__ == "__main__":
    main()
