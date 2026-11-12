mod client;
mod data;
mod json_highlighter;

use client::{RestInput, RestOutput, SocketEvent};
use data::{Folder, ParamRow, Route, RouteKind, Workspace};
use iced::futures::{Stream, StreamExt};
use iced::widget::{
    button, checkbox, column, container, mouse_area, pick_list, row, scrollable, text, text_editor,
    text_input,
};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length, Size, Subscription, Task, Theme,
};
use tokio::sync::mpsc::UnboundedSender;

const PLUM: Color = Color::from_rgb8(107, 46, 99);
const NAVY: Color = Color::from_rgb8(27, 54, 68);
const INK: Color = Color::from_rgb8(32, 49, 58);
const MUTED: Color = Color::from_rgb8(108, 124, 130);
const BORDER: Color = Color::from_rgb8(224, 228, 226);
const CANVAS: Color = Color::from_rgb8(246, 245, 244);
const SURFACE: Color = Color::WHITE;
const GREEN: Color = Color::from_rgb8(47, 122, 86);
const ERROR: Color = Color::from_rgb8(157, 46, 40);

const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
// shortcut: only the latest 100 socket messages stay in memory, raise this when session history is required.
const MAX_SOCKET_LOG: usize = 100;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Apui")
        .window_size(Size::new(860.0, 610.0))
        .resizable(false)
        .centered()
        .theme(|_: &App| Theme::Light)
        .subscription(App::subscription)
        .run()
}

fn json_format(color: &Color, _: &Theme) -> iced_core::text::highlighter::Format<iced_core::Font> {
    iced_core::text::highlighter::Format {
        color: Some(*color),
        font: Some(iced_core::Font::MONOSPACE),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorTab {
    Params,
    Headers,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DragItem {
    Folder(String),
    Route { folder: String, route: String },
}

#[derive(Debug, Clone)]
enum Message {
    Mode(RouteKind),
    NewRoute,
    RouteNameChanged(String),
    UrlChanged(String),
    MethodChanged(String),
    ParamEnabled(usize, bool),
    ParamNameChanged(usize, String),
    ParamValueChanged(usize, String),
    ParamTypeChanged(usize, String),
    ParamDescriptionChanged(usize, String),
    AddParam,
    RemoveParam(usize),
    HeadersChanged(text_editor::Action),
    BodyChanged(text_editor::Action),
    BeautifyJson,
    EditorTab(EditorTab),
    SendRest,
    RestFinished(Result<RestOutput, String>),
    CreateFolderName(String),
    CreateFolder,
    SelectFolder(String),
    SelectRoute(String, String),
    SaveRoute,
    MoveTarget(String),
    MoveRoute,
    MoveFolderUp(String),
    MoveFolderDown(String),
    MoveRouteUp(String, String),
    MoveRouteDown(String, String),
    ConnectSocket,
    DisconnectSocket,
    SocketEvent(SocketEvent),
    SocketUrlChanged(String),
    SocketHeadersChanged(text_editor::Action),
    SocketMessageChanged(String),
    SendSocketMessage,
    DragStart(DragItem),
    DragTarget(Option<DragItem>),
    FinishDrag,
}

struct App {
    workspace: Workspace,
    can_save: bool,
    mode: RouteKind,
    selected_folder: String,
    selected_route: Option<String>,
    route_name: String,
    method: String,
    url: String,
    params: Vec<ParamRow>,
    headers: text_editor::Content,
    body: text_editor::Content,
    tab: EditorTab,
    folder_name: String,
    move_target: String,
    busy: bool,
    response: Option<RestOutput>,
    notice: Option<String>,
    socket_url: String,
    socket_headers: text_editor::Content,
    socket_message: String,
    socket_log: Vec<(bool, String)>,
    socket_status: String,
    socket_session: Option<(u64, String, String)>,
    socket_sender: Option<UnboundedSender<String>>,
    next_socket_id: u64,
    drag_source: Option<DragItem>,
    drag_target: Option<DragItem>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let (workspace, can_save, notice) = match data::load() {
            Ok(workspace) => (workspace, true, None),
            Err(error) => (
                Workspace::default(),
                false,
                Some(format!("{error} No se sobrescribió el archivo.")),
            ),
        };
        let selected_folder = workspace
            .folders
            .first()
            .map(|folder| folder.id.clone())
            .unwrap_or_default();
        (
            Self {
                workspace,
                can_save,
                mode: RouteKind::Rest,
                selected_folder,
                selected_route: None,
                route_name: "New request".into(),
                method: "GET".into(),
                url: String::new(),
                params: Vec::new(),
                headers: text_editor::Content::new(),
                body: text_editor::Content::new(),
                tab: EditorTab::Params,
                folder_name: String::new(),
                move_target: String::new(),
                busy: false,
                response: None,
                notice,
                socket_url: String::new(),
                socket_headers: text_editor::Content::new(),
                socket_message: String::new(),
                socket_log: Vec::new(),
                socket_status: "Disconnected".into(),
                socket_session: None,
                socket_sender: None,
                next_socket_id: 1,
                drag_source: None,
                drag_target: None,
            },
            Task::none(),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        let socket = match &self.socket_session {
            Some(session) => Subscription::run_with(session.clone(), socket_subscription),
            None => Subscription::none(),
        };
        let mouse = iced::event::listen_with(|event, _, _| match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                Some(Message::FinishDrag)
            }
            _ => None,
        });
        Subscription::batch([socket, mouse])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Mode(mode) => {
                if self.mode != mode {
                    self.stop_socket();
                    if mode == RouteKind::WebSocket {
                        self.socket_url = self.url.clone();
                        self.socket_headers = text_editor::Content::with_text(&self.headers.text());
                    } else {
                        self.url = self.socket_url.clone();
                        self.headers = text_editor::Content::with_text(&self.socket_headers.text());
                    }
                    self.mode = mode;
                    self.notice = None;
                }
            }
            Message::NewRoute => self.new_route(),
            Message::RouteNameChanged(value) => self.route_name = value,
            Message::UrlChanged(value) => self.url = value,
            Message::MethodChanged(value) => self.method = value,
            Message::ParamEnabled(index, enabled) => {
                if let Some(param) = self.params.get_mut(index) {
                    param.enabled = enabled;
                }
            }
            Message::ParamNameChanged(index, value) => {
                if let Some(param) = self.params.get_mut(index) {
                    param.name = value;
                }
            }
            Message::ParamValueChanged(index, value) => {
                if let Some(param) = self.params.get_mut(index) {
                    param.value = value;
                }
            }
            Message::ParamTypeChanged(index, value) => {
                if let Some(param) = self.params.get_mut(index) {
                    param.kind = value;
                }
            }
            Message::ParamDescriptionChanged(index, value) => {
                if let Some(param) = self.params.get_mut(index) {
                    param.description = value;
                }
            }
            Message::AddParam => self.params.push(ParamRow::default()),
            Message::RemoveParam(index) => {
                if index < self.params.len() {
                    self.params.remove(index);
                }
            }
            Message::HeadersChanged(action) => self.headers.perform(action),
            Message::BodyChanged(action) => self.body.perform(action),
            Message::BeautifyJson => {
                match serde_json::from_str::<serde_json::Value>(&self.body.text()) {
                    Ok(value) => match serde_json::to_string_pretty(&value) {
                        Ok(formatted) => {
                            self.body = text_editor::Content::with_text(&formatted);
                            self.notice = None;
                        }
                        Err(error) => {
                            self.notice = Some(format!("No pude formatear el JSON: {error}"))
                        }
                    },
                    Err(error) => self.notice = Some(format!("JSON inválido: {error}")),
                }
            }
            Message::EditorTab(tab) => self.tab = tab,
            Message::SendRest => {
                if self.busy {
                    return Task::none();
                }
                self.busy = true;
                self.notice = None;
                self.response = None;
                return Task::perform(
                    client::send_rest(RestInput {
                        method: self.method.clone(),
                        url: self.url.clone(),
                        params: self.params.clone(),
                        headers: self.headers.text(),
                        body: self.body.text(),
                    }),
                    Message::RestFinished,
                );
            }
            Message::RestFinished(result) => {
                self.busy = false;
                match result {
                    Ok(response) => self.response = Some(response),
                    Err(error) => self.notice = Some(error),
                }
            }
            Message::CreateFolderName(value) => self.folder_name = value,
            Message::CreateFolder => self.create_folder(),
            Message::SelectFolder(id) => {
                if self.workspace.folders.iter().any(|folder| folder.id == id) {
                    self.selected_folder = id;
                    self.selected_route = None;
                    self.route_name = "New request".into();
                    self.clear_request();
                }
            }
            Message::SelectRoute(folder_id, route_id) => {
                self.load_route(&folder_id, &route_id);
            }
            Message::SaveRoute => self.save_route(),
            Message::MoveTarget(name) => {
                if let Some(folder) = self
                    .workspace
                    .folders
                    .iter()
                    .find(|folder| folder.name == name)
                {
                    self.move_target = folder.id.clone();
                }
            }
            Message::MoveRoute => self.move_route(),
            Message::MoveFolderUp(id) => self.move_folder(&id, -1),
            Message::MoveFolderDown(id) => self.move_folder(&id, 1),
            Message::MoveRouteUp(folder, route) => self.move_route_order(&folder, &route, -1),
            Message::MoveRouteDown(folder, route) => self.move_route_order(&folder, &route, 1),
            Message::ConnectSocket => self.connect_socket(),
            Message::DisconnectSocket => self.disconnect_socket(),
            Message::SocketEvent(event) => self.socket_event(event),
            Message::SocketUrlChanged(value) => self.socket_url = value,
            Message::SocketHeadersChanged(action) => self.socket_headers.perform(action),
            Message::SocketMessageChanged(value) => self.socket_message = value,
            Message::SendSocketMessage => self.send_socket_message(),
            Message::DragStart(item) => {
                self.drag_source = Some(item.clone());
                self.drag_target = Some(item);
            }
            Message::DragTarget(target) => self.drag_target = target,
            Message::FinishDrag => self.finish_drag(),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        row![self.sidebar(), self.main_panel()]
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let nav = row![
            nav_button("REST", RouteKind::Rest, self.mode),
            nav_button("WebSocket", RouteKind::WebSocket, self.mode)
        ]
        .spacing(5);

        let folder_input = row![
            text_input("New folder", &self.folder_name)
                .on_input(Message::CreateFolderName)
                .size(11)
                .padding(7)
                .style(input_style),
            button(text("+").size(14))
                .padding([5, 9])
                .on_press(Message::CreateFolder)
                .style(secondary_button_style),
        ]
        .spacing(5)
        .align_y(Alignment::Center);

        let mut entries = column![].spacing(5);
        for folder in &self.workspace.folders {
            entries = entries.push(self.folder_row(folder));
            for route in &folder.routes {
                entries = entries.push(self.route_row(folder, route));
            }
        }

        container(
            column![
                text("apui").size(20).color(Color::WHITE),
                nav,
                button(text("+  New request").size(11))
                    .width(Fill)
                    .padding([7, 6])
                    .on_press(Message::NewRoute)
                    .style(secondary_button_style),
                text("COLLECTIONS")
                    .size(10)
                    .color(Color::from_rgb8(175, 192, 197)),
                text("Drag to move or reorder")
                    .size(9)
                    .color(Color::from_rgb8(175, 192, 197)),
                folder_input,
                scrollable(entries).height(Fill),
                text("Local workspace")
                    .size(10)
                    .color(Color::from_rgb8(175, 192, 197)),
            ]
            .spacing(14)
            .padding(16),
        )
        .width(Length::Fixed(188.0))
        .height(Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(NAVY)),
            ..Default::default()
        })
        .into()
    }

    fn folder_row(&self, folder: &Folder) -> Element<'_, Message> {
        let selected = self.selected_folder == folder.id;
        let heading = button(
            text(format!("▾  {}", folder.name))
                .size(11)
                .color(Color::WHITE),
        )
        .width(Fill)
        .padding([6, 5])
        .on_press(Message::SelectFolder(folder.id.clone()))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if selected {
                Color::from_rgb8(45, 74, 87)
            } else {
                NAVY
            })),
            text_color: Color::WHITE,
            border: Border::default().rounded(4.0),
            ..Default::default()
        });
        mouse_area(
            row![
                heading,
                mouse_area(text("⋮⋮").size(10).color(Color::from_rgb8(175, 192, 197)))
                    .on_press(Message::DragStart(DragItem::Folder(folder.id.clone()))),
                small_button("↑", Message::MoveFolderUp(folder.id.clone())),
                small_button("↓", Message::MoveFolderDown(folder.id.clone())),
            ]
            .spacing(2)
            .align_y(Alignment::Center),
        )
        .on_enter(Message::DragTarget(Some(DragItem::Folder(
            folder.id.clone(),
        ))))
        .on_exit(Message::DragTarget(None))
        .into()
    }

    fn route_row(&self, folder: &Folder, route: &Route) -> Element<'_, Message> {
        let selected = self.selected_route.as_deref() == Some(route.id.as_str());
        let kind = if route.kind == RouteKind::Rest {
            &route.method
        } else {
            "WS"
        };
        let label = if route.name.trim().is_empty() {
            route.url.as_str()
        } else {
            route.name.as_str()
        };
        let open = button(
            text(format!("{kind}  {label}"))
                .size(10)
                .color(Color::from_rgb8(220, 227, 229)),
        )
        .width(Fill)
        .padding([5, 3])
        .on_press(Message::SelectRoute(folder.id.clone(), route.id.clone()))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if selected {
                Color::from_rgb8(45, 74, 87)
            } else {
                NAVY
            })),
            text_color: Color::WHITE,
            border: Border::default().rounded(3.0),
            ..Default::default()
        });
        mouse_area(
            row![
                iced::widget::Space::new().width(Length::Fixed(12.0)),
                open,
                mouse_area(text("⋮⋮").size(10).color(Color::from_rgb8(175, 192, 197))).on_press(
                    Message::DragStart(DragItem::Route {
                        folder: folder.id.clone(),
                        route: route.id.clone(),
                    })
                ),
                small_button(
                    "↑",
                    Message::MoveRouteUp(folder.id.clone(), route.id.clone())
                ),
                small_button(
                    "↓",
                    Message::MoveRouteDown(folder.id.clone(), route.id.clone())
                ),
            ]
            .spacing(2)
            .align_y(Alignment::Center),
        )
        .on_enter(Message::DragTarget(Some(DragItem::Route {
            folder: folder.id.clone(),
            route: route.id.clone(),
        })))
        .on_exit(Message::DragTarget(None))
        .into()
    }

    fn main_panel(&self) -> Element<'_, Message> {
        let header = row![
            text("My workspace  /  API client").size(12).color(INK),
            iced::widget::Space::new().width(Fill),
            text(if self.busy { "Sending…" } else { "Local" })
                .size(11)
                .color(MUTED),
        ]
        .align_y(Alignment::Center)
        .padding([3, 0]);

        let content = match self.mode {
            RouteKind::Rest => self.rest_panel(),
            RouteKind::WebSocket => self.socket_panel(),
        };
        container(column![header, content].spacing(14).padding(22))
            .width(Fill)
            .height(Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(CANVAS)),
                ..Default::default()
            })
            .into()
    }

    fn route_toolbar(&self) -> Element<'_, Message> {
        let name = text_input("Route name", &self.route_name)
            .on_input(Message::RouteNameChanged)
            .padding(8)
            .size(12)
            .style(input_style);
        let mut toolbar = row![name, primary_button("Save", Message::SaveRoute)]
            .spacing(8)
            .align_y(Alignment::Center);
        if self.selected_route.is_some() && !self.workspace.folders.is_empty() {
            let folder_names: Vec<String> = self
                .workspace
                .folders
                .iter()
                .map(|folder| folder.name.clone())
                .collect();
            let selected = self
                .workspace
                .folders
                .iter()
                .find(|folder| folder.id == self.move_target)
                .or_else(|| {
                    self.workspace
                        .folders
                        .iter()
                        .find(|folder| folder.id == self.selected_folder)
                })
                .map(|folder| folder.name.clone());
            toolbar = toolbar.push(
                pick_list(folder_names, selected, Message::MoveTarget)
                    .placeholder("Folder")
                    .width(Length::Fixed(124.0)),
            );
            toolbar = toolbar.push(secondary_button("Move", Message::MoveRoute));
        }
        toolbar.into()
    }

    fn rest_panel(&self) -> Element<'_, Message> {
        let methods: Vec<&str> = METHODS.to_vec();
        let url_bar = row![
            pick_list(methods, Some(self.method.as_str()), |method: &str| {
                Message::MethodChanged(method.to_owned())
            })
            .width(Length::Fixed(86.0)),
            text_input("https://api.example.com/path", &self.url)
                .on_input(Message::UrlChanged)
                .padding(9)
                .size(12)
                .style(input_style),
            container(primary_button(
                if self.busy { "Sending…" } else { "Send" },
                Message::SendRest,
            ))
            .width(Length::Fixed(72.0)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let tabs = row![
            tab_button("Params", EditorTab::Params, self.tab),
            tab_button("Headers", EditorTab::Headers, self.tab),
            tab_button("Body", EditorTab::Body, self.tab),
        ]
        .spacing(4);
        let editor: Element<'_, Message> = match self.tab {
            EditorTab::Params => self.params_view(),
            EditorTab::Headers => text_editor::<Message, Theme, iced::Renderer>(&self.headers)
                .placeholder("Header-Name: value  (one header per line)")
                .on_action(Message::HeadersChanged)
                .height(Length::Fixed(100.0))
                .padding(9)
                .size(11)
                .into(),
            EditorTab::Body => column![
                row![
                    text("application/json").size(10).color(MUTED),
                    iced::widget::Space::new().width(Fill),
                    secondary_button("Beautify", Message::BeautifyJson),
                ]
                .align_y(Alignment::Center),
                text_editor::<Message, Theme, iced::Renderer>(&self.body)
                    .placeholder("Request body")
                    .on_action(Message::BodyChanged)
                    .highlight_with::<json_highlighter::JsonHighlighter>((), json_format)
                    .height(Length::Fixed(88.0))
                    .padding(9)
                    .size(11),
            ]
            .spacing(5)
            .into(),
        };
        let editor_panel = match self.tab {
            EditorTab::Params => editor,
            EditorTab::Headers | EditorTab::Body => panel(editor),
        };
        let response_header = row![
            text("Response").size(14).color(INK),
            iced::widget::Space::new().width(Fill),
            self.response_badge(),
        ]
        .align_y(Alignment::Center);
        let response_body: Element<'_, Message> = match &self.response {
            Some(response) if response.body.is_empty() => {
                text("(empty response)").size(11).color(MUTED).into()
            }
            Some(response) => scrollable(
                text(response.body.clone())
                    .size(11)
                    .font(iced::Font::MONOSPACE)
                    .color(INK),
            )
            .height(Fill)
            .into(),
            None => container(text("Response body appears here.").size(11).color(MUTED))
                .padding(8)
                .into(),
        };

        column![
            self.route_toolbar(),
            url_bar,
            tabs,
            editor_panel,
            response_header,
            panel(response_body),
            self.notice_view(),
        ]
        .spacing(10)
        .height(Fill)
        .into()
    }

    fn params_view(&self) -> Element<'_, Message> {
        let heading = row![
            text("On").size(10).width(Length::Fixed(42.0)),
            text("Name").size(10).width(Length::Fixed(110.0)),
            text("Value").size(10).width(Length::Fixed(120.0)),
            text("Type").size(10).width(Length::Fixed(82.0)),
            text("Description").size(10).width(Fill),
            iced::widget::Space::new().width(Length::Fixed(50.0)),
        ]
        .spacing(4);
        let types = vec!["Text", "Number", "Boolean"];
        let mut rows = column![].spacing(4);
        for (index, param) in self.params.iter().enumerate() {
            rows = rows.push(
                row![
                    checkbox(param.enabled)
                        .label("On")
                        .size(12)
                        .spacing(3)
                        .on_toggle(move |enabled| Message::ParamEnabled(index, enabled))
                        .width(Length::Fixed(42.0)),
                    text_input("Name", &param.name)
                        .on_input(move |value| Message::ParamNameChanged(index, value))
                        .padding(6)
                        .size(10)
                        .style(input_style)
                        .width(Length::Fixed(110.0)),
                    text_input("Value", &param.value)
                        .on_input(move |value| Message::ParamValueChanged(index, value))
                        .padding(6)
                        .size(10)
                        .style(input_style)
                        .width(Length::Fixed(120.0)),
                    pick_list(types.clone(), Some(param.kind.as_str()), move |kind| {
                        Message::ParamTypeChanged(index, kind.to_owned())
                    })
                    .width(Length::Fixed(82.0)),
                    text_input("Description", &param.description)
                        .on_input(move |value| Message::ParamDescriptionChanged(index, value))
                        .padding(6)
                        .size(10)
                        .style(input_style)
                        .width(Fill),
                    button(text("Remove").size(10))
                        .padding([5, 4])
                        .width(Length::Fixed(50.0))
                        .on_press(Message::RemoveParam(index))
                        .style(secondary_button_style),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            );
        }
        column![
            container(heading).width(Fill).padding([0, 9]),
            panel(
                scrollable(rows)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::hidden(),
                    ))
                    .height(Length::Fixed(56.0)),
            ),
            secondary_button("Add row", Message::AddParam),
        ]
        .spacing(4)
        .into()
    }

    fn socket_panel(&self) -> Element<'_, Message> {
        let url_bar = row![
            text_input("wss://example.com/socket", &self.socket_url)
                .on_input(Message::SocketUrlChanged)
                .padding(9)
                .size(12)
                .style(input_style),
            if self.socket_session.is_some() {
                primary_button("Disconnect", Message::DisconnectSocket)
            } else {
                primary_button("Connect", Message::ConnectSocket)
            },
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let mut messages = column![].spacing(6);
        if self.socket_log.is_empty() {
            messages = messages.push(text("Messages will appear here.").size(11).color(MUTED));
        } else {
            for (outgoing, value) in &self.socket_log {
                messages = messages.push(
                    text(format!("{}  {value}", if *outgoing { "→" } else { "←" }))
                        .size(11)
                        .font(iced::Font::MONOSPACE)
                        .color(if *outgoing { PLUM } else { INK }),
                );
            }
        }
        let status_color = if self.socket_status == "Connected" {
            GREEN
        } else if self.socket_status.starts_with("Error") {
            ERROR
        } else {
            MUTED
        };
        column![
            self.route_toolbar(),
            url_bar,
            text("Headers  ·  one per line").size(10).color(MUTED),
            panel(
                text_editor::<Message, Theme, iced::Renderer>(&self.socket_headers)
                    .placeholder("Authorization: Bearer …")
                    .on_action(Message::SocketHeadersChanged)
                    .height(Length::Fixed(52.0))
                    .padding(8)
                    .size(11),
            ),
            row![text(&self.socket_status).size(11).color(status_color)].align_y(Alignment::Center),
            panel(scrollable(messages).height(Fill)),
            row![
                text_input("Message", &self.socket_message)
                    .on_input(Message::SocketMessageChanged)
                    .on_submit(Message::SendSocketMessage)
                    .padding(9)
                    .size(12)
                    .style(input_style),
                primary_button("Send", Message::SendSocketMessage),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            self.notice_view(),
        ]
        .spacing(8)
        .height(Fill)
        .into()
    }

    fn response_badge(&self) -> Element<'_, Message> {
        if self.busy {
            return text("Sending…").size(10).color(MUTED).into();
        }
        match &self.response {
            Some(response) => {
                let color = if response.status.starts_with('2') {
                    GREEN
                } else {
                    ERROR
                };
                row![
                    container(text(&response.status).size(10).color(color))
                        .padding([4, 7])
                        .style(move |_| container::Style {
                            background: Some(Background::Color(if color == GREEN {
                                Color::from_rgb8(232, 243, 236)
                            } else {
                                Color::from_rgb8(249, 235, 233)
                            })),
                            border: Border::default().rounded(4.0),
                            ..Default::default()
                        }),
                    text(format!("{} ms", response.elapsed_ms))
                        .size(10)
                        .color(MUTED),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into()
            }
            None => text("Ready").size(10).color(MUTED).into(),
        }
    }

    fn notice_view(&self) -> Element<'_, Message> {
        match &self.notice {
            Some(notice) => text(notice).size(10).color(ERROR).into(),
            None => iced::widget::Space::new().height(Length::Fixed(0.0)).into(),
        }
    }

    fn clear_request(&mut self) {
        self.method = "GET".into();
        self.url.clear();
        self.params.clear();
        self.headers = text_editor::Content::new();
        self.body = text_editor::Content::new();
        self.response = None;
        self.tab = EditorTab::Params;
        self.socket_url.clear();
        self.socket_headers = text_editor::Content::new();
        self.socket_log.clear();
    }

    fn new_route(&mut self) {
        self.stop_socket();
        self.selected_route = None;
        self.route_name = "New request".into();
        self.mode = RouteKind::Rest;
        self.clear_request();
        self.notice = None;
    }

    fn load_route(&mut self, folder_id: &str, route_id: &str) {
        let Some(route) = self
            .workspace
            .folders
            .iter()
            .find(|folder| folder.id == folder_id)
            .and_then(|folder| folder.routes.iter().find(|route| route.id == route_id))
            .cloned()
        else {
            return;
        };
        self.stop_socket();
        self.selected_folder = folder_id.to_owned();
        self.move_target = folder_id.to_owned();
        self.selected_route = Some(route.id);
        self.mode = route.kind;
        self.route_name = route.name;
        self.method = route.method;
        self.url = route.url.clone();
        self.params = route
            .param_rows
            .clone()
            .unwrap_or_else(|| data::legacy_params(&route.params));
        self.headers = text_editor::Content::with_text(&route.headers);
        self.body = text_editor::Content::with_text(&route.body);
        self.socket_url = route.url;
        self.socket_headers = text_editor::Content::with_text(&route.headers);
        self.response = None;
        self.notice = None;
        self.socket_log.clear();
    }

    fn create_folder(&mut self) {
        if !self.can_save {
            return;
        }
        let name = self.folder_name.trim();
        if name.is_empty() {
            self.notice = Some("Escribe un nombre para la carpeta.".into());
            return;
        }
        if self
            .workspace
            .folders
            .iter()
            .any(|folder| folder.name.eq_ignore_ascii_case(name))
        {
            self.notice = Some("Ya existe una carpeta con ese nombre.".into());
            return;
        }
        let previous = self.workspace.clone();
        let folder = Folder {
            id: data::new_id(),
            name: name.to_owned(),
            routes: Vec::new(),
        };
        self.selected_folder = folder.id.clone();
        self.workspace.folders.push(folder);
        if self.persist(previous) {
            self.folder_name.clear();
            self.notice = None;
        }
    }

    fn save_route(&mut self) {
        if !self.can_save {
            return;
        }
        let name = self.route_name.trim();
        if name.is_empty() {
            self.notice = Some("Escribe un nombre para la ruta.".into());
            return;
        }
        let Some(folder_index) = self
            .workspace
            .folders
            .iter()
            .position(|folder| folder.id == self.selected_folder)
        else {
            self.notice = Some("Selecciona una carpeta antes de guardar.".into());
            return;
        };
        let previous = self.workspace.clone();
        let id = self.selected_route.clone().unwrap_or_else(data::new_id);
        let route = Route {
            id: id.clone(),
            name: name.to_owned(),
            kind: self.mode,
            method: self.method.clone(),
            url: if self.mode == RouteKind::Rest {
                self.url.clone()
            } else {
                self.socket_url.clone()
            },
            params: data::legacy_params_text(&self.params),
            param_rows: Some(self.params.clone()),
            headers: if self.mode == RouteKind::Rest {
                self.headers.text()
            } else {
                self.socket_headers.text()
            },
            body: self.body.text(),
        };
        let routes = &mut self.workspace.folders[folder_index].routes;
        if let Some(existing) = routes.iter_mut().find(|route| route.id == id) {
            *existing = route;
        } else {
            routes.push(route);
        }
        self.selected_route = Some(id);
        if self.persist(previous) {
            self.notice = Some("Route saved".into());
        }
    }

    fn move_route(&mut self) {
        if !self.can_save {
            return;
        }
        let Some(route_id) = self.selected_route.clone() else {
            return;
        };
        if self.move_target.is_empty() || self.move_target == self.selected_folder {
            self.notice = Some("Elige otra carpeta para mover la ruta.".into());
            return;
        }
        let Some(source_index) = self
            .workspace
            .folders
            .iter()
            .position(|folder| folder.id == self.selected_folder)
        else {
            return;
        };
        let Some(route_index) = self.workspace.folders[source_index]
            .routes
            .iter()
            .position(|route| route.id == route_id)
        else {
            return;
        };
        let Some(target_folder) = self
            .workspace
            .folders
            .iter()
            .find(|folder| folder.id == self.move_target)
            .map(|folder| folder.id.clone())
        else {
            return;
        };
        let previous = self.workspace.clone();
        let route = self.workspace.folders[source_index]
            .routes
            .remove(route_index);
        let Some(target_index) = self
            .workspace
            .folders
            .iter()
            .position(|folder| folder.id == target_folder)
        else {
            self.workspace = previous;
            return;
        };
        self.workspace.folders[target_index].routes.push(route);
        let old_folder = self.selected_folder.clone();
        self.selected_folder = self.move_target.clone();
        if self.persist(previous) {
            self.notice = Some("Ruta movida.".into());
        } else {
            self.selected_folder = old_folder;
        }
    }

    fn move_folder(&mut self, id: &str, direction: isize) {
        if !self.can_save {
            return;
        }
        let Some(index) = self
            .workspace
            .folders
            .iter()
            .position(|folder| folder.id == id)
        else {
            return;
        };
        let target = index as isize + direction;
        if !(0..self.workspace.folders.len() as isize).contains(&target) {
            return;
        }
        let previous = self.workspace.clone();
        self.workspace.folders.swap(index, target as usize);
        self.persist(previous);
    }

    fn move_route_order(&mut self, folder_id: &str, route_id: &str, direction: isize) {
        if !self.can_save {
            return;
        }
        let Some(folder_index) = self
            .workspace
            .folders
            .iter()
            .position(|folder| folder.id == folder_id)
        else {
            return;
        };
        let Some(index) = self.workspace.folders[folder_index]
            .routes
            .iter()
            .position(|route| route.id == route_id)
        else {
            return;
        };
        let target = index as isize + direction;
        if !(0..self.workspace.folders[folder_index].routes.len() as isize).contains(&target) {
            return;
        }
        let previous = self.workspace.clone();
        self.workspace.folders[folder_index]
            .routes
            .swap(index, target as usize);
        self.persist(previous);
    }

    fn connect_socket(&mut self) {
        if self.socket_session.is_some() {
            return;
        }
        self.notice = None;
        self.socket_log.clear();
        let id = self.next_socket_id;
        self.next_socket_id = self.next_socket_id.wrapping_add(1);
        let url = self.socket_url.clone();
        let headers = self.socket_headers.text();
        self.socket_status = "Connecting…".into();
        self.socket_session = Some((id, url, headers));
    }

    fn disconnect_socket(&mut self) {
        if let Some(sender) = self.socket_sender.take() {
            drop(sender);
            self.socket_status = "Disconnecting…".into();
        } else {
            self.stop_socket();
        }
    }

    fn stop_socket(&mut self) {
        self.socket_session = None;
        self.socket_sender = None;
        self.socket_status = "Disconnected".into();
    }

    fn socket_event(&mut self, event: SocketEvent) {
        let id = match &event {
            SocketEvent::Connected { id, .. }
            | SocketEvent::Received { id, .. }
            | SocketEvent::Error { id, .. }
            | SocketEvent::Closed { id } => *id,
        };
        if !self
            .socket_session
            .as_ref()
            .is_some_and(|(active, _, _)| *active == id)
        {
            return;
        }
        match event {
            SocketEvent::Connected { sender, .. } => {
                self.socket_sender = Some(sender);
                self.socket_status = "Connected".into();
                self.push_socket_log(false, "Connected".into());
                self.notice = None;
            }
            SocketEvent::Received { text, .. } => self.push_socket_log(false, text),
            SocketEvent::Error { text, .. } => {
                self.socket_status = "Error".into();
                self.notice = Some(text);
                self.socket_sender = None;
                self.socket_session = None;
            }
            SocketEvent::Closed { .. } => {
                self.socket_status = "Disconnected".into();
                self.socket_sender = None;
                self.socket_session = None;
            }
        }
    }

    fn send_socket_message(&mut self) {
        let message = self.socket_message.clone();
        if message.is_empty() {
            return;
        }
        match &self.socket_sender {
            Some(sender) => match sender.send(message.clone()) {
                Ok(()) => {
                    self.push_socket_log(true, message);
                    self.socket_message.clear();
                    self.notice = None;
                }
                Err(_) => self.notice = Some("La conexión ya no está disponible.".into()),
            },
            None => self.notice = Some("Conecta el WebSocket antes de enviar.".into()),
        }
    }

    fn finish_drag(&mut self) {
        let (Some(source), Some(target)) = (self.drag_source.take(), self.drag_target.take())
        else {
            return;
        };
        if source == target || !self.can_save {
            return;
        }
        let previous = self.workspace.clone();
        match (source, target) {
            (DragItem::Folder(source), DragItem::Folder(target)) => {
                let Some(from) = self
                    .workspace
                    .folders
                    .iter()
                    .position(|item| item.id == source)
                else {
                    return;
                };
                let Some(to) = self
                    .workspace
                    .folders
                    .iter()
                    .position(|item| item.id == target)
                else {
                    return;
                };
                let folder = self.workspace.folders.remove(from);
                let insert_at = if from < to { to - 1 } else { to };
                self.workspace.folders.insert(insert_at, folder);
            }
            (DragItem::Route { folder, route }, DragItem::Folder(target)) => {
                self.move_route_to(&folder, &route, &target, None, &previous);
                return;
            }
            (
                DragItem::Route { folder, route },
                DragItem::Route {
                    folder: target_folder,
                    route: target_route,
                },
            ) => {
                self.move_route_to(
                    &folder,
                    &route,
                    &target_folder,
                    Some(&target_route),
                    &previous,
                );
                return;
            }
            (DragItem::Folder(_), DragItem::Route { .. }) => return,
        }
        self.persist(previous);
    }

    fn move_route_to(
        &mut self,
        source_folder: &str,
        route_id: &str,
        target_folder: &str,
        before_route: Option<&str>,
        previous: &Workspace,
    ) {
        let Some(source_index) = self
            .workspace
            .folders
            .iter()
            .position(|folder| folder.id == source_folder)
        else {
            return;
        };
        let Some(route_index) = self.workspace.folders[source_index]
            .routes
            .iter()
            .position(|route| route.id == route_id)
        else {
            return;
        };
        let Some(target_index) = self
            .workspace
            .folders
            .iter()
            .position(|folder| folder.id == target_folder)
        else {
            return;
        };
        let route = self.workspace.folders[source_index]
            .routes
            .remove(route_index);
        let destination = &mut self.workspace.folders[target_index].routes;
        let index = before_route
            .and_then(|target| destination.iter().position(|route| route.id == target))
            .unwrap_or(destination.len());
        destination.insert(index, route);
        let old_folder = self.selected_folder.clone();
        if self.selected_route.as_deref() == Some(route_id) {
            self.selected_folder = target_folder.to_owned();
        }
        if !self.persist(previous.clone()) {
            self.selected_folder = old_folder;
        }
    }

    fn push_socket_log(&mut self, outgoing: bool, message: String) {
        self.socket_log.push((outgoing, message));
        if self.socket_log.len() > MAX_SOCKET_LOG {
            self.socket_log.remove(0);
        }
    }

    fn persist(&mut self, previous: Workspace) -> bool {
        match data::save(&self.workspace) {
            Ok(()) => true,
            Err(error) => {
                self.workspace = previous;
                self.notice = Some(error);
                false
            }
        }
    }
}

fn nav_button(
    label: &'static str,
    kind: RouteKind,
    active: RouteKind,
) -> Element<'static, Message> {
    let selected = kind == active;
    button(text(label).size(11))
        .width(Fill)
        .padding([7, 6])
        .on_press(Message::Mode(kind))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if selected {
                Color::from_rgb8(45, 74, 87)
            } else {
                NAVY
            })),
            text_color: Color::WHITE,
            border: Border::default().rounded(4.0),
            ..Default::default()
        })
        .into()
}

fn small_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(10).color(Color::from_rgb8(208, 218, 220)))
        .padding([3, 4])
        .on_press(message)
        .style(|_, _| button::Style {
            background: None,
            text_color: Color::from_rgb8(208, 218, 220),
            ..Default::default()
        })
        .into()
}

fn primary_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(12))
        .padding([8, 12])
        .on_press(message)
        .style(|_, _| button::Style {
            background: Some(Background::Color(PLUM)),
            text_color: Color::WHITE,
            border: Border::default().rounded(5.0),
            ..Default::default()
        })
        .into()
}

fn secondary_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(11))
        .padding([7, 9])
        .on_press(message)
        .style(secondary_button_style)
        .into()
}

fn secondary_button_style(_: &Theme, _: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::WHITE)),
        text_color: INK,
        border: Border::default().rounded(5.0).width(1.0).color(BORDER),
        ..Default::default()
    }
}

fn tab_button(
    label: &'static str,
    tab: EditorTab,
    selected: EditorTab,
) -> Element<'static, Message> {
    button(
        text(label)
            .size(11)
            .color(if tab == selected { PLUM } else { MUTED }),
    )
    .padding([7, 10])
    .on_press(Message::EditorTab(tab))
    .style(move |_, _| button::Style {
        background: Some(Background::Color(if tab == selected {
            Color::from_rgb8(243, 238, 242)
        } else {
            SURFACE
        })),
        text_color: if tab == selected { PLUM } else { MUTED },
        border: Border::default().rounded(4.0),
        ..Default::default()
    })
    .into()
}

fn input_style(_: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(SURFACE),
        border: Border::default().rounded(5.0).width(1.0).color(
            if matches!(status, text_input::Status::Focused { .. }) {
                PLUM
            } else {
                BORDER
            },
        ),
        icon: MUTED,
        placeholder: MUTED,
        value: INK,
        selection: Color::from_rgb8(229, 210, 227),
    }
}

fn panel<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .width(Fill)
        .padding(8)
        .style(|_| container::Style {
            background: Some(Background::Color(SURFACE)),
            border: Border::default().rounded(5.0).width(1.0).color(BORDER),
            ..Default::default()
        })
        .into()
}

fn socket_subscription(
    data: &(u64, String, String),
) -> std::pin::Pin<Box<dyn Stream<Item = Message> + Send + 'static>> {
    Box::pin(client::socket_worker(data).map(Message::SocketEvent))
}
