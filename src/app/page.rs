#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Sync,
    Roms,
    ChdConverter,
    Settings,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Dashboard => "Panel Principal",
            Page::Sync => "Sincronización por Sistemas",
            Page::Roms => "Explorador y Copiado de Juegos",
            Page::ChdConverter => "Conversor CHD",
            Page::Settings => "Configuración",
        }
    }

    pub fn icon_name(self) -> &'static str {
        match self {
            Page::Dashboard => "speedometer-symbolic",
            Page::Sync => "emblem-synchronizing-symbolic",
            Page::Roms => "applications-games-symbolic",
            Page::ChdConverter => "media-optical-symbolic",
            Page::Settings => "preferences-system-symbolic",
        }
    }
}
