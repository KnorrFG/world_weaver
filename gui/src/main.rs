
use color_eyre::Result;

use world_weaver::Gui;

pub fn main() -> Result<()> {
    pretty_env_logger::init();

    iced::application(Gui::new, Gui::update, Gui::view)
        // .theme(Gui::theme)
        .run()?;
    Ok(())
}
