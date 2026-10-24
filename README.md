# linkman

**Symlinks, without the guesswork.**

A light, fast symlink studio for the terminal: a beautiful dual-pane TUI with
full mouse support, and a sharp CLI for scripts. Create, inspect, retarget,
audit and remove links — safely.

```text
  ▌ linkman   symlink studio
   l  link   L  rename   r  rel on   f  force off   .  hidden off   ?  help
────────────────────────────────────────────────────────────────────────────────
╭ SOURCE 5 items ── ~/projects/app ╮ ╭ DEST 2 items ───── ~/bin ╮
│    NAME                   INFO   │ │    NAME               INFO │
│  ↑ ..                            │ │  ↑ ..                      │
│▌ ▸ src/                    dir    │ │▌ ↗ app                link │
│  · Cargo.toml              2.1 KB │ │  ↗ tool               link │
│  ↗ target                   link  │ │                            │
╰──────────────────────────────────╯ ╰────────────────────────────╯
```

## Highlights

- **Two panes, one job.** SOURCE left, DEST right. Mark files, press `l`, done
  — no `ln -s` path gymnastics.
- **Find anything.** `Ctrl+P` opens a floating fuzzy finder over every action
  and every file in both panes, grouped into sections.
- **Safe by default.** `unlink` only ever removes symlinks, `--force` refuses
  to replace a real directory, conflicts ask first, and everything can be
  undone (`u`) — and redone (`U`) if you undo by accident.
- **Seven palettes.** Press `T` to preview and pick a theme live: `black`,
  Catppuccin Mocha, Tokyo Night, Dracula, Gruvbox, Everforest, GitHub Light.
- **Mouse-native.** Click, double-click, right-click to mark, scroll, hit the
  toolbar and every dialog button.
- **Small and fast.** One Rust binary, no daemon, no database, starts instantly.

## Install

**Arch Linux (AUR)**

```sh
yay -S linkman-bin        # or: paru -S linkman-bin
```

**Debian / Ubuntu**

```sh
sudo apt install ./linkman_<version>_amd64.deb
```

**Any Linux — static build (no glibc version needed)**

```sh
gh release download -R zaidejjo/linkman -p 'linkman-*-x86_64-unknown-linux-musl.tar.gz'
tar -xzf linkman-*-x86_64-unknown-linux-musl.tar.gz
sudo install -m755 linkman-*/linkman /usr/local/bin/linkman
```

**macOS**

```sh
gh release download -R zaidejjo/linkman -p 'linkman-*-apple-darwin.tar.gz'
tar -xzf linkman-*-apple-darwin.tar.gz
sudo install -m755 linkman-*/linkman /usr/local/bin/linkman
```

**With cargo**

```sh
cargo install --git https://github.com/zaidejjo/linkman linkman
```

**From source**

```sh
git clone https://github.com/zaidejjo/linkman && cd linkman
cargo build --release
install -m755 target/release/linkman ~/.local/bin/linkman
```

Every release also ships a `.deb`, tarballs for x86_64 glibc and aarch64
Linux, and a `SHA256SUMS.txt` — grab them from the
[Releases](https://github.com/zaidejjo/linkman/releases) page.

Check it worked:

```sh
linkman --version
```

## Quick start

```sh
linkman                                  # both panes start in the current folder
linkman tui ~/projects/app --dest ~/bin  # source folder + destination folder
```

Pick something in **SOURCE** (`j`/`k` or the mouse), move to where the links
should live (`Tab` switches panes), press `l`.

The first launch writes a commented starter config to
`~/.config/linkman/config.jsonc` and tells you about it in the status line.

## The TUI

### Linking

1. Move to an item in **SOURCE** (`j`/`k`, arrows, or the mouse).
2. Optionally press `Space` to mark several items.
3. Navigate **DEST** to where the links should live (`Tab` switches panes).
4. Press `l`.

Links are absolute by default. Toggle `r` for relative links, `f` to overwrite
without asking. When a name already exists you get a dialog:

- **Overwrite** — replace the existing entry (`o`)
- **Backup and overwrite** — keep the old entry as `<name>.bak` (`b`)
- **Skip existing** — create only the new ones (`s`)
- **Cancel** — change your mind (`c` / `Esc`)

Existing links in DEST can be retargeted at the selected SOURCE with `t`, or
removed with `x` (never the file they point at).

### Find anything with Ctrl+P

Press `Ctrl+P` outside a prompt, confirmation or the theme picker to open a
floating fuzzy finder. It searches every action **and every file or folder in
both panes**. Results are grouped under section headings — `LINKS`,
`HISTORY`, `VIEW`, `GO`, `LOOK`, `APP`, then `SOURCE` and `DEST` files — and
sections are ordered by their best match, so a strong hit still comes first:

```text
╭ Command palette ─────────────────────────────────────────╮
│  ⌕  note                                       10 results│
│──────────────────────────────────────────────────────────│
│  LINKS  3                                                │
│  Remove symlinks                        x · targets stay…│
│  SOURCE  2                                               │
│▌ notes.md                           SOURCE · ~/projects/…│
╰──────────────────────────────────────────────────────────╯
```

`↑↓` or the wheel to move, `Enter` or a click to run, `Esc` to close. Choosing
a folder opens it; choosing a file reveals and selects it in its pane.

The matcher is fzf-flavoured: word starts outrank the middle of words, title
matches outrank subtitles, contiguous runs beat scattered letters, and
initials work (`tgrel` → “Toggle relative”). Typing a capital letter makes the
search case sensitive, so `Link` will not surface `unlink`.

### Undo and redo

Every link, retarget and removal in the current session can be undone with `u`
or `Ctrl+Z`. Undo is deliberately careful: a link is only removed if it still
points exactly where linkman put it, backups are restored, removed links are
recreated with their original raw target (relative stays relative), and
regular files are never deleted.

Hit it by accident? `U` or `Ctrl+Y` redoes it, with the same checks: redo
never overwrites an occupied path and never removes a changed link. A new
operation clears the redo history. Both directions honour `undo_limit`
(default 50).

### Keys

| Key | Action |
| --- | --- |
| `↑ ↓` / `j k` | move |
| `Enter` / `→` | open folder |
| `←` / `h` / `Backspace` | parent folder |
| `Tab` / `Shift+Tab` | switch pane |
| `1` / `2` | focus SOURCE / DEST |
| `g` / `G`, `Home` / `End` | top / bottom |
| `PgUp` / `PgDn`, `Ctrl+U` / `Ctrl+D` | page |
| `Ctrl+P` | fuzzy command palette |
| `T` | choose a theme (live preview) |
| `Ctrl+L` | jump to a path |
| `~` | home folder |
| `/` | filter by name |
| `.` | show/hide dotfiles |
| `Space` | mark / unmark |
| `a` | mark all |
| `Esc` | clear filter, then marks |
| `l` | link SOURCE → DEST |
| `L` / `n` | link with a custom name |
| `t` | retarget the selected DEST link to SOURCE |
| `u` / `Ctrl+Z` | undo the last link, retarget or removal |
| `U` / `Ctrl+Y` | redo the operation you just undid |
| `r` | relative / absolute links |
| `f` | force overwrite |
| `x` / `Delete` | remove selected/marked symlinks |
| `R` / `Ctrl+R` | refresh |
| `?` | help |
| `q` / `Ctrl+C` | quit |

### Mouse

- **Click** a row to focus and select it
- **Double-click** a folder to open it
- **Right-click** to mark / unmark
- **Wheel** to scroll
- **Click** the toolbar, a palette result, and dialog buttons

The footer always keeps `q quit` within reach; the status line shows live
badges for relative/absolute mode, force, hidden files, and marked items.

### Themes

Press `T` (or `Ctrl+P` → “Choose theme”) to open the picker. Every palette is
listed wearing its own colours; moving the cursor previews it live across the
whole TUI, `Enter` keeps it and `Esc` puts the old one back.

| Name | Look |
| --- | --- |
| `black` | deep slate + mint · default |
| `catppuccin-mocha` | mauve pastels on deep indigo |
| `tokyo-night` | blue-violet night |
| `dracula` | purple night with vivid green |
| `gruvbox` | warm retro brown |
| `everforest` | muted forest green |
| `github-light` | clean light theme for bright terminals |

The picker's choice lasts for the session — set `"theme"` in the config to
make it permanent. Aliases such as `mocha`, `tokyonight`, `light` and
`default` work too; an unknown name falls back to `black` with a warning.

## Config

linkman reads its TUI preferences from JSON **or JSONC** (comments and
trailing commas are fine):

- `$XDG_CONFIG_HOME/linkman/config.json` / `config.jsonc`
- `~/.config/linkman/config.json` / `config.jsonc`
- or an explicit path in `$LINKMAN_CONFIG`

```jsonc
{
  // TUI startup defaults
  "relative": false,
  "force": false,
  "show_hidden": false,
  "undo_limit": 50,        // 1–500
  "theme": "black",        // see Themes above

  // optional starting folders; CLI arguments always win
  "source": "~/projects",
  "dest": "~/bin"
}
```

The first TUI launch creates the file for you; after that it is yours —
linkman never rewrites it. `linkman config --init` writes one from the CLI,
`linkman config` shows the path and the effective defaults. Unknown keys and
syntax errors are reported as a warning instead of stopping the TUI. Config
preferences apply to the TUI; explicit CLI flags always win for CLI
operations.

## CLI

### Create

```sh
linkman link ./dist ~/bin/app             # ~/bin/app -> /current/dist
linkman link ./dist                       # ./dist -> /current/dist
linkman link ./dist -t ~/bin -r           # ~/bin/dist -> ../dist
linkman link ./dist -t ~/bin -n app       # custom link name
```

| Flag | Meaning |
| --- | --- |
| `-t, --into <DIR>` | create the link inside DIR using the target's name |
| `-n, --name <NAME>` | custom name with `--into` |
| `-r, --relative` | store a relative target |
| `-f, --force` | replace an existing file/symlink |
| `-b, --backup` | move the existing entry to `<name>.bak` |
| `-p, --parents` | create missing parent directories |
| `--dry-run` | show what would happen |
| `--json` | machine-readable outcome |

A dangling target is allowed and clearly flagged — useful for links you'll
populate later.

### Remove

```sh
linkman unlink ~/bin/app
linkman unlink build dist cache --dry-run
```

Only symlinks are removed. Regular files and directories are refused.

### Inspect

```sh
linkman list ~/bin                   # top-level audit
linkman list . -r                    # recursive; paths relative to DIR
linkman info ~/bin/app               # follow the chain: target, health, perms
linkman doctor . --json              # find broken links and loops
```

`list` and `doctor` exit `1` when they find broken or looped links, so they
drop cleanly into scripts and CI:

```sh
linkman doctor . || echo "fix your links"
linkman list ~/bin --json | jq '.links[] | select(.status != "ok")'
```

### Global

```sh
linkman --help
linkman --version
linkman config                     # show config path + effective defaults
linkman config --init              # write a commented starter config
linkman completions bash           # bash, zsh, fish, elvish, powershell
```

Every inspection command accepts `--json` and prints stable, pretty-printed
output.

## Shell completions

```sh
# bash
linkman completions bash > ~/.local/share/bash-completion/completions/linkman

# zsh (make sure ~/.zfunc is in your $fpath)
linkman completions zsh > ~/.zfunc/_linkman

# fish
linkman completions fish > ~/.config/fish/completions/linkman.fish
```

Completions cover every command, alias and flag — and they are already
installed for you by the AUR package and the `.deb`.

## Safety notes

- Removing a link never touches its target.
- `--force` will not replace a real directory.
- `--backup` moves the old entry aside instead of deleting it.
- TUI undo refuses to remove a link that no longer matches what linkman wrote;
  redo refuses to overwrite an occupied path.
- Broken links are first-class: they can be created, listed, found and fixed.
- Paths are resolved lexically; nothing is canonicalized behind your back.

## Development

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The same checks run on every push and pull request. The filesystem engine
lives in `src/fsops.rs`, the CLI in `src/cli.rs`, the TUI in `src/tui/`
(`app` state + `ui` rendering).

## License

MIT
