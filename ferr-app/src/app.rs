use crate::state::AppState;
use iced::{Element, Task};

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(crate::state::Tab),
    // ... we will add more messages later
}

pub struct FerrApp {
    state: AppState,
}

impl FerrApp {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                state: AppState::new(),
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::TabSelected(tab) => {
                self.state.current_tab = tab;
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        use crate::state::Tab;
        use crate::theme::*;
        use iced::widget::{button, column, container, row, text};

        let sidebar = container(
            column![
                text("Volumes").size(11).color(TEXT_MUTED),
                text("Destinations").size(11).color(TEXT_MUTED),
                text("Récent").size(11).color(TEXT_MUTED),
            ]
            .spacing(20),
        )
        .width(iced::Length::Fixed(172.0))
        .height(iced::Length::Fill)
        .padding(16)
        .style(|_t| container::Style::default().background(SURFACE_ALT));

        let make_tab =
            |label: &'static str, tab_id: Tab, active: bool| -> Element<'static, Message> {
                let color_val = if active { TEXT_PRIMARY } else { TEXT_DIM };
                button(text(label).size(13).color(color_val))
                    .on_press(Message::TabSelected(tab_id))
                    .style(|_t, _s| button::Style::default())
                    .padding([9, 12])
                    .into()
            };

        let nav = row![
            make_tab("Copie", Tab::Copy, self.state.current_tab == Tab::Copy),
            make_tab("Watch", Tab::Watch, self.state.current_tab == Tab::Watch),
            make_tab(
                "Vérification",
                Tab::Verify,
                self.state.current_tab == Tab::Verify
            ),
            make_tab(
                "Historique",
                Tab::History,
                self.state.current_tab == Tab::History
            ),
            make_tab(
                "Profils",
                Tab::Profiles,
                self.state.current_tab == Tab::Profiles
            ),
            make_tab("Scan", Tab::Scan, self.state.current_tab == Tab::Scan),
            make_tab(
                "Caméra & média",
                Tab::Camera,
                self.state.current_tab == Tab::Camera
            ),
        ]
        .spacing(0);

        let active_view: Element<'_, Message> = match self.state.current_tab {
            Tab::Copy => crate::ui::tabs::copy_tab::view(),
            Tab::Watch => crate::ui::tabs::watch_tab::view(),
            Tab::Verify => crate::ui::tabs::verify_tab::view(),
            Tab::History => crate::ui::tabs::history_tab::view(),
            Tab::Profiles => crate::ui::tabs::profiles_tab::view(),
            Tab::Scan => crate::ui::tabs::scan_tab::view(),
            Tab::Camera => crate::ui::tabs::camera_tab::view(),
        };

        let content = column![
            container(nav)
                .width(iced::Length::Fill)
                .padding([0, 14])
                .style(|_t| { container::Style::default().background(SURFACE_ALT) }),
            container(active_view)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .padding(20)
        ];

        let main_row = row![sidebar, content];

        container(main_row)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .style(|_t| container::Style::default().background(APP_BG))
            .into()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
}
