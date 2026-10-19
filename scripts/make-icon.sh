#!/bin/sh
# Regenerates the committed icon and card from the original DeskTuk logo on Windows.
set -e
cd "$(dirname "$0")/.."
powershell.exe -NoProfile -File scripts/build-brand-assets.ps1
