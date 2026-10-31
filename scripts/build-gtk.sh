#!/usr/bin/env bash
set -euo pipefail
project=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
build=${GTK_BUILD_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/oxidye/native/gtk}
install_prefix=${OXIDYE_PREFIX:-$HOME/.local}
probes=false
install_helpers=false
for option in "$@"; do
    case "$option" in
        --probes) probes=true ;;
        --install) install_helpers=true ;;
        *) printf 'Usage: %s [--probes] [--install]\n' "$0" >&2; exit 2 ;;
    esac
done
mkdir -p "$build"
(
    flock 9
    for version in 3 4; do
        package=gtk+-3.0
        if [[ $version == 4 ]]; then package=gtk4; fi
        source="$project/src/native/gtk-css-check.c"
        target="$build/gtk-css-check-$version"
        if [[ ! -x $target || $source -nt $target ]]; then
            "${CC:-cc}" -Wall -Wextra -O2 -o "$target.new" "$source" $(pkg-config --cflags --libs "$package")
            mv "$target.new" "$target"
        fi
        if $probes; then
            source="$project/tests/native/probe-gtk.c"
            target="$build/probe-gtk$version"
            if [[ $version == 4 ]]; then package=libadwaita-1; fi
            if [[ ! -x $target || $source -nt $target ]]; then
                "${CC:-cc}" -Wall -Wextra -Wno-deprecated-declarations -O2 -o "$target.new" "$source" $(pkg-config --cflags --libs "$package")
                mv "$target.new" "$target"
            fi
        fi
    done
) 9>"$build/.build.lock"
if $install_helpers; then
    for version in 3 4; do
        destination="$install_prefix/libexec/oxidye/gtk-css-check-$version"
        install -Dm755 "$build/gtk-css-check-$version" "$destination.new"
        mv -f "$destination.new" "$destination"
    done
fi
printf '%s\n' "$build"
