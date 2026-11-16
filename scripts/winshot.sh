#!/bin/sh
# Capture only the track-gpui window: winshot.sh <out.png>
id=$(swift "$(dirname "$0")/winshot.swift" track-gpui) || { echo "window not found" >&2; exit 1; }
screencapture -x -o -l"$id" "$1"
