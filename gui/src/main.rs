use color_eyre::Result;
use world_weaver::{Gui, load_config, state::options_menu::OptionsMenu};

pub fn main() -> Result<()> {
    pretty_env_logger::init();
    let cfg = load_config()?;
    let opt_menu = OptionsMenu::new(&cfg.clone().unwrap_or_default())?;
    iced::application(
        move || Gui::new(cfg.clone(), opt_menu.clone()),
        Gui::update,
        Gui::view,
    )
    .run()?;
    Ok(())
}
