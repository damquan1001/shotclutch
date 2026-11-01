#!/usr/bin/env bash
# Regenerate committed icons from the original ShotClutch SVG (PySide6 + Pillow).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
python "$ROOT/scripts/generate-icons.py"
