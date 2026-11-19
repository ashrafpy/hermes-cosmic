mod app;
mod hermes;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::HermesApplet>(())
}
