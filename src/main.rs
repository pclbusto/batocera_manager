mod app;
mod chd;
mod db;
mod diagnostic;
mod domain;
mod hash;
mod net;
mod sync;
mod systems;
mod ui;

pub use app::{APP_ID, APP_TITLE, BatoceraManagerApp, Message, Page};

use cosmic::app::Settings;
use cosmic::iced::Size;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::default().size(Size::new(1150.0, 780.0));
    cosmic::app::run::<BatoceraManagerApp>(settings, ())?;
    Ok(())
}
