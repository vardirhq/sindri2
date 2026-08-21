#!/bin/sh

set -eu

output_path=${1:?"usage: capture-editor.sh OUTPUT.png [SCENE.json]"}
output_dir=$(dirname "$output_path")
mkdir -p "$output_dir"

# The editor takes a scene on its command line and falls back to the demo one,
# so passing a second argument here photographs whichever scene is of interest
# rather than only the fixture.
scene=${2:-}

if [ -n "$scene" ]; then
    cargo run --package sindri-editor -- "$scene" &
else
    cargo run --package sindri-editor &
fi
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

    # The title carries the open scene now — "demo.scene.json - Sindri Editor" —
    # so this matches the program name at the end rather than the whole title.
    window_id=$(xdotool search --name 'Sindri Editor$' 2>/dev/null | head -n 1 || true)
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
xdotool windowmap --sync "$window_id"
xdotool windowmove --sync "$window_id" 0 0
xdotool windowraise "$window_id"
sleep 3

# A window that exists but has not drawn yet grabs as a uniform black image.
# Accepting one would upload an empty screenshot as the editor's artifact and
# check its colours as though it were a frame, so a capture counts only once it
# has content. Roughly one grab in five arrived blank before this.
drawn() {
    deviation=$(identify -format "%[fx:standard_deviation]" "$1" 2>/dev/null || echo 0)
    awk -v deviation="$deviation" 'BEGIN { exit !(deviation > 0.01) }'
}

attempt=0
while [ "$attempt" -lt 80 ]; do
    if ! kill -0 "$editor_pid" 2>/dev/null; then
        wait "$editor_pid"
        exit 1
    fi

    if import -window "$window_id" "$output_path" 2>/dev/null && drawn "$output_path"; then
        exit 0
    fi

    attempt=$((attempt + 1))
    sleep 0.25
done

echo "Sindri Editor did not draw a frame to capture" >&2
exit 1
