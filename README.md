# Apui

Apui is a small desktop client for REST requests and WebSocket connections. The initial window is 860 × 610 px and uses the approved plum and deep blue palette.

## Run

```sh
cargo run
```

## AppImage

Build the x86_64 AppImage in the pinned Ubuntu-compatible Docker image:

```sh
packaging/build-appimage.sh
```

The artifact is written to `outputs/Apui-<version>-x86_64.AppImage` and can be attached to a GitHub release.

The app supports `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD` and `OPTIONS`. Edit query parameters in the Params table (Name, Value, Type and Description), disable rows to omit them from the URL, and add or remove rows as needed. Type and Description are saved as metadata; query values are sent as text. Headers use `Name: value` lines. The Body tab edits raw text with JSON syntax colors and a Beautify action; invalid JSON stays unchanged and reports an error. Send shows the HTTP status, elapsed time and response body. JSON responses are formatted for reading; responses larger than 2 MiB are truncated in the viewer.

Switch to WebSocket to connect to `ws://` or `wss://`, optionally provide request headers, send text messages, and read the connection status, incoming messages and errors. The log keeps the latest 100 entries for the current connection.

## Saved routes

Create a folder in the left sidebar, select it, then use **New request**. Set a route name and its fields, then choose **Save**. Selecting a saved route loads it for editing; saving updates that route. The folder picker and **Move** button move a saved route between folders. Drag a folder or route by its handle to reorder it; drop a route onto a folder to move it. The up/down controls provide keyboard-accessible ordering actions.

Workspace data is stored at `$XDG_CONFIG_HOME/apui/workspace.json`, or `$HOME/.config/apui/workspace.json` when `XDG_CONFIG_HOME` is not set. Drag folders to reorder them, and drag routes to reorder within a folder or drop them into another folder. The up/down controls offer the same operations by keyboard and pointer. Writes use a same-directory temporary file and atomic rename; the file is created with owner-only permissions on Unix. If the existing file is unreadable or invalid, Apui reports the problem and disables writes rather than replacing it.

Routes saved by earlier versions with `params` as `key=value` lines are loaded into enabled Text rows. Saving the route keeps a compatible `params` string and stores the full table, including enabled state, type and description.

## Design

The editable design is [`design/apui.figcraft`](design/apui.figcraft). Previews: [Params](design/previews/apui-workspace-860x610.png) and [Body JSON](design/previews/apui-body-860x610.png). See [`design/interface-spec.md`](design/interface-spec.md) and [`design/ui-inventory.md`](design/ui-inventory.md).
