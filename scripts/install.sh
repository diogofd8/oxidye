#!/usr/bin/env bash
set -euo pipefail
project=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
install_prefix=${OXIDYE_PREFIX:-$HOME/.local}
config_root=${XDG_CONFIG_HOME:-$HOME/.config}
if [[ $install_prefix != /* || $config_root != /* ]]; then
    printf 'OXIDYE_PREFIX and XDG_CONFIG_HOME must be absolute directories.\n' >&2
    exit 2
fi
cargo build --manifest-path "$project/Cargo.toml" --release --locked
install -Dm755 "$project/target/release/oxidye" "$install_prefix/bin/oxidye.new"
mv -f "$install_prefix/bin/oxidye.new" "$install_prefix/bin/oxidye"
"$project/scripts/build-gtk.sh" --install >/dev/null
# Keep existing selections and authored customizations. Application integration files and the device-specific backup are created only by the theme command.
for mapping in 'oxidye.json:config.json' 'default-accents.json:accents.json' 'qt-dark.json:qt-dark.json' 'qt-light.json:qt-light.json' 'gtk-dark.json:gtk-dark.json' 'gtk-light.json:gtk-light.json' 'gtk3.css:gtk/gtk3.css' 'gtk4.css:gtk/gtk4.css'; do
    source="$project/assets/${mapping%%:*}"
    destination="$config_root/oxidye/${mapping#*:}"
    if [[ ! -e $destination && ! -L $destination ]]; then
        install -Dm644 "$source" "$destination"
    fi
done
printf 'Installed %s/bin/oxidye and GTK validators.\n' "$install_prefix"
printf 'Configuration: %s/oxidye/config.json\n' "$config_root"
printf 'Run oxidye theme to apply the configured fixed theme.\n'
