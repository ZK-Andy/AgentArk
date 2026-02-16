#!/bin/bash
# Extract a single frame from a video

VIDEO=""
TIME="00:00:00"
OUT=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --time)
            TIME="$2"
            shift 2
            ;;
        --out)
            OUT="$2"
            shift 2
            ;;
        *)
            VIDEO="$1"
            shift
            ;;
    esac
done

if [[ -z "$VIDEO" ]]; then
    echo "Usage: frame.sh <video> [--time HH:MM:SS] [--out output.jpg]"
    exit 1
fi

if [[ -z "$OUT" ]]; then
    OUT="${VIDEO%.*}_frame.jpg"
fi

ffmpeg -ss "$TIME" -i "$VIDEO" -frames:v 1 -q:v 2 "$OUT" -y
echo "Saved frame to: $OUT"
