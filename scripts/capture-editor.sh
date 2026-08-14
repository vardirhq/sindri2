#!/bin/sh

set -eu

output_path=${1:?"usage: capture-editor.sh OUTPUT.png"}
output_dir=$(dirname "$output_path")
mkdir -p "$output_dir"

cargo build --package sindri-editor
xcompmgr -a &
compositor_pid=$!
target/debug/sindri-editor &
editor_pid=$!
window_id=""

cleanup() {
    kill "$editor_pid" 2>/dev/null || true
    kill "$compositor_pid" 2>/dev/null || true
    wait "$editor_pid" 2>/dev/null || true
    wait "$compositor_pid" 2>/dev/null || true
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
# The compositor redirects the WGPU-backed client window into an X pixmap that
# ImageMagick can read deterministically.
capture_attempt=0
while [ "$capture_attempt" -lt 30 ]; do
    if import -window "$window_id" "$output_path" 2>/dev/null; then
        color_count=$(identify -format '%k' "$output_path" 2>/dev/null || echo 0)
        if [ "$color_count" -gt 16 ]; then
            exit 0
        fi
    fi
    capture_attempt=$((capture_attempt + 1))
    sleep 0.5
done

echo "Sindri Editor window could not be captured" >&2
exit 1
