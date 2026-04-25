use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use color_eyre::Result;
use iced::{
    Length,
    widget::{Space, button, column, row, space, text, tooltip},
};
use log::debug;

use crate::{
    TryIntoExt, bold_text, elem_list, load_remembered_saves,
    message::ui_messages::LoadMenu as MyMessage,
    save_active_game_save_path, save_remembered_saves,
    state::{MainMenu, Playing, State, cmd},
    top_level_container,
};

#[derive(Clone, Debug)]
pub struct LoadMenu {
    saves: Vec<RememberedSaveEntry>,
}

#[derive(Clone, Debug)]
struct RememberedSaveEntry {
    path: PathBuf,
    modified: Option<SystemTime>,
}

impl RememberedSaveEntry {
    fn filename(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<invalid file name>")
            .to_string()
    }
}

impl LoadMenu {
    pub fn try_new() -> Result<Self> {
        let mut saves = load_remembered_saves()?
            .into_iter()
            .map(|path| RememberedSaveEntry {
                modified: fs::metadata(&path).and_then(|x| x.modified()).ok(),
                path,
            })
            .collect::<Vec<_>>();

        saves.sort_by_key(|save| std::cmp::Reverse(save.modified));

        debug!(
            "Remembered saves:\n{}",
            saves
                .iter()
                .map(|save| format!("{} -> {:?}", save.filename(), save.path))
                .collect::<Vec<_>>()
                .join("\n")
        );

        Ok(Self { saves })
    }

    fn write_remembered_saves_index(&self) -> Result<()> {
        let remembered: Vec<_> = self.saves.iter().map(|save| save.path.clone()).collect();
        save_remembered_saves(&remembered)
    }

    fn open_save_via_dialog(&mut self) -> Result<Option<PathBuf>> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("World Weaver saves", &["wwsave"])
            .pick_file()
        else {
            return Ok(None);
        };
        let modified = fs::metadata(&path).ok().and_then(|m| m.modified().ok());

        if let Some(existing) = self.saves.iter_mut().find(|save| save.path == path) {
            existing.modified = modified;
        } else {
            self.saves.push(RememberedSaveEntry {
                path: path.clone(),
                modified,
            });
            self.write_remembered_saves_index()?;
        }
        self.saves
            .sort_by_key(|save| std::cmp::Reverse(save.modified));
        Ok(Some(path))
    }
}

impl State for LoadMenu {
    fn update(
        &mut self,
        event: crate::message::UiMessage,
        ctx: &mut crate::context::Context,
    ) -> Result<super::StateCommand> {
        let msg: MyMessage = event.try_into_ex()?;
        use MyMessage::*;

        match msg {
            OpenSave => {
                let Some(path) = self.open_save_via_dialog()? else {
                    return cmd::none();
                };
                ctx.load_game_from_path(&path)?;
                save_active_game_save_path(&path)?;
                cmd::transition(Playing::new())
            }
            LoadSave(i) => {
                let save = &self.saves[i];
                ctx.load_game_from_path(&save.path)?;
                save_active_game_save_path(&save.path)?;
                cmd::transition(Playing::new())
            }
            Back => cmd::transition(MainMenu::try_new()?),
            ForgetSave(i) => {
                self.saves.remove(i);
                self.write_remembered_saves_index()?;
                cmd::none()
            }
        }
    }

    fn view<'a>(
        &'a self,
        _ctx: &'a crate::context::Context,
    ) -> iced::Element<'a, crate::message::UiMessage> {
        let mut tlc = Vec::from(elem_list![
            bold_text("Load Game").width(Length::Fill).center(),
            Space::new().height(30),
            row![
                space::horizontal(),
                button("Open...").on_press(MyMessage::OpenSave.into()),
                button("Back").on_press(MyMessage::Back.into()),
                space::horizontal()
            ]
            .spacing(10)
        ]);

        for (i, save) in self.saves.iter().enumerate() {
            let is_available = save.path.exists();
            let warning: iced::Element<'_, crate::message::UiMessage> = if is_available {
                Space::new()
                    .width(Length::Shrink)
                    .height(Length::Shrink)
                    .into()
            } else {
                tooltip(
                    text("⚠"),
                    "This save file is missing or unreadable.",
                    tooltip::Position::Top,
                )
                .into()
            };

            let time = save
                .modified
                .map(format_system_time_utc)
                .unwrap_or_else(|| "<unavailable>".to_string());

            let load_button = if is_available {
                button("Load").on_press(MyMessage::LoadSave(i).into())
            } else {
                button("Load")
            };

            tlc.push(
                row![
                    warning,
                    column![
                        text(save.filename()),
                        text(save.path.display().to_string()).size(14),
                        text(time).size(14)
                    ]
                    .spacing(4),
                    space::horizontal(),
                    button("forget").on_press(MyMessage::ForgetSave(i).into()),
                    load_button
                ]
                .spacing(10)
                .into(),
            );
        }

        top_level_container(
            column(tlc)
                .spacing(20)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .into()
    }

    fn clone(&self) -> Box<dyn State> {
        Box::new(Clone::clone(self))
    }
}

fn format_system_time_utc(t: SystemTime) -> String {
    let secs = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return "<invalid time>".into(),
    };

    const SECS_PER_MIN: u64 = 60;
    const SECS_PER_HOUR: u64 = 60 * SECS_PER_MIN;
    const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;

    let days = secs / SECS_PER_DAY;
    let secs_of_day = secs % SECS_PER_DAY;

    let hour = secs_of_day / SECS_PER_HOUR;
    let min = (secs_of_day % SECS_PER_HOUR) / SECS_PER_MIN;
    let sec = secs_of_day % SECS_PER_MIN;

    let (year, month, day) = days_to_ymd(days as i64);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, month, day, hour, min, sec
    )
}

fn days_to_ymd(mut days: i64) -> (i32, u32, u32) {
    let mut year = 1970;

    while days >= if is_leap(year) { 366 } else { 365 } {
        days -= if is_leap(year) { 366 } else { 365 };
        year += 1;
    }

    let month_days = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month = 0;
    for &month_len in &month_days {
        if days < month_len {
            break;
        }
        days -= month_len;
        month += 1;
    }

    (year, (month + 1) as u32, (days + 1) as u32)
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
