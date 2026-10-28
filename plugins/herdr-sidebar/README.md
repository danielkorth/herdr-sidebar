# herdr-sidebar

**The sidebar your terminal was missing** — a VS Code-inspired file explorer + source
control panel in one dockable herdr pane.

<img src="docs/media/hero.png" alt="The sidebar docked beside a 2x2 fleet of Claude Code and Codex agents" width="860">

**The full tour lives in the [repo README](../../README.md)** — features, screenshots,
keys, and settings.

Quick Open is `Ctrl+P`; project text search has its own activity view and opens with `Ctrl+F`
or `Ctrl+Shift+F`. It searches live with match-case, whole-word, regex, and include/exclude filters.
Herdr keybindings can invoke `show-explorer`, `show-search`, `show-git`, or `quick-open`
directly (`-windows` suffix on Windows), so chords such as `Cmd+P` are remappable by the host.
The Git footer keeps branch switching and sync one click away in every view and can be hidden
from Settings.
Common image formats render directly in the preview pane; videos show a poster frame when
`ffmpeg` is available on `PATH`.
The built-in editor has Vim-style NORMAL mode by default (`h/j/k/l`, `w/b`, `dd`, `yy`,
`p`, `u`, counts, visual `v`, and more); `i` enters INSERT, Esc returns to NORMAL, and
`:w`, `:q`, `:wq` save/close through the usual conflict and unsaved-change prompts. `:preview`
leaves edit mode. The footer shows the active mode and common keys.

To open clicked files in a separate terminal editor, configure "Custom editor…" in sidebar
Settings, then enable "Use editor on click". Clicking an already-open file focuses its existing
editor tab; keyboard Enter continues to use the built-in preview.

## Install

Requires herdr 0.8 or newer. Source builds require Rust 1.89 or newer.

```
herdr plugin install alexarthurs/herdr-sidebar/plugins/herdr-sidebar
```

or from a local checkout:

```
cargo build --release
herdr plugin link .
```

Open it (or just focus a tab — the hook docks it):

```
herdr plugin action invoke herdr-sidebar.open-sidebar-windows   # windows
herdr plugin action invoke herdr-sidebar.open-sidebar           # linux / macos
```
