#!/bin/sh
# What the preview renders and where things land, as text.
#
# A page that loads is not a page that lays out: the sweep counts errors, the layout diff
# compares element rectangles, and a picture is only made when a person asks for one.
#
#   scripts/preview-check.sh            sweep, then diff the layout goldens
#   scripts/preview-check.sh --update   re-record the goldens
#
# The gallery package is a build artifact, so this runs where it exists; the goldens live
# under target/ and are not committed.
set -e
root="$(git rev-parse --show-toplevel)"
cd "$root"

app=target/gallery/out
bin=target/debug/examples/render
golden=target/preview-golden
pages=${PREVIEW_PAGES:-Pages/HomePage Pages/Clipboard/ClipboardPage Controls/ControlExample}

if [ ! -x "$bin" ]; then
    echo "build the render example first: cargo build -p openxaml --example render" >&2
    exit 1
fi
if [ ! -d "$app" ]; then
    echo "no gallery package at $app" >&2
    exit 1
fi

total=0
rendered=0
failing=""
for page in $(find "$app" -name '*.xbf' | sort); do
    total=$((total + 1))
    if "$bin" "$page" /dev/null 64 64 >/dev/null 2>&1; then
        rendered=$((rendered + 1))
    else
        failing="$failing $(basename "$page")"
    fi
done
echo "pages_rendered=$rendered / $total"
if [ -n "$failing" ]; then
    echo "failing:$failing" | fold -w 100 -s
fi

mkdir -p "$golden"
status=0
for page in $pages; do
    file=$(find "$app" -name "$(basename "$page").xbf" | head -1)
    [ -n "$file" ] || continue
    name=$(echo "$page" | tr '/' '_')
    OPENCOMP_TRACE=2 "$bin" "$file" "$golden/$name.ppm" 800 600 2>&1 \
        | grep -E '^ +[A-Za-z]' > "$golden/$name.layout" || true
    if [ "$1" = "--update" ]; then
        cp "$golden/$name.layout" "$golden/$name.expected"
        echo "recorded $name"
        continue
    fi
    if [ -f "$golden/$name.expected" ]; then
        if ! diff -u "$golden/$name.expected" "$golden/$name.layout" > "$golden/$name.diff"; then
            echo "layout changed: $name (see $golden/$name.diff)"
            status=1
        fi
    else
        echo "no golden for $name; run with --update"
    fi
done
exit $status