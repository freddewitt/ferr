use crate::app::Message;
use iced::Element;

pub mod copy_tab {
    use super::*;
    pub fn view() -> Element<'static, Message> {
        iced::widget::text("Onglet Copie").into()
    }
}

pub mod watch_tab {
    use super::*;
    pub fn view() -> Element<'static, Message> {
        iced::widget::text("Onglet Watch").into()
    }
}

pub mod verify_tab {
    use super::*;
    pub fn view() -> Element<'static, Message> {
        iced::widget::text("Onglet Vérification").into()
    }
}

pub mod history_tab {
    use super::*;
    pub fn view() -> Element<'static, Message> {
        iced::widget::text("Onglet Historique").into()
    }
}

pub mod profiles_tab {
    use super::*;
    pub fn view() -> Element<'static, Message> {
        iced::widget::text("Onglet Profils").into()
    }
}

pub mod scan_tab {
    use super::*;
    pub fn view() -> Element<'static, Message> {
        iced::widget::text("Onglet Scan (Bit Rot)").into()
    }
}

pub mod camera_tab {
    use super::*;
    pub fn view() -> Element<'static, Message> {
        iced::widget::text("Onglet Caméra & Média").into()
    }
}
