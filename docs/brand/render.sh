#!/bin/bash
# Renders the brand from its sources, with Chrome in headless mode:
#
#   assets/icon/noctiluca.svg    → assets/icon/icon_1024.png, Noctiluca.icns,
#                                  and the Android launcher icons
#   docs/brand/banner.html       → docs/media/banner.png (the README's)
#   docs/brand/tv-banner.html    → the Android TV launcher banner
#
#     docs/brand/render.sh

set -e
cd "$(dirname "$0")/../.."

CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

shot() { # source, width, height, output
    "$CHROME" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
        --default-background-color=00000000 --window-size="$2,$3" \
        --screenshot="$4" "file://$PWD/$1" >/dev/null 2>&1
}

shot assets/icon/noctiluca.svg 1024 1024 "$TMP/icon.png"
shot docs/brand/banner.html 1600 560 docs/media/banner.png
shot docs/brand/tv-banner.html 640 360 android/app/src/main/res/drawable-xhdpi/banner.png
python3 docs/brand/make_icons.py "$TMP/icon.png"
