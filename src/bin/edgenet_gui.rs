#![cfg_attr(
    target_os = "windows",
    windows_subsystem = "windows"
)]

#[path = "../native_vpn.rs"]
mod native_vpn;

#[path = "../vpn.rs"]
mod vpn;

#[path = "../tap_driver.rs"]
mod tap_driver;

use eframe::egui;
use std::net::UdpSocket;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

const BLUE: egui::Color32 = egui::Color32::from_rgb(30, 116, 235);
const GREEN: egui::Color32 = egui::Color32::from_rgb(18, 184, 88);
const RED: egui::Color32 = egui::Color32::from_rgb(238, 73, 73);
const BG: egui::Color32 = egui::Color32::from_rgb(238, 242, 246);
const CARD: egui::Color32 = egui::Color32::from_rgb(248, 250, 252);
const CARD_ALT: egui::Color32 = egui::Color32::from_rgb(243, 247, 250);
const BORDER: egui::Color32 = egui::Color32::from_rgb(205, 214, 224);
const TEXT: egui::Color32 = egui::Color32::from_rgb(18, 31, 52);
const MUTED: egui::Color32 = egui::Color32::from_rgb(86, 104, 130);

#[derive(Debug)]
enum GuiEvent {
    PublicIp(String),
    ConnectStarted,
    ConnectFailed(String),
    DisconnectFinished,
}

struct EdgeNetApp {
    server: String,
    port: String,

    public_ip: String,
    public_ip_loading: bool,
    local_ip: String,

    vpn_ip: String,
    vpn_gateway: String,
    protocol: String,
    tun_name: String,

    status_text: String,
    last_error: String,
    show_details: bool,

    tx: Sender<GuiEvent>,
    rx: Receiver<GuiEvent>,
    last_poll: Instant,
}

impl EdgeNetApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&cc.egui_ctx);

        let (tx, rx) = mpsc::channel();

        let mut app = Self {
            server: vpn::DEFAULT_SERVER.to_string(),
            port: vpn::DEFAULT_PORT.to_string(),

            public_ip: "Detecting...".to_string(),
            public_ip_loading: false,
            local_ip: detect_local_ip(),

            vpn_ip: "-".to_string(),
            vpn_gateway: "-".to_string(),
            protocol: "-".to_string(),
            tun_name: "-".to_string(),

            status_text: "Disconnected".to_string(),
            last_error: String::new(),
            show_details: false,

            tx,
            rx,
            last_poll: Instant::now(),
        };

        app.refresh_public_ip();
        app
    }

    fn refresh_public_ip(&mut self) {
        if self.public_ip_loading {
            return;
        }

        self.public_ip_loading = true;
        self.public_ip = "Detecting...".to_string();

        let tx = self.tx.clone();

        thread::spawn(move || {
            let ip = detect_public_ip();
            let _ = tx.send(GuiEvent::PublicIp(ip));
        });
    }

    fn connect_vpn(&mut self) {
        let state = native_vpn::status();

        if matches!(
            state,
            native_vpn::VpnState::Connecting
                | native_vpn::VpnState::Connected
                | native_vpn::VpnState::Disconnecting
        ) {
            return;
        }

        let server = self.server.trim().to_string();
        if server.is_empty() {
            self.last_error = "Server cannot be empty.".to_string();
            return;
        }

        let port: u16 = match self.port.trim().parse() {
            Ok(port) if port != 0 => port,
            _ => {
                self.last_error = "Invalid port.".to_string();
                return;
            }
        };

        let config = match vpn::build_profile(&server, port) {
            Ok(config) => config,
            Err(error) => {
                self.last_error = error;
                return;
            }
        };

        self.last_error.clear();
        self.status_text = "Connecting...".to_string();
        self.vpn_ip = "-".to_string();
        self.vpn_gateway = "-".to_string();
        self.protocol = "-".to_string();
        self.tun_name = "-".to_string();

        let tx = self.tx.clone();

        thread::spawn(move || {
        let result = (|| -> Result<(), String> {
        tap_driver::ensure_tap_installed()?;
        native_vpn::connect(&config)?;
        Ok(())
            })();

            });
    }

    fn disconnect_vpn(&mut self) {
        let state = native_vpn::status();

        if !matches!(
            state,
            native_vpn::VpnState::Connected | native_vpn::VpnState::Connecting
        ) {
            return;
        }

        self.status_text = "Disconnecting...".to_string();
        let tx = self.tx.clone();

        thread::spawn(move || match native_vpn::disconnect() {
            Ok(_) => {
                let _ = tx.send(GuiEvent::DisconnectFinished);
            }
            Err(error) => {
                let _ = tx.send(GuiEvent::ConnectFailed(error));
            }
        });
    }

    fn restore_defaults(&mut self) {
        self.server = vpn::DEFAULT_SERVER.to_string();
        self.port = vpn::DEFAULT_PORT.to_string();
    }

    fn clear_runtime_values(&mut self) {
        self.vpn_ip = "-".to_string();
        self.vpn_gateway = "-".to_string();
        self.protocol = "-".to_string();
        self.tun_name = "-".to_string();
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                GuiEvent::PublicIp(ip) => {
                    self.public_ip = ip;
                    self.public_ip_loading = false;
                }
                GuiEvent::ConnectStarted => {
                    self.status_text = "Connecting...".to_string();
                }
                GuiEvent::ConnectFailed(error) => {
                    self.status_text = "Error".to_string();
                    self.last_error = error;
                }
                GuiEvent::DisconnectFinished => {
                    self.status_text = "Disconnected".to_string();
                    self.clear_runtime_values();
                }
            }
        }
    }

    fn poll_vpn(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(500) {
            return;
        }

        self.last_poll = Instant::now();

        match native_vpn::status() {
            native_vpn::VpnState::Disconnected => {
                self.status_text = "Disconnected".to_string();
                self.clear_runtime_values();
            }
            native_vpn::VpnState::Connecting => {
                self.status_text = "Connecting...".to_string();
            }
            native_vpn::VpnState::Connected => {
                self.status_text = "Connected".to_string();

                self.vpn_ip = non_empty_or_dash(native_vpn::virtual_ip());
                self.vpn_gateway = non_empty_or_dash(native_vpn::gateway());
                self.protocol = non_empty_or_dash(native_vpn::protocol());
                self.tun_name = non_empty_or_dash(native_vpn::tun_name());
            }
            native_vpn::VpnState::Disconnecting => {
                self.status_text = "Disconnecting...".to_string();
            }
            native_vpn::VpnState::Error => {
                self.status_text = "Error".to_string();
                let error = native_vpn::last_error();
                if !error.is_empty() {
                    self.last_error = error;
                }
            }
            native_vpn::VpnState::Unknown(code) => {
                self.status_text = format!("Unknown ({code})");
            }
        }
    }

    fn connection_active(&self) -> bool {
        matches!(
            native_vpn::status(),
            native_vpn::VpnState::Connecting
                | native_vpn::VpnState::Connected
                | native_vpn::VpnState::Disconnecting
        )
    }

    fn state_color(&self) -> egui::Color32 {
        match native_vpn::status() {
            native_vpn::VpnState::Connected => GREEN,
            native_vpn::VpnState::Connecting | native_vpn::VpnState::Disconnecting => BLUE,
            native_vpn::VpnState::Error => RED,
            native_vpn::VpnState::Disconnected | native_vpn::VpnState::Unknown(_) => MUTED,
        }
    }

    fn render_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("EdgeNet").size(25.0).strong().color(TEXT));
                ui.label(
                    egui::RichText::new("SECURE NETWORK")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("●")
                            .size(13.0)
                            .color(self.state_color()),
                    );
                    ui.label(
                        egui::RichText::new(&self.status_text)
                            .size(14.0)
                            .strong()
                            .color(self.state_color()),
                    );
                });
            });
        });
    }

    fn render_public_ip(&mut self, ui: &mut egui::Ui) {
        compact_card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Public IP").size(12.0).strong().color(MUTED));
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(&self.public_ip)
                            .size(20.0)
                            .strong()
                            .color(TEXT),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_copy = !self.public_ip_loading
                        && self.public_ip != "Unavailable"
                        && self.public_ip != "Detecting...";
                    if icon_button(ui, "Copy", can_copy).clicked() {
                        ui.ctx().copy_text(self.public_ip.clone());
                    }
                    if icon_button(ui, "↻", !self.public_ip_loading).clicked() {
                        self.refresh_public_ip();
                    }
                });
            });
        });
    }

    fn render_server(&mut self, ui: &mut egui::Ui) {
        let active = self.connection_active();
        compact_card().show(ui, |ui| {
            ui.label(egui::RichText::new("Server").size(12.0).strong().color(MUTED));
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                let total = ui.available_width();
                let port_w = 68.0;
                let host_w = (total - port_w - 8.0).max(180.0);
                ui.add_enabled(
                    !active,
                    egui::TextEdit::singleline(&mut self.server)
                        .desired_width(host_w)
                        .hint_text("Server"),
                );
                ui.add_enabled(
                    !active,
                    egui::TextEdit::singleline(&mut self.port)
                        .desired_width(port_w)
                        .hint_text("Port"),
                );
            });
        });
    }

    fn render_action(&mut self, ui: &mut egui::Ui) {
        let state = native_vpn::status();
        let connected = matches!(
            state,
            native_vpn::VpnState::Connected | native_vpn::VpnState::Connecting
        );

        let (label, color, enabled) = if connected {
            ("Disconnect", RED, true)
        } else {
            (
                if matches!(state, native_vpn::VpnState::Connecting) {
                    "Connecting..."
                } else {
                    "Connect"
                },
                BLUE,
                matches!(
                    state,
                    native_vpn::VpnState::Disconnected | native_vpn::VpnState::Error
                ),
            )
        };

        if ui
            .add_enabled(
                enabled,
                egui::Button::new(
                    egui::RichText::new(label)
                        .size(16.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .fill(color)
                .stroke(egui::Stroke::NONE)
                .corner_radius(egui::CornerRadius::same(9))
                .min_size(egui::vec2(ui.available_width(), 44.0)),
            )
            .clicked()
        {
            if connected {
                self.disconnect_vpn();
            } else {
                self.connect_vpn();
            }
        }
    }

    fn render_network(&self, ui: &mut egui::Ui) {
        egui::Frame::default()
            .fill(CARD)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(0))
            .show(ui, |ui| {
                egui::Grid::new("network_grid")
                    .num_columns(2)
                    .spacing([0.0, 0.0])
                    .min_col_width((ui.available_width() - 1.0) / 2.0)
                    .show(ui, |ui| {
                        mini_status(ui, "Local IP", &self.local_ip, BLUE);
                        mini_status(ui, "VPN IP", &self.vpn_ip, GREEN);
                        ui.end_row();
                        mini_status(ui, "Gateway", &self.vpn_gateway, MUTED);
                        mini_status(ui, "Protocol", &self.protocol, BLUE);
                        ui.end_row();
                    });
            });
    }

    fn render_error(&self, ui: &mut egui::Ui) {
        if self.last_error.is_empty() {
            return;
        }
        ui.add_space(8.0);
        egui::Frame::default()
            .fill(egui::Color32::from_rgb(255, 238, 238))
            .stroke(egui::Stroke::new(1.0, RED.linear_multiply(0.55)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(&self.last_error).size(11.0).color(RED));
            });
    }

    fn render_details(&mut self, ui: &mut egui::Ui) {
        let label = if self.show_details { "⌃  Details" } else { "⌄  Details" };
        let response = ui.add(
            egui::Button::new(egui::RichText::new(label).size(13.0).strong().color(TEXT))
                .fill(CARD)
                .stroke(egui::Stroke::new(1.0, BORDER))
                .corner_radius(egui::CornerRadius::same(9))
                .min_size(egui::vec2(ui.available_width(), 36.0)),
        );
        if response.clicked() {
            self.show_details = !self.show_details;
        }

        if !self.show_details {
            return;
        }

        ui.add_space(6.0);
        compact_card().show(ui, |ui| {
            detail_row(ui, "Server", &format!("{}:{}", self.server, self.port));
            detail_row(ui, "Public IP", &self.public_ip);
            detail_row(ui, "Local IP", &self.local_ip);
            detail_row(ui, "VPN IP", &self.vpn_ip);
            detail_row(ui, "Gateway", &self.vpn_gateway);
            detail_row(ui, "Protocol", &self.protocol);
            detail_row(ui, "Tunnel", &self.tun_name);
            detail_row(ui, "Engine", "OpenVPN3 Core");

            let native_error = native_vpn::last_error();
            if !native_error.is_empty() {
                detail_row(ui, "Native Error", &native_error);
            }
        });
    }

}

impl eframe::App for EdgeNetApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.process_events();
        self.poll_vpn();
        ui.ctx().request_repaint_after(Duration::from_millis(250));

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(egui::Margin::same(14)))
            .show(ui, |ui| {
                ui.set_max_width(360.0);
                ui.vertical_centered(|ui| {
                    ui.set_width(360.0);
                    self.render_header(ui);
                    ui.add_space(10.0);
                    self.render_public_ip(ui);
                    ui.add_space(8.0);
                    self.render_server(ui);
                    ui.add_space(8.0);
                    self.render_action(ui);
                    ui.add_space(8.0);
                    self.render_network(ui);
                    ui.add_space(8.0);
                    self.render_details(ui);
                    self.render_error(ui);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("v0.3.3").size(10.0).color(MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("OpenVPN3 Core").size(10.0).color(MUTED));
                        });
                    });
                });
            });
    }
}

impl Drop for EdgeNetApp {
    fn drop(&mut self) {
        let state = native_vpn::status();

        if matches!(
            state,
            native_vpn::VpnState::Connected | native_vpn::VpnState::Connecting
        ) {
            let _ = native_vpn::disconnect();
        }
    }
}

fn configure_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.extreme_bg_color = egui::Color32::WHITE;
    visuals.faint_bg_color = CARD_ALT;
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_fill = egui::Color32::WHITE;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(232, 239, 247);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(222, 232, 243);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.selection.bg_fill = BLUE.linear_multiply(0.18);
    visuals.selection.stroke = egui::Stroke::new(1.0, BLUE);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.spacing.item_spacing = egui::vec2(7.0, 5.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.text_edit_width = 180.0;
    ctx.set_style_of(egui::Theme::Light, style);
}

fn compact_card() -> egui::Frame {
    egui::Frame::default()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

fn icon_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).size(13.0).strong().color(BLUE))
            .fill(egui::Color32::WHITE)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .corner_radius(egui::CornerRadius::same(7))
            .min_size(egui::vec2(42.0, 32.0)),
    )
}

fn mini_status(ui: &mut egui::Ui, title: &str, value: &str, accent: egui::Color32) {
    egui::Frame::default()
        .fill(CARD)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_height(50.0);
            ui.label(egui::RichText::new(title).size(11.0).strong().color(MUTED));
            ui.add_space(2.0);
            ui.label(egui::RichText::new(value).size(15.0).strong().color(accent));
        });
}

fn detail_row(ui: &mut egui::Ui, name: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(name).size(11.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(11.0).color(TEXT));
        });
    });
}

fn non_empty_or_dash(value: String) -> String {
    if value.trim().is_empty() {
        "-".to_string()
    } else {
        value
    }
}

fn detect_local_ip() -> String {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => socket,
        Err(_) => return "-".to_string(),
    };

    // 不会真正发送 UDP 数据，只让 Windows 选择默认出口接口。
    if socket.connect("1.1.1.1:80").is_err() {
        return "-".to_string();
    }

    match socket.local_addr() {
        Ok(addr) => addr.ip().to_string(),
        Err(_) => "-".to_string(),
    }
}

fn detect_public_ip() -> String {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(_) => return "Unavailable".to_string(),
    };

    // 公网出口检测不依赖 EdgeNet VPN，第一个服务失败则尝试下一个。
    let services = [
        "https://api.ipify.org",
        "https://ifconfig.me/ip",
        "https://icanhazip.com",
    ];

    for url in services {
        let response = client
            .get(url)
            .header("User-Agent", "EdgeNet/0.3.3")
            .send();

        let response = match response {
            Ok(response) => response,
            Err(_) => continue,
        };

        if !response.status().is_success() {
            continue;
        }

        let text = match response.text() {
            Ok(text) => text,
            Err(_) => continue,
        };

        let ip = text.trim();
        if ip.parse::<std::net::IpAddr>().is_ok() {
            return ip.to_string();
        }
    }

    "Unavailable".to_string()
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("EdgeNet")
            .with_inner_size([390.0, 640.0])
            .with_min_inner_size([370.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "EdgeNet",
        options,
        Box::new(|cc| Ok(Box::new(EdgeNetApp::new(cc)))),
    )
}
