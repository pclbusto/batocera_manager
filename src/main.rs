mod app;
mod ui;

// Core compartido: se consume desde la lib en lugar de recompilarlo en el binario.
use batocera_manager::{chd, db, diagnostic, domain, net, services, sync, systems};

pub use app::{APP_ID, APP_TITLE, BatoceraManagerApp, Message, Page};

use cosmic::app::Settings;
use cosmic::iced::Size;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::default().size(Size::new(1150.0, 780.0));
    cosmic::app::run::<BatoceraManagerApp>(settings, ())?;
    Ok(())
}
