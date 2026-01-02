use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use color_eyre::{Result, eyre::eyre};
use engine::{game::Game, save_archive::SaveArchive};
use iced::{
    Length,
    widget::{Space, button, column, row, space, text},
};
use log::debug;

use crate::{
    TryIntoExt, active_game_save_path, bold_text,
    context::game_context::GameContext,
    elem_list,
    message::ui_messages::LoadMenu as MyMessage,
    saves_dir,
    state::{MainMenu, Playing, cmd},
    top_level_container,
};

#[derive(Clone, Debug)]
pub struct LoadMenu {
    saves: Vec<SaveEntry>,
}

#[derive(Clone, Debug)]
struct SaveEntry {
    filename: String,
    path: PathBuf,
    modified: SystemTime,
}

impl LoadMenu {
    pub fn try_new() -> Result<Self> {
        let dir = saves_dir()?;
        debug!("Save-files (in {dir:?}):");

        let mut saves = Vec::new();

        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if !path.is_file() {
                    continue;
                }

                let meta = entry.metadata()?;
                let modified = meta.modified()?;

                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or(eyre!("invalid file name"))?
                    .to_string();

                debug!("{filename}");
                saves.push(SaveEntry {
                    filename,
                    path,
                    modified,
                });
            }
        }

        // newest first
        saves.sort_by_key(|s| std::cmp::Reverse(s.modified));

        Ok(Self { saves })
    }
}

impl super::State for LoadMenu {
    fn update(
        &mut self,
        event: crate::message::UiMessage,
        ctx: &mut crate::context::Context,
    ) -> Result<super::StateCommand> {
        let msg: MyMessage = event.try_into_ex()?;
        use MyMessage::*;

        match msg {
            LoadSave(i) => {
                let save = &self.saves[i];
                ctx.game = None;
                let active_game_path = active_game_save_path()?;
                fs::copy(&save.path, &active_game_path)?;
                let mut archive = SaveArchive::open(&active_game_path)?;
                let gd = archive.read_game_data()?;
                let game = Game::load(ctx.config.get_llm(), ctx.config.get_image_model()?, gd);
                let game_ctx = GameContext::try_new(game, archive)?;
                ctx.game = Some(game_ctx);
                cmd::transition(Playing::new())
            }
            Back => cmd::transition(MainMenu::try_new()?),
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
                button("Back").on_press(MyMessage::Back.into()),
                space::horizontal()
            ]
        ]);

        for (i, save) in self.saves.iter().enumerate() {
            let time = format_system_time_utc(save.modified);

            tlc.push(
                row![
                    column![text(&save.filename), text(time.to_string()).size(14)],
                    space::horizontal(),
                    button("load").on_press(MyMessage::LoadSave(i).into())
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

    fn clone(&self) -> Box<dyn super::State> {
        Box::new(Clone::clone(self))
    }
}

fn format_system_time_utc(t: SystemTime) -> String {
    let secs = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return "<invalid time>".into(),
    };

    // Manual UTC conversion
    const SECS_PER_MIN: u64 = 60;
    const SECS_PER_HOUR: u64 = 60 * SECS_PER_MIN;
    const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;

    let days = secs / SECS_PER_DAY;
    let secs_of_day = secs % SECS_PER_DAY;

    let hour = secs_of_day / SECS_PER_HOUR;
    let min = (secs_of_day % SECS_PER_HOUR) / SECS_PER_MIN;
    let sec = secs_of_day % SECS_PER_MIN;

    // Gregorian calendar conversion (UTC)
    let (year, month, day) = days_to_ymd(days as i64);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, month, day, hour, min, sec
    )
}

/// Converts days since Unix epoch to (year, month, day)
fn days_to_ymd(mut days: i64) -> (i32, u32, u32) {
    let mut year = 1970;

    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days >= days_in_year {
            days -= days_in_year;
            year += 1;
        } else {
            break;
        }
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
    for &d in &month_days {
        if days >= d {
            days -= d;
            month += 1;
        } else {
            break;
        }
    }

    (year, (month + 1) as u32, (days + 1) as u32)
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
