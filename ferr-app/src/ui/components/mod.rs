// Stubs pour l'instant
pub mod toggle {
    use iced::Element;
    pub fn toggle<'a, Message: 'a>(on: bool, _on_toggle: Message) -> Element<'a, Message> {
        iced::widget::text(if on { "[ON]" } else { "[OFF]" }).into()
    }
}
pub mod drop_zone {
    use iced::Element;
    pub fn drop_zone<'a, Message: 'a>(_label: &str) -> Element<'a, Message> {
        iced::widget::text("Drop Zone").into()
    }
}
pub mod dest_card {
    use iced::Element;
    pub fn dest_card<'a, Message: 'a>(_title: &str) -> Element<'a, Message> {
        iced::widget::text("Dest Card").into()
    }
}
pub mod progress_bar {
    use iced::{Color, Element};
    pub fn progress_bar_custom<'a, Message: 'a>(
        _value: f32,
        _color: Color,
    ) -> Element<'a, Message> {
        iced::widget::text("Progress...").into()
    }
}
pub mod stat_card {
    use iced::Element;
    pub fn stat_card<'a, Message: 'a>(_label: &str, _value: &str) -> Element<'a, Message> {
        iced::widget::text("Stat Card").into()
    }
}
pub mod par2_panel {
    use iced::Element;
    pub fn par2_panel<'a, Message: 'a>() -> Element<'a, Message> {
        iced::widget::text("PAR2 Panel").into()
    }
}
