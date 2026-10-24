# linkman

**Symlinks, without the guesswork.**

`linkman` is a light, fast symlink studio for the terminal: a beautiful dual-pane
TUI with full mouse support, plus a sharp CLI for scripts. Create, inspect,
retarget, audit and remove links — safely.

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

## Why linkman

- **Two panes, one job.** SOURCE on the left, DEST on the right. Mark files,
  press `l`, done. No `ln -s` path gymnastics.
- **Beautifully quiet.** Seven hand-tuned palettes (the slate/mint `black`
  default, Catppuccin Mocha, Tokyo Night, Dracula, Gruvbox, Everforest and
  GitHub Light), a real inspector, live status, and dialogs that stay out of
  your way.
- **Find anything.** `Ctrl+P` opens a floating fuzzy palette over actions, files
  and folders from both panes.
- **Safe by default.** `unlink` only ever removes symlinks. `--force` refuses
  to replace a real directory. Conflicts ask before overwriting, and the last
  operation can always be undone — and redone if you undo by accident.
- **Mouse-native.** Click rows, double-click folders, right-click to mark,
  scroll the wheel, hit the toolbar, the palette, and dialog buttons.
- **Small and fast.** No daemon, no database. A single Rust binary that starts
  instantly.
- **Yours.** JSON/JSONC config, generated shell completions, and a scriptable
  CLI with `--json`; `doctor` and `list` exit non-zero when they find problems.

## Install

```sh
cargo install --path .
# or
cargo build --release
install -m755 target/release/linkman ~/.local/bin/linkman
```

## The TUI

Run `linkman` with no arguments, or point it at folders:

```sh
linkman                                  # both panes start in the current folder
linkman tui ~/projects/app --dest ~/bin  # source folder + destination folder
```

### Linking

1. Move to an item in **SOURCE** (use `j`/`k`, arrows, or the mouse).
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

Press `Ctrl+P` outside a prompt, confirmation or theme picker to open a
floating fuzzy finder. It searches every action **and every file or folder in
both panes**. Results are grouped under section headings — `LINKS`,
`HISTORY`, `VIEW`, `GO`, `LOOK`, `APP`, then `SOURCE` and `DEST` files — and
sections are ordered by their best match, so a strong hit still comes first.
Type fragments of a name, a path, or an action description:

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

The matcher is fzf-flavoured: word starts outrank the middle of words,
title matches outrank subtitles, contiguous runs beat scattered letters, and
initials work (`tgrel` → “Toggle relative”). Typing a capital letter makes the
search case sensitive, so `Link` will not surface `unlink`. Paths shown in both
panes are listed once.

### Undo and redo

Every link, retarget and removal in the current session can be undone with `u`
or `Ctrl+Z`. Undo is deliberately careful:

- a link is only removed if it still points exactly where linkman put it;
- backups created during linking are restored;
- removed links are recreated with their original raw target (relative stays
  relative);
- regular files are never deleted, and an externally retargeted link is left
  untouched.

Because undo is one keystroke away, it is easy to hit it by accident. Press
`U` or `Ctrl+Y` to redo the operation the undo just reversed. Redo re-checks
the filesystem too: it will not overwrite a path that has become occupied, it
only removes a link if it still points where it did when it was removed, and
it will never clobber a replaced entry that had no backup. Starting a new
operation clears the redo history. Both directions honour `undo_limit` from
the config (default 50).

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

## CLI

### Create

```sh
linkman link ./dist ~/bin/app             # ~/bin/app -> /current/dist
linkman link ./dist                      # ./dist -> /current/dist
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
linkman list . -r                    # recursive; paths shown relative to DIR
linkman list ~/bin --json
linkman info ~/bin/app
linkman info ~/bin/app --json
```

`info` follows the link chain, shows the raw and resolved target, health,
permissions, size, and modification time.

### Health check

```sh
linkman doctor ~/projects
linkman doctor . --shallow           # only the top folder
linkman doctor . --json
```

Finds broken links and loops. Exit code `1` when issues exist, so it drops
cleanly into CI:

```sh
linkman doctor . || echo "fix your links"
```

`list` also exits `1` when it finds broken or looped links.

### Global

```sh
linkman --help
linkman --version
linkman config                     # show config path + effective defaults
linkman config --init              # write a commented starter config
linkman completions bash           # bash, zsh, fish, elvish, powershell
```

## Config

linkman reads its TUI preferences from JSON **or JSONC** (comments and
trailing commas are fine):

- `$XDG_CONFIG_HOME/linkman/config.json`
- `$XDG_CONFIG_HOME/linkman/config.jsonc`
- `~/.config/linkman/config.json` / `config.jsonc`
- or an explicit path in `$LINKMAN_CONFIG`

```jsonc
{
  // TUI startup defaults
  "relative": false,
  "force": false,
  "show_hidden": false,
  "undo_limit": 50,        // 1–500
  "theme": "black",        // see Themes below

  // optional starting folders; CLI arguments always win
  "source": "~/projects",
  "dest": "~/bin"
}
```

Create one with `linkman config --init`, or just start the TUI: the first
launch creates `~/.config/linkman/config.jsonc` for you (and the status line
says so). An existing file is never overwritten or rewritten — edit it by hand.
Unknown keys and syntax errors are reported as a warning rather
than stopping the TUI. Config preferences apply to the TUI; explicit CLI flags
remain in charge of CLI operations.

## Themes

The TUI ships with seven palettes. `black` is the default: a deep slate canvas
with mint highlights and amber links.

| Name | Look |
| --- | --- |
| `black` | deep slate + mint · default |
| `catppuccin-mocha` | mauve pastels on deep indigo |
| `tokyo-night` | blue-violet night |
| `dracula` | purple night with vivid green |
| `gruvbox` | warm retro brown |
| `everforest` | muted forest green |
| `github-light` | clean light theme for bright terminals |

Press `T` (or open the palette with `Ctrl+P` and pick “Choose theme”) to see
the whole list in a floating picker. Each row wears its own colours; moving the
cursor previews that palette live across the whole TUI. `Enter` keeps it and
`Esc` restores the one you had. The choice lasts for the session — put the name
in the config to make it permanent:

```jsonc
{ "theme": "tokyo-night" }
```

Aliases such as `mocha`, `tokyonight`, `light` and `default` also work. An
unknown name falls back to `black` with a warning in the status line.

## Shell completions

```sh
# bash
linkman completions bash > ~/.local/share/bash-completion/completions/linkman

# zsh (make sure ~/.zfunc is in your $fpath)
linkman completions zsh > ~/.zfunc/_linkman

# fish
linkman completions fish > ~/.config/fish/completions/linkman.fish

# PowerShell
linkman completions powershell | Out-String | Invoke-Expression
```

Completions cover every command, alias, and flag.

## JSON

Every inspection command speaks JSON:

```json
{
  "path": "/home/me/bin/app",
  "target": "../projects/app/dist",
  "resolved": "/home/me/projects/app/dist",
  "status": "ok",
  "kind": "dir"
}
```

`--json` output is pretty-printed and stable enough for `jq`:

```sh
linkman doctor . --json | jq '.issues[] | .path'
linkman list ~/bin --json | jq '.links[] | select(.status != "ok")'
```

## Safety notes

- Removing a link never touches its target.
- `--force` will not replace a real directory.
- `--backup` moves the old entry aside instead of deleting it.
- TUI undo refuses to remove a link that no longer matches what linkman wrote.
- TUI redo refuses to overwrite an occupied path or remove a changed link.
- Broken links are first-class: they can be created, listed, found, and fixed.
- Paths are resolved lexically; nothing is canonicalized behind your back.

## Development

```sh
cargo test
cargo clippy --all-targets
cargo fmt
```

The filesystem engine lives in `src/fsops.rs`; the CLI in `src/cli.rs`; the TUI
in `src/tui/` (`app` state + `ui` rendering).

## License

MIT
