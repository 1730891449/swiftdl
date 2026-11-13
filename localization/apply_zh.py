"""Restore the reviewed Chinese source before the first cloud build."""
from pathlib import Path
import base64
import gzip
import hashlib
import json

root = Path(__file__).resolve().parents[1]
payload = root / "localization/zh-CN.edits.json.gz.b64"
encoded = payload.read_text(encoding="utf-8").strip()
expected = "3a6ff74f3fb211cfb2dbce17c093c87a16cf3114ec374e5f827d71669918b8ea"
# Restore characters omitted during the initial connector transfer.
if hashlib.sha256(encoded.encode()).hexdigest() == "0357fb4bc16d873cdcb6aa0aa83b04164af2b04622d86a2e5c2a7edb24c4ea0a":
    fixes = [[4777, "/"], [7729, "/"], [8510, "/"], [9865, "/"], [13173, "/"], [15515, "//"], [15620, "/"], [15851, "/"], [17056, "/"], [18217, "/"], [18525, "/"]]
    for offset, text in reversed(fixes):
        encoded = encoded[:offset] + text + encoded[offset:]
if hashlib.sha256(encoded.encode()).hexdigest() != expected:
    raise SystemExit("Localization payload checksum mismatch; refusing to modify source")
changes = json.loads(gzip.decompress(base64.b64decode(encoded, validate=True)))
# Validate every input before changing any source file.
outputs = {}
for name, change in changes.items():
    path = (root / name).resolve()
    if not path.is_relative_to(root):
        raise SystemExit("Localization path leaves repository")
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    if hashlib.sha256(text.encode()).hexdigest() != change["sha256"]:
        raise SystemExit(f"Source differs from reviewed base: {name}")
    for start, end, replacement in reversed(change["edits"]):
        text = text[:start] + replacement + text[end:]
    outputs[path] = text
for path, text in outputs.items():
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")
payload.write_text(encoded + "\n", encoding="utf-8", newline="\n")
print(f"Restored {len(outputs)} reviewed Chinese source files")
