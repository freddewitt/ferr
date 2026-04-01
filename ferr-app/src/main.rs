// ferr-app est un crate GUI en cours de développement.
// Les constantes de thème et les champs d'état sont définis par avance
// pour l'interface à venir — les warnings dead_code sont attendus.
#![allow(dead_code)]

mod app;
mod bridge;
mod state;
mod theme;
mod ui;

use app::FerrApp;
use iced::{application, window, Size};

fn main() -> iced::Result {
    application("ferr", FerrApp::update, FerrApp::view)
        .subscription(FerrApp::subscription)
        .theme(|_| iced::Theme::Dark)
        .window(window::Settings {
            size: Size::new(900.0, 600.0),
            min_size: Some(Size::new(800.0, 500.0)),
            ..Default::default()
        })
        .run_with(FerrApp::new)
}
