#!/bin/bash
# Drop-detection test: record a track through the MacBook microphone while
# marking each real drop with M, then compare the program's drops with yours.
#
#     ./test_drops.sh            (the recording is named after the time)
#     ./test_drops.sh mytrack    (recordings/mytrack.wav)

set -e
cd "$(dirname "$0")"

NAME="${1:-test-$(date +%H%M)}"
WAV="recordings/$NAME.wav"
MARKS="recordings/$NAME.marks.txt"
mkdir -p recordings
# Marks are added to the end of their file, so start this name afresh.
rm -f "$WAV" "$MARKS"

cat <<EOF

  Drop test: $WAV

  1. Play one track (or a mix) with at least one breakdown and drop through
     the speakers, from start to finish, with no pause in between.
  2. Press M exactly when each drop lands: the moment the kick and bass
     come back in.
  3. When the track ends, press Esc in the program window.

  Starting the program...

EOF

cargo build --release --quiet --example analyse
cargo run --release --quiet -- --device "MacBook Pro Microphone" --record "$WAV"

if [ ! -s "$MARKS" ]; then
  echo
  echo "  No marks were made, so there is nothing to compare. Run again and press M on each drop."
  exit 1
fi

echo
cargo run --release --quiet --example analyse -- "$WAV" | sed -n '/^mood changes/,$p'
