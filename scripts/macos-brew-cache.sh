#!/usr/bin/env bash
# CI helper: cache the Homebrew kegs that the macOS build bundles (mpv, ffmpeg, dylibbundler and
# every library they load), so the job doesn't download and pour ~40 bottles each run.
#   key           print a cache key from the current mpv, ffmpeg and dylibbundler versions
#   install       brew install mpv and dylibbundler, without mpv's yt-dlp dependency
#   save FILE     tar the kegs (found by following otool -L from libmpv, ffmpeg and ffprobe)
#   restore FILE  unpack them, link mpv, ffmpeg and dylibbundler, and check every library resolves
# restore fails if anything is missing; the workflow then installs with brew as before.
set -euo pipefail
prefix=$(brew --prefix)
roots=(mpv ffmpeg dylibbundler)

# Every Homebrew file reachable from libmpv, ffmpeg and ffprobe through otool -L, as real paths.
closure() {
  local queue=("$prefix/opt/mpv/lib/libmpv.dylib" "$prefix/opt/ffmpeg/bin/ffmpeg" \
    "$prefix/opt/ffmpeg/bin/ffprobe" "$prefix/opt/dylibbundler/bin/dylibbundler")
  local seen=" " f dep
  while [ ${#queue[@]} -gt 0 ]; do
    f=${queue[0]}
    queue=("${queue[@]:1}")
    case "$seen" in *" $f "*) continue ;; esac
    seen+="$f "
    if [ ! -e "$f" ]; then
      echo "missing: $f" >&2
      return 1
    fi
    echo "$f"
    while read -r dep; do
      queue+=("$dep")
    done < <(otool -L "$f" | awk -v p="$prefix/" 'NR > 1 && index($1, p) == 1 { print $1 }')
  done
}

# Formula names from the closure's paths (.../opt/<name>/... or .../Cellar/<name>/...).
formulae() {
  { printf '%s\n' "${roots[@]}"; closure | sed -nE "s#^$prefix/(opt|Cellar)/([^/]+)/.*#\2#p"; } | sort -u
}

case "${1:-}" in
  key)
    os=$(sw_vers -productVersion | cut -d. -f1)
    versions=$(brew info --json=v2 "${roots[@]}" | jq -r '[.formulae[] |
      .name + "-" + .versions.stable + (if .revision > 0 then "_\(.revision)" else "" end)] | join("-")')
    echo "brew-macos$os-$(uname -m)-$versions"
    ;;
  install)
    # Chyk never runs yt-dlp, and it brings deno (130 MB), so install mpv's other dependencies
    # and then mpv on its own.
    deps=$(brew deps --direct --include-build mpv | grep -vx yt-dlp)
    brew install $deps dylibbundler
    HOMEBREW_DEVELOPER=1 brew install --ignore-dependencies mpv
    ;;
  save)
    list=$(formulae)
    echo "$list" > "$prefix/.chyk-cache-formulae"
    paths=(.chyk-cache-formulae)
    for f in $list; do
      keg=$(cd "$prefix/opt/$f" && pwd -P)
      paths+=("opt/$f" "${keg#"$prefix"/}")
    done
    tar -cf "$2" -C "$prefix" "${paths[@]}"
    echo "cached $(wc -w <<<"$list" | tr -d ' ') formulae: $(echo $list)"
    ;;
  restore)
    tar -xUf "$2" -C "$prefix"
    brew link --overwrite "${roots[@]}" >/dev/null
    for f in "$prefix/lib/libmpv.dylib" "$prefix/bin/ffmpeg" "$prefix/bin/ffprobe" "$prefix/bin/dylibbundler"; do
      [ -e "$f" ] || { echo "missing: $f" >&2; exit 1; }
    done
    closure >/dev/null
    echo "restored $(wc -l <"$prefix/.chyk-cache-formulae" | tr -d ' ') formulae"
    ;;
  *)
    echo "usage: $0 key | install | save FILE | restore FILE" >&2
    exit 2
    ;;
esac
