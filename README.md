# oxidye

**Oxide + dye.** A small Rust command that colours the desktop with fixed light/dark themes and OKLCH accents, or full image-derived palettes from Matugen.

Oxidye applies Breeze palettes to Qt 5/6, custom colours and widget styling to GTK 3/4 with Adwaita / adw-gtk3 / libadwaita, mode-specific icons and optional VS Code themes. Fixed mode keeps its neutral surfaces and semantic colours while generating decorative accents in OKLCH. The separate Matugen command generates a full palette from an image. Both prepare dark and light appearances together. It runs on demand and exits; there is no daemon or polling.

## Usage

```sh
oxidye theme --mode dark
oxidye theme --mode light
oxidye theme --accent 'oklch(0.55 0.2 260)'
oxidye theme --mode dark --accent 'oklch(0.6 0.2 145)'

# Reapply the current appearance and accent.
oxidye theme

# Return to the default crimson (#7B2424), keeping the current mode.
oxidye theme --accent 'oklch(0.397103719 0.120772653 24.4920635)'

# Machine-readable result for a shell or desktop widget.
oxidye theme --mode dark --format json

# Restore the configuration saved before the first application.
oxidye restore-theme
```

Omitting either option retains its current value. `theme --format json` returns the applied mode, source, normalized seed, named accent fallback, icons and configured VS Code theme. Errors go to stderr. `restore-theme` restores the files and preferences saved before the first application.

Input must be opaque OKLCH. Percentage lightness and degree hues work too: `oklch(55% 0.2 260deg)`. Mode-only changes reuse the selected accent. Reopen GTK/Qt applications for complete colour and icon updates; some toolkit styles cache their settings. Oxidye does not restart applications.

`theme` always selects fixed colours, including when a Matugen theme is active. Matugen is an optional dependency used only by the separate image-generation command. Oxidye does not set wallpapers or control Hyprbar.

## Image themes with Matugen

Install Matugen separately if you want image-based generation; the adapter is tested with Matugen 4.2. Its executable must be available on `PATH` when generating a new image theme.

```sh
sudo pacman -S --needed matugen
oxidye matugen --image /path/to/image.jpg --mode dark
oxidye matugen --image /path/to/image.jpg --mode light --scheme scheme-vibrant

# Switch or reapply a previously rendered image theme.
oxidye matugen --mode light
oxidye matugen

# Return to fixed neutrals and the saved fixed accent.
oxidye theme --mode dark
```

Each explicit `--image` request runs Matugen to obtain both appearances, then validates and renders them before applying the selected one. Omitting `--mode` keeps the configured appearance. The default generation scheme is `scheme-tonal-spot`; `--scheme` requires `--image` and accepts `scheme-content`, `scheme-expressive`, `scheme-fidelity`, `scheme-fruit-salad`, `scheme-monochrome`, `scheme-neutral`, `scheme-rainbow`, `scheme-tonal-spot`, `scheme-vibrant` and `scheme-smart`.

Without `--image`, Oxidye reads its previously rendered toolkit files. These switches require neither Matugen nor the original image. If either appearance is missing, inconsistent or invalid, supply `--image` to generate the pair again. Fixed and image-derived palettes have separate files and selections; image generation never changes your fixed accent or profiles. Only the current appearance, application settings, icons and editor selection are shared.

Matugen owns generation caching under `${XDG_CACHE_HOME:-$HOME/.cache}/oxidye/matugen/generation-v1/`. Its subprocess receives an isolated configuration with caching enabled, templates empty, update checks disabled and explicit source-colour index `0`. `--dry-run` prevents hooks and wallpaper changes. Your standalone Matugen configuration and cache are untouched. Oxidye retains the GTK/Qt outputs and selection metadata under its own configuration directory; it stores no extra raw Matugen JSON cache. Failed validation can leave a Matugen generation cache entry, while the active theme and successful selection remain unchanged.

Matugen generates Material colours. Its RGB values are retained when they meet the toolkit contrast requirements; derived states and contrast repairs use OKLCH. Error/destructive colours use Matugen's error roles. Success, warning and information colours use independent green, amber and blue seeds with harmonization disabled. The GTK layout and Breeze widget design stay the same in both workflows.

## Dependencies

The supported and tested environment is Arch Linux on a graphical user session.

| Purpose | Requirements |
| --- | --- |
| Build | Rust/Cargo, a C compiler, `pkg-config`, Bash, coreutils and `flock` |
| GTK | `gtk3`, `gtk4`, `libadwaita`, `adw-gtk-theme` and GTK development headers for the CSS validators |
| Qt | [Breeze](https://archlinux.org/packages/extra/x86_64/breeze/) for Qt 6, `breeze5` for Qt 5, and the [qt5ct](https://archlinux.org/packages/extra/x86_64/qt5ct/) / [qt6ct](https://archlinux.org/packages/extra/x86_64/qt6ct/) platform-theme plugins |
| Settings/reload | `glib2` (`gsettings`, `gdbus`), `gsettings-desktop-schemas` with `accent-color`, `procps-ng` (`pkill`) and a working session D-Bus |
| Default icons | `papirus-icon-theme`, or change the icon names in `config.json` |
| Optional integrations | `xsettingsd` if you use XSettings; VS Code with the configured themes installed |
| Optional image generation | `matugen` 4.2-compatible JSON export; unnecessary for fixed mode and previously rendered image themes |

On Arch, install a Rust toolchain and the following packages. Skip any optional integration you do not use:

```sh
sudo pacman -S --needed base-devel util-linux gtk3 gtk4 libadwaita adw-gtk-theme breeze breeze5 qt5ct qt6ct glib2 gsettings-desktop-schemas procps-ng papirus-icon-theme
```

Qt applications must load their qtct platform-theme plugin. The installer sets Breeze and the colour palette in both qtct configurations; it does not change your compositor or shell environment. To check each toolkit explicitly:

```sh
QT_QPA_PLATFORMTHEME=qt5ct qt5ct
QT_QPA_PLATFORMTHEME=qt6ct qt6ct
```

Configure the corresponding plugin for your applications/session. Existing fonts and interface preferences are preserved; install your chosen fonts separately. The default VS Code mapping uses the `GitHub.github-vscode-theme` extension. Remove the `vscode` entry from the configuration, or set it to `null`, to disable editor integration. No Plasma session or `plasma-workspace` is required.

Cargo installs the Rust library dependencies from [Cargo.toml](Cargo.toml), using the versions in the committed [Cargo.lock](Cargo.lock): Clap, Serde, serde_json, palette, csscolorparser, jsonc-parser and tempfile.

## Install

From a checkout of this repository, after installing the dependencies:

```sh
./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
oxidye theme
```

The installer builds the Rust executable and two finite native GTK CSS validators. It installs them under `~/.local/bin` and `~/.local/libexec/oxidye`. It supplies missing source files under `${XDG_CONFIG_HOME:-$HOME/.config}/oxidye`. Existing theme sources, mode/accent selection and unrelated application preferences are preserved. It does not apply a theme until you call `theme` or `matugen`.

Installation requires `Cargo.toml`, `Cargo.lock`, `src/`, `assets/` and `scripts/`. The GTK validator is application support code in `src/native/`. The installer needs no development tests or additional documentation files. This README contains the installation and usage guide.

`OXIDYE_PREFIX` overrides `~/.local` for installation/testing. With a custom prefix, pass `--validator-dir PREFIX/libexec/oxidye` to the theme command. Rust/native binaries should be rebuilt for the target architecture/runtime.

For another device, install from this repository and copy your authored theme inputs if you customized them: `config.json`, `accents.json`, `qt-dark.json`, `qt-light.json`, `gtk-dark.json`, `gtk-light.json`, `gtk/gtk3.css` and `gtk/gtk4.css` from your Oxidye configuration directory. Reapply on the new device to regenerate absolute GTK imports and Qt paths. Let that device create its own restore backup.

## Configuration

Edit `${XDG_CONFIG_HOME:-$HOME/.config}/oxidye/config.json`. It maps:

- The current `mode` and the dark/light icon theme names (default `Papirus-Dark` / `Papirus`).
- The shared `accents.json` containing both OKLCH accent sets.
- Fixed Qt/GTK profiles, GTK layouts and generated output paths.
- Optional `vscode.dark` and `vscode.light` theme names.

Paths in that file are relative to the `oxidye` configuration directory. Changing an accent leaves the neutral and semantic source colours intact. The program stages and validates both appearances before installation, rejects concurrent operations, and rolls back failed file/settings updates.

| Default location | Contents |
| --- | --- |
| `~/.local/bin/oxidye` | Command |
| `~/.local/libexec/oxidye/` | Native GTK CSS validators |
| `~/.config/oxidye/` | Configuration, profiles, GTK layouts and generated palettes |
| `~/.config/oxidye/matugen/` | Separately rendered image-theme pair and last successful image/scheme selection |
| `~/.local/state/oxidye/` | Operation lock and restore backup |
| `~/.cache/oxidye/` | Matugen generation namespace and local native helper/test builds |

`XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME` override their respective roots. GTK/Qt, KDE and optional VS Code integration files stay in their standard application directories. `OXIDYE_PREFIX` changes the installer prefix; with a custom prefix use `theme --validator-dir PREFIX/libexec/oxidye`.

Read the current mode, accent source and last successful application from these files:

```sh
cat "${XDG_CONFIG_HOME:-$HOME/.config}/oxidye/config.json"
cat "${XDG_CONFIG_HOME:-$HOME/.config}/oxidye/accents.json"
cat "${XDG_CONFIG_HOME:-$HOME/.config}/oxidye/applied.json"
```

## VS Code integration

The default configuration includes the following mode-to-theme mapping:

```json
"vscode": {
  "dark": "GitHub Dark High Contrast",
  "light": "GitHub Light High Contrast"
}
```

Set these names to themes installed in your editor. Applying a mode updates `Code/User/settings.json` under your configuration root, including `workbench.colorTheme` and both preferred light/dark theme pairs. The editor can still switch between the configured themes using its own shortcut; the next Oxidye application selects the theme matching the desktop mode. VS Code normally reloads settings live. Comments, trailing commas, fonts, keybindings and unrelated settings are preserved. Invalid JSONC aborts the operation. Remove `vscode` or set it to `null` to disable this integration.

## Calling from another program

Run a finite process with separate arguments. For example, a desktop widget can use this argument array, replacing the executable path with its installed location:

```json
["/absolute/path/to/oxidye", "theme", "--mode", "dark", "--format", "json"]
```

For accent-only changes, pass `theme`, `--accent`, the complete OKLCH string, `--format`, `json`. Shell quoting is unnecessary inside an argument array. Wait for completion; exit code `0` indicates success. An example JSON result is:

```json
{"version":1,"mode":"dark","source":"fixed","seed":"oklch(0.550000000 0.200000000 260.0000000)","named_accent":"blue","icon_theme":"Papirus-Dark","vscode_theme":"GitHub Dark High Contrast"}
```

For image generation, use `matugen`, `--image`, the image path, `--mode`, the appearance and `--format`, `json`. Its result has `source: "matugen"` and also includes the canonical image path in `image` and the selected `scheme`. A rendered mode-only switch reports the same image and scheme even when the image is no longer available.

`vscode_theme` is omitted when editor integration is disabled. `named_accent` is the nearest supported system preference; GTK/Qt use the generated palette colours. Diagnostics go to stderr. Failed operations emit no success JSON. Hyprbar and terminal integration are separate from this program.

## Restoration and testing options

`oxidye restore-theme` restores the files, permissions and GSettings preferences saved in `${XDG_STATE_HOME:-$HOME/.local/state}/oxidye/theme-backup.json`. Later applications preserve the original snapshots and add newly mapped output paths. The backup belongs to its device and configuration root; do not copy it to another machine.

Both `theme` and `matugen` accept `--validator-dir DIRECTORY` to select native GTK validators from a custom installation. `--no-notify` skips GSettings and reload notifications while still writing configuration and palette files. Use temporary `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME` directories for isolated testing. `restore-theme --no-notify` similarly restores files without changing GSettings or sending reload notifications.

## Source layout

- `src/lib.rs`: synchronous OKLCH transformation, gamut mapping and contrast repair.
- `src/theme.rs`: shared configuration, staging, application and restoration for both sources.
- `src/matugen.rs`, `src/matugen/colours.rs`: optional Matugen subprocess, rendered-theme selection, complete Material role mapping and contrast repair.
- `src/qt.rs`, `src/qt/install.rs`: fixed QPalette/KDE mapping and Qt settings.
- `src/gtk.rs`, `src/gtk/install.rs`: fixed GTK layouts, colour imports and settings.
- `src/vscode.rs`: targeted JSONC theme edits that preserve unrelated settings.
- `src/config_io.rs`: locks, atomic file replacement, snapshots and rollback.
- `src/native/`: required GTK CSS validator source, compiled for GTK 3 and GTK 4.
- `assets/`: authored generator references, fixed toolkit profiles/layouts, default configuration and approved crimson accent source.
- `scripts/`: installation and native helper builds.

The local `tests/` directory contains additional development tests and widget probes. It is not required to install or use Oxidye.

## Colour rules

The immutable reference is `oklch(0.397103719 0.120772653 24.4920635)` (`#7B2424`). It reproduces the approved dark/light accent pair, including the muted light selection and quiet inactive selection. Other hues transform profile tones in OKLCH. Out-of-gamut chroma is reduced at fixed lightness/hue; bounded lightness searches resolve text/focus contrast. Bright accents can use dark foregrounds. Toolkit export truncates RGB channels consistently with the approved palettes.

Fourteen decorative roles change. Error/destructive, success, warning and information colours remain fixed. GTK and light Qt text meet 4.5:1 and focus 3:1, including 80% compositing over black/white. The approved dark SweetAmbar Qt exception for translucent links/text on brighter button/tooltip surfaces remains reported in the palette metadata; fixed neutrals are not silently changed.

These reference rules describe the fixed workflow. Matugen uses its own palette for surfaces and accents, with text contrast of at least 4.5:1 and necessary focus/control boundaries of 3:1 after compositing. Its generated output does not inherit the fixed dark Qt exception.

## Validate

Run the checks available from the application source:

```sh
cargo fmt -- --check --config max_width=65535,use_small_heuristics=Max
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
```

When the optional local `tests/` directory is available, its installation and native suites use temporary XDG configurations and a private D-Bus session. They also need `jq`, Node.js, CMake, Qt 5/6 Widgets/Test, `kconfigwidgets5`, `kcolorscheme`, `libadwaita` headers and a graphical session:

```sh
sudo pacman -S --needed cmake qt5-base qt6-base kconfigwidgets5 kcolorscheme jq nodejs
./tests/install.sh
./tests/native/run.sh
```

The native suite uses a temporary XDG configuration, private D-Bus session and persistent temporary GSettings backend, including the GTK settings portal. It checks crimson, blue, green, amber and grey in both modes, all Qt 5/6 palette roles, Breeze painting, GTK colours, icons, fonts, geometry and exact restoration. Rust tests cover gamut/endpoint inputs, approved colour goldens, fixed-neutral invariance, mode-only cached-file reuse, JSONC preservation, concurrency and failed validation/installation/GSettings rollback.

With Matugen installed, `node tests/real-matugen.cjs` exercises real image generation across all supported schemes. Set `OXIDYE_TEST_IMAGE=/path/to/image.jpg` when running the installation or native suite to include image-theme application and rendered mode switches.
