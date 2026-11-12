# Interface specification

**Status:** approved and implemented  
**Editable source:** [apui.figcraft](apui.figcraft), frame `Apui — API workspace`  
**Desktop window:** fixed at 860 × 610 px  
**Params preview:** [apui-workspace-860x610.png](previews/apui-workspace-860x610.png)  
**Body preview:** [apui-body-860x610.png](previews/apui-body-860x610.png)

## Workspace

The left 188 px navigation rail uses `#1b3644`. It contains REST and WebSocket mode buttons, a new request action, a folder name field, the saved folder tree, and drag handles. Route rows show the HTTP method or `WS`; selecting a row loads it into the editor. Drag folders to reorder them. Drag routes onto another route to reorder, or onto a folder to move them. Up/down buttons provide the same ordering actions without dragging.

The main panel uses a warm neutral canvas, white fields and panels, ink text, and `#6b2e63` for primary actions, selected tabs and focus. Surfaces have light borders and small corner radii. Inputs keep visible labels or placeholders and keyboard focus styling.

## REST flow

1. Create a request in the selected folder or select a saved route.
2. Set its name, HTTP method, URL and one editor tab: Params, Headers or Body. Params are editable rows with Name, Value, Type and Description; disabled rows are omitted from the request URL. Add or remove rows as needed. Type and Description are saved metadata; values are sent as text.
3. Send the request. The result shows the HTTP status, elapsed time and response body. A non-2xx status remains a response, while network, validation and body-read failures appear as errors.
4. Save new or edited routes. Choose a different folder and move a saved route when needed.

Headers accept `Name: value` lines. Legacy routes with `params` stored as `key=value` lines migrate to enabled Text rows when opened. Saving preserves the compatible `params` string and stores the full table. Supported methods are GET, POST, PUT, PATCH, DELETE, HEAD and OPTIONS. HTTP URLs must use `http` or `https`; each request has a 60 second timeout. Response rendering is capped at 2 MiB.

The Body tab edits raw text with JSON syntax colors and a Beautify action. Beautify changes the editor only after parsing valid JSON; invalid JSON stays untouched and displays a parse error.

## WebSocket flow

The WebSocket view accepts `ws` and `wss` URLs plus optional `Name: value` headers. Connect reports `Connecting`, `Connected`, `Disconnecting`, `Disconnected` or `Error`. The message log distinguishes outgoing and incoming text; binary frames are shown as lossy UTF-8 text. Disconnect closes the stream. The log retains at most 100 messages per connection.

## Persistence and error states

Folders and routes are stored in `workspace.json` under the user's config directory. Each successful create, edit, move or reorder is written atomically. Invalid JSON, duplicate IDs, duplicate folder names or invalid entries cause a visible startup error and disable writes so the existing file is not overwritten. Save failures restore the in-memory workspace snapshot and show the error.

## Components and tokens

Color tokens: plum `#6b2e63`, navy `#1b3644`, canvas `#f6f5f4`, surface `#ffffff`, ink `#20313a`, muted `#6c7c82`, border `#e0e4e2`. The interface uses the platform sans-serif for controls and a monospace face for response and WebSocket text. Spacing is based on 4, 8, 10, 14 and 22 px steps. Panels use 4–6 px corners; focus borders use plum.

The same button, input, panel, route-row and editor-tab builders are shared across REST and WebSocket views; see [UI inventory](ui-inventory.md). The Params table and JSON body editor use the same shared text-input and panel styles.
