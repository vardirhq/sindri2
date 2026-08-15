#!/bin/sh

set -eu

output_path=${1:?"usage: capture-editor.sh OUTPUT.png"}
output_dir=$(dirname "$output_path")
mkdir -p "$output_dir"

cargo run --package sindri-editor &
editor_pid=$!
window_id=""

cleanup() {
    kill "$editor_pid" 2>/dev/null || true
    wait "$editor_pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

attempt=0
while [ "$attempt" -lt 120 ]; do
    if ! kill -0 "$editor_pid" 2>/dev/null; then
        wait "$editor_pid"
        exit 1
    fi

    window_id=$(xdotool search --name '^Sindri Editor$' 2>/dev/null | head -n 1 || true)
    if [ -n "$window_id" ]; then
        break
    fi

    attempt=$((attempt + 1))
    sleep 0.25
done

if [ -z "$window_id" ]; then
    echo "Sindri Editor window did not appear" >&2
    exit 1
fi

sleep 2
xdotool windowmove "$window_id" 0 0

attempt=0
while [ "$attempt" -lt 20 ]; do
    if import -window root -crop 1440x1024+0+0 "$output_path" 2>/dev/null; then
        exit 0
    fi

    attempt=$((attempt + 1))
    sleep 0.25
done

echo "Sindri Editor window could not be captured" >&2
exit 1
