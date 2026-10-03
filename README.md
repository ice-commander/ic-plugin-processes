# Processes

An Ice Commander plugin that lists the machine's running processes in a file
panel, as a table.

## What it does

It registers the panel source `processes` (title "Processes") and a toolbar
button on the right (priority 20, tooltip "Processes") that opens it. The
desktop host also offers the source on the panel's source selector. Opening it
replaces the panel's current content with a table of four columns:

- **Name** — the process name; rows are sorted by it, case ignored;
- **PID** — the key column: the host hands it back as the selection;
- **Memory** — `B` below 1 KB, whole `KB` below 1 MB, then `MB` and `GB` with
  one decimal (1024-based);
- **CPU** — `sysinfo`'s per-process usage, one decimal and `%`.

Every time the host asks for rows the plugin builds a new `sysinfo::System`
and reads the process table; nothing is watched or refreshed in the
background.

While the source is shown, the panel carries two actions of the plugin, both
clickable whenever they are shown:

- **Back** (`processes.back`) — closes the source; the host restores what the
  panel showed before.
- **End process** (`processes.kill`) — ends every selected PID, then reopens
  the source, which redraws the list; with nothing selected it only redraws.
  A PID that is no longer running, or that could not be ended, is logged as a
  warning through the host and the rest of the selection is still tried. The
  action is registered with `IC_ENABLE_ON_FILE`, but the desktop host does not
  apply enable flags to panel actions.

Ending a process is `sysinfo`'s `kill()`: `SIGKILL` on macOS and Linux,
`taskkill /PID <pid> /F` on Windows, with the rights of the user running Ice
Commander.

No extensions, filesystems, views, dialogues or locale catalogues.

## Known limitations

- **End process** acts at once, without a confirmation.
- After **End process**, **Back** goes to the process list again instead of
  what the panel showed before: the reopen makes the host take the list itself
  as the view to restore.
- The interface is English only: the title, both tooltips, the column headings
  and the log messages are plain English strings.
- The plugin registers no refresh, search or sort action of its own.
- CPU comes from two refreshes made back to back inside one reading, closer
  together than `sysinfo`'s `MINIMUM_CPU_UPDATE_INTERVAL`, so it is a rough
  figure.

## Building

```sh
./build.sh          # release build; the library is copied into bin/
./test.sh           # cargo test --workspace
./deploy-local.sh   # copies bin/ into the per-user plugin folder
```

`deploy-local.sh` installs into `~/Library/Application Support/ice-commander/plugins`
on macOS, `%APPDATA%\ice-commander\plugins` on Windows and
`${XDG_DATA_HOME:-~/.local/share}/ice-commander/plugins` elsewhere; set
`IC_PLUGIN_DIR` to use another folder. It removes only libraries it deployed
earlier that are no longer built. Then switch the plugin on in
**Settings → Plugins** and restart.

Four tests read the live process table, so the result depends on the machine
they run on; one of them tries to end PID `u32::MAX` and expects "not running".

The version comes from `package.json`; `node builder/gen-version.js` writes it
into `version.rs`. `ic-plugin-api` is taken from
`https://github.com/ice-commander/plugin-api.git` (branch `main`).

## Licence

MIT or Apache-2.0, at your option, except the SVG icons in
`src/processes-panel/assets/`: they are from Icons8 and are not covered by
these licences — see [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md).
Contributions are taken under the DCO; sign off with `git commit -s`.
