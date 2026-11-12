# UI inventory

| Element | Variants and states | Implementation | Used in |
| --- | --- | --- | --- |
| Navigation button | REST/WebSocket; selected/idle | `nav_button` | Sidebar |
| Primary button | Plum action; idle/sending | `primary_button` | Send, Connect, Disconnect, Save |
| Secondary button | Light surface, bordered | `secondary_button` | Create folder, move |
| Text input | Idle/focused; valid/invalid value | `input_style` | Folder name, route name, URL, WebSocket message |
| Folder row | Selected/idle; drag reorder; move up/down | `folder_row` | Sidebar |
| Route row | Selected/idle; method or WS; drag reorder/move; move up/down | `route_row` | Sidebar folder tree |
| Editor tab | Params/Headers/Body; selected/idle | `tab_button` | REST editor |
| Params table | Enabled/disabled row; Name, Value, Type, Description; add/remove row | `params_view` with shared inputs and `ParamRow` data | REST Params tab |
| JSON body editor | Empty/filled/focused; syntax colors; Beautify success/error | Iced `text_editor` with `JsonHighlighter` and `json_format`; `BeautifyJson` | REST Body tab |
| Text editor | Headers and WebSocket headers; empty/filled/focused | Iced `text_editor` inside `panel` | REST Headers and WebSocket headers |
| Panel | Surface with border; response/editor/log content | `panel` | Request editor, response body, WebSocket headers and log |
| Response status | Ready/sending, 2xx, other status, error | `response_badge`, `notice_view` | REST response |
| WebSocket state | Connecting/connected/disconnecting/disconnected/error | `socket_status` + `notice_view` | WebSocket view |
| Message log row | Outgoing/incoming | `socket_log` renderer | WebSocket view |

## Tokens

| Token | Value |
| --- | --- |
| Plum | `#6b2e63` |
| Navy | `#1b3644` |
| Canvas | `#f6f5f4` |
| Surface | `#ffffff` |
| Ink | `#20313a` |
| Muted | `#6c7c82` |
| Border | `#e0e4e2` |

Controls use visible focus borders and text labels. Keyboard input works through Iced's native buttons, pick list, text inputs and multiline editors.
