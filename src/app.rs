use crate::hermes::{self, CpuSample, GatewayStatus, ServiceAction};
use cosmic::app::{Core, Task};
use cosmic::applet::padded_control;
use cosmic::iced::alignment::Alignment;
use cosmic::iced::window::Id;
use cosmic::iced::{Border, Color, Length, Limits, Rectangle, Shadow, Subscription};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::theme;
use cosmic::widget::{self, button, column, container, divider, icon, row, text};
use cosmic::{Action, Application, Element};
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "io.github.hermes.CosmicAppletHermes";
const POPUP_WIDTH: f32 = 360.0;

const ICON_IDLE_SVG: &[u8] = include_bytes!(
    "../data/icons/hicolor/scalable/apps/io.github.hermes.CosmicAppletHermes-symbolic.svg"
);
const ICON_ACTIVE_DARK_SVG: &[u8] = include_bytes!(
    "../data/icons/hicolor/scalable/apps/io.github.hermes.CosmicAppletHermes-active-symbolic.svg"
);
const ICON_ACTIVE_LIGHT_SVG: &[u8] = include_bytes!(
    "../data/icons/hicolor/scalable/apps/io.github.hermes.CosmicAppletHermes-active-light-symbolic.svg"
);

const PLATFORM_TELEGRAM_SVG: &[u8] = include_bytes!("../data/icons/platforms/telegram.svg");
const PLATFORM_DISCORD_SVG: &[u8] = include_bytes!("../data/icons/platforms/discord.svg");
const PLATFORM_WHATSAPP_SVG: &[u8] = include_bytes!("../data/icons/platforms/whatsapp.svg");
const PLATFORM_WEIXIN_SVG: &[u8] = include_bytes!("../data/icons/platforms/weixin.svg");
const PLATFORM_SIGNAL_SVG: &[u8] = include_bytes!("../data/icons/platforms/signal.svg");
const PLATFORM_MATRIX_SVG: &[u8] = include_bytes!("../data/icons/platforms/matrix.svg");
const PLATFORM_SLACK_SVG: &[u8] = include_bytes!("../data/icons/platforms/slack.svg");
const PLATFORM_GENERIC_SVG: &[u8] = include_bytes!("../data/icons/platforms/generic.svg");

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup(Rectangle<i32>),
    PopupClosed(Id),
    PollStatus,
    StatusUpdate(GatewayStatus),
    StartService,
    StopService,
    RestartService,
    OpenDashboard,
    OpenChat,
    ActionResult(Result<(), String>),
}

pub struct HermesApplet {
    core: Core,
    popup: Option<Id>,
    status: GatewayStatus,
    action_error: Option<String>,
    action_in_progress: bool,
}

impl HermesApplet {
    fn prev_cpu(&self) -> Option<(CpuSample, String)> {
        self.status
            .resource_stats
            .as_ref()
            .map(|s| (s.cpu_sample, s.cpu_usage.clone()))
    }

    fn current_icon_bytes(&self) -> &'static [u8] {
        if self.status.is_running {
            if theme::is_dark() {
                ICON_ACTIVE_DARK_SVG
            } else {
                ICON_ACTIVE_LIGHT_SVG
            }
        } else {
            ICON_IDLE_SVG
        }
    }

    fn symbolic_icon(&self, size: u16) -> cosmic::widget::Icon {
        let handle = icon::from_svg_bytes(self.current_icon_bytes()).symbolic(true);
        widget::icon(handle)
            .class(theme::Svg::Custom(Rc::new(|theme| {
                cosmic::iced::widget::svg::Style {
                    color: Some(theme.cosmic().background(theme.transparent).on.into()),
                }
            })))
            .width(Length::Fixed(size as f32))
            .height(Length::Fixed(size as f32))
    }

    fn popup_icon(&self, size: u16) -> cosmic::widget::Icon {
        let handle = icon::from_svg_bytes(self.current_icon_bytes()).symbolic(true);
        widget::icon(handle)
            .class(theme::Svg::Custom(Rc::new(|theme| {
                cosmic::iced::widget::svg::Style {
                    color: Some(theme.cosmic().background(false).on.into()),
                }
            })))
            .width(Length::Fixed(size as f32))
            .height(Length::Fixed(size as f32))
    }

    fn render_overview(&self) -> Element<'_, Message> {
        let status_subtitle = if self.status.is_running {
            let uptime_part = self
                .status
                .uptime
                .as_deref()
                .map(|u| format!(" · up {u}"))
                .unwrap_or_default();
            format!("Online{uptime_part}")
        } else {
            "Offline".to_string()
        };

        let header_left = row([])
            .align_y(Alignment::Center)
            .spacing(8)
            .width(Length::Fill)
            .push(self.popup_icon(22))
            .push(
                column([])
                    .spacing(1)
                    .push(text::title4("Hermes Gateway"))
                    .push(text::caption(status_subtitle)),
            );

        let refresh_btn = button::icon(icon::from_name("view-refresh-symbolic"))
            .extra_small()
            .on_press(Message::PollStatus);

        let header = padded_control(
            row([])
                .align_y(Alignment::Center)
                .spacing(8)
                .push(header_left)
                .push(refresh_btn),
        );

        let mut body = column([]).spacing(4).padding([8, 0]).push(header);
        body = body.push(padded_control(divider::horizontal::default()));

        if let Some(err) = &self.action_error {
            let err_row = row([])
                .align_y(Alignment::Center)
                .spacing(6)
                .push(
                    widget::icon(
                        icon::from_name("dialog-warning-symbolic")
                            .size(14)
                            .symbolic(true)
                            .into(),
                    )
                    .size(14),
                )
                .push(text::caption(err).width(Length::Fill));
            body = body.push(padded_control(err_row));
        }

        // Resource stats card (when running)
        if let Some(stats) = &self.status.resource_stats {
            let stats_row = row([])
                .align_y(Alignment::Center)
                .spacing(12)
                .push(
                    column([])
                        .spacing(2)
                        .width(Length::Fill)
                        .push(text::caption("Memory"))
                        .push(text::body(&stats.memory)),
                )
                .push(
                    column([])
                        .spacing(2)
                        .width(Length::Fill)
                        .push(text::caption("CPU"))
                        .push(text::body(&stats.cpu_usage)),
                )
                .push(
                    column([])
                        .spacing(2)
                        .push(text::caption("PID"))
                        .push(text::body(stats.pid.to_string())),
                );
            body = body.push(padded_control(solid_card(stats_row)));
        }

        // Connected platforms card (single-line: "Platform      <logo(s)>")
        let connected: Vec<_> = self
            .status
            .connected_platforms
            .iter()
            .filter(|p| p.connected)
            .collect();
        if !connected.is_empty() {
            let label = if connected.len() == 1 {
                "Platform"
            } else {
                "Platforms"
            };
            let mut logos_row = row([]).align_y(Alignment::Center).spacing(8);
            for platform in connected {
                logos_row = logos_row.push(platform_logo(&platform.name, true));
            }
            let platform_row = row([])
                .align_y(Alignment::Center)
                .spacing(8)
                .push(container(text::body(label)).width(Length::Fill))
                .push(logos_row);
            body = body.push(padded_control(solid_card(platform_row)));
        }

        // Profiles card
        if !self.status.profiles.is_empty() {
            let mut profiles_col = column([]).spacing(6);
            profiles_col = profiles_col.push(text::caption("PROFILES"));
            for profile in &self.status.profiles {
                let badge = if profile.is_current { "✓ " } else { "" };
                let pid_text = profile
                    .pid
                    .map(|p| format!("PID {p}"))
                    .unwrap_or_else(|| "stopped".to_string());

                profiles_col = profiles_col.push(
                    row([])
                        .align_y(Alignment::Center)
                        .spacing(8)
                        .push(
                            container(text::body(format!("{badge}{}", profile.name)))
                                .width(Length::Fill),
                        )
                        .push(text::caption(pid_text)),
                );
            }
            body = body.push(padded_control(solid_card(profiles_col)));
        } else if !self.status.is_running {
            body = body.push(padded_control(text::caption(
                "Gateway is not running · press Start to launch",
            )));
        }

        body = body.push(padded_control(divider::horizontal::light()));

        // Quick launch actions: Open Dashboard + Quick Chat
        let dash_label = if self.status.dashboard_running {
            "Dashboard :9119"
        } else {
            "Launch Dashboard"
        };
        let quick_row = padded_control(
            row([])
                .align_y(Alignment::Center)
                .spacing(8)
                .push(
                    button::standard(dash_label)
                        .width(Length::Fill)
                        .on_press(Message::OpenDashboard),
                )
                .push(
                    button::standard("Quick Chat")
                        .width(Length::Fill)
                        .on_press(Message::OpenChat),
                ),
        );
        body = body.push(quick_row);

        body = body.push(padded_control(divider::horizontal::light()));

        // Service lifecycle controls
        let service_row: Element<'_, Message> = if self.action_in_progress {
            padded_control(
                row([])
                    .align_y(Alignment::Center)
                    .spacing(8)
                    .push(text::body("Working…")),
            )
            .into()
        } else if self.status.is_running {
            padded_control(
                row([])
                    .align_y(Alignment::Center)
                    .spacing(8)
                    .push(container(row([])).width(Length::Fill))
                    .push(button::destructive("Stop").on_press(Message::StopService))
                    .push(button::standard("Restart").on_press(Message::RestartService)),
            )
            .into()
        } else {
            padded_control(
                row([])
                    .align_y(Alignment::Center)
                    .spacing(8)
                    .push(container(row([])).width(Length::Fill))
                    .push(button::suggested("Start").on_press(Message::StartService)),
            )
            .into()
        };

        body = body.push(service_row);

        body.into()
    }
}

fn platform_logo<'a>(name: &str, connected: bool) -> Element<'a, Message> {
    let (bytes, symbolic) = match name.to_ascii_lowercase().as_str() {
        "telegram" => (PLATFORM_TELEGRAM_SVG, false),
        "discord" => (PLATFORM_DISCORD_SVG, false),
        "whatsapp" => (PLATFORM_WHATSAPP_SVG, false),
        "weixin" | "wechat" => (PLATFORM_WEIXIN_SVG, false),
        "signal" => (PLATFORM_SIGNAL_SVG, false),
        "matrix" => (PLATFORM_MATRIX_SVG, true),
        "slack" => (PLATFORM_SLACK_SVG, false),
        _ => (PLATFORM_GENERIC_SVG, true),
    };

    let handle = icon::from_svg_bytes(bytes).symbolic(symbolic);
    let mut img = widget::icon(handle)
        .width(Length::Fixed(20.0))
        .height(Length::Fixed(20.0));

    if symbolic {
        img = img.class(theme::Svg::Custom(Rc::new(move |theme| {
            let mut color: Color = theme.cosmic().background(false).component.on.into();
            if !connected {
                color.a = 0.4;
            }
            cosmic::iced::widget::svg::Style { color: Some(color) }
        })));
    }

    img.into()
}

fn solid_card<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(10)
        .width(Length::Fill)
        .class(theme::Container::Custom(Box::new(|theme| {
            let cosmic = theme.cosmic();
            let palette = cosmic.background(false);
            let mut bg: Color = palette.component.base.into();
            bg.a = 1.0;
            let mut border_color: Color = palette.component.divider.into();
            border_color.a = 1.0;

            cosmic::iced::widget::container::Style {
                text_color: Some(palette.component.on.into()),
                background: Some(bg.into()),
                border: Border {
                    radius: cosmic.corner_radii.radius_s.into(),
                    width: 1.0,
                    color: border_color,
                },
                shadow: Shadow::default(),
                icon_color: Some(palette.component.on.into()),
                snap: true,
            }
        })))
        .into()
}

impl Application for HermesApplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let app = Self {
            core,
            popup: None,
            status: GatewayStatus::default(),
            action_error: None,
            action_in_progress: false,
        };
        let initial_fetch = Task::perform(hermes::fetch_status(None), |status| {
            Action::App(Message::StatusUpdate(status))
        });
        (app, initial_fetch)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        cosmic::iced::time::every(Duration::from_secs(5)).map(|_| Message::PollStatus)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup(anchor_rect) => {
                if let Some(id) = self.popup.take() {
                    return cosmic::task::message(Action::Surface(destroy_popup(id)));
                }

                let action = app_popup::<HermesApplet>(
                    |_| Default::default(),
                    move |state: &mut HermesApplet| {
                        let new_id = Id::unique();
                        state.popup = Some(new_id);
                        state.action_error = None;
                        let mut popup_settings = state.core.applet.get_popup_settings(
                            state.core.main_window_id().unwrap(),
                            new_id,
                            None,
                            None,
                            None,
                        );
                        popup_settings.positioner.anchor_rect = anchor_rect;
                        popup_settings.positioner.size_limits = Limits::NONE
                            .min_width(POPUP_WIDTH)
                            .max_width(POPUP_WIDTH)
                            .min_height(120.0)
                            .max_height(800.0);
                        popup_settings
                    },
                    Some(Box::new(move |state: &HermesApplet| {
                        Element::from(state.core.applet.popup_container(state.render_overview()))
                            .map(Action::App)
                    })),
                );
                let open_task = cosmic::task::message(Action::Surface(action));
                let prev_cpu = self.prev_cpu();
                let refresh_task = Task::perform(hermes::fetch_status(prev_cpu), |status| {
                    Action::App(Message::StatusUpdate(status))
                });
                return Task::batch([open_task, refresh_task]);
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            Message::PollStatus => {
                let prev_cpu = self.prev_cpu();
                return Task::perform(hermes::fetch_status(prev_cpu), |status| {
                    Action::App(Message::StatusUpdate(status))
                });
            }
            Message::StatusUpdate(status) => {
                self.status = status;
                self.action_in_progress = false;
            }
            Message::StartService => {
                self.action_in_progress = true;
                self.action_error = None;
                return Task::perform(hermes::service_action(ServiceAction::Start), |res| {
                    Action::App(Message::ActionResult(res))
                });
            }
            Message::StopService => {
                self.action_in_progress = true;
                self.action_error = None;
                return Task::perform(hermes::service_action(ServiceAction::Stop), |res| {
                    Action::App(Message::ActionResult(res))
                });
            }
            Message::RestartService => {
                self.action_in_progress = true;
                self.action_error = None;
                return Task::perform(hermes::service_action(ServiceAction::Restart), |res| {
                    Action::App(Message::ActionResult(res))
                });
            }
            Message::OpenDashboard => {
                self.action_error = None;
                return Task::perform(hermes::open_dashboard(), |res| {
                    Action::App(Message::ActionResult(res))
                });
            }
            Message::OpenChat => {
                self.action_error = None;
                return Task::perform(hermes::open_chat(), |res| {
                    Action::App(Message::ActionResult(res))
                });
            }
            Message::ActionResult(result) => {
                self.action_in_progress = false;
                if let Err(e) = result {
                    self.action_error = Some(e);
                } else {
                    self.action_error = None;
                    let prev_cpu = self.prev_cpu();
                    return Task::perform(
                        async move {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            hermes::fetch_status(prev_cpu).await
                        },
                        |status| Action::App(Message::StatusUpdate(status)),
                    );
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let suggested = self.core.applet.suggested_size(true);
        let (major_pad, minor_pad) = self.core.applet.suggested_padding(true);
        let icon_size = (suggested.0 + 4).min(suggested.1 + 2 * minor_pad);
        let icon_el = self.symbolic_icon(icon_size);

        let on_press_rect = |offset: cosmic::iced::Vector, bounds: Rectangle| {
            let anchor_rect = Rectangle {
                x: (bounds.x - offset.x) as i32,
                y: (bounds.y - offset.y) as i32,
                width: bounds.width as i32,
                height: bounds.height as i32,
            };
            Message::TogglePopup(anchor_rect)
        };

        let btn = if self.core.applet.is_horizontal() {
            button::custom(
                widget::layer_container(icon_el)
                    .center_y(Length::Fixed(f32::from(suggested.1 + 2 * minor_pad))),
            )
            .padding([0, major_pad])
            .class(theme::Button::AppletIcon)
            .on_press_with_rectangle(on_press_rect)
        } else {
            button::custom(
                widget::layer_container(icon_el)
                    .center_x(Length::Fixed(f32::from(suggested.0 + 2 * minor_pad))),
            )
            .padding([major_pad, 0])
            .class(theme::Button::AppletIcon)
            .on_press_with_rectangle(on_press_rect)
        };

        self.core.applet.autosize_window(btn).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        self.core.applet.popup_container(self.render_overview()).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
