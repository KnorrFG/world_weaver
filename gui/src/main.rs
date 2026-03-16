use color_eyre::Result;
use log::LevelFilter;
use world_weaver::{Gui, load_config, state::options_menu::OptionsMenu};

pub fn main() -> Result<()> {
    let mut logger = pretty_env_logger::formatted_builder();
    logger
        .filter_level(LevelFilter::Off)
        .filter_module("world_weaver", LevelFilter::Info)
        .filter_module("engine", LevelFilter::Info)
        .parse_default_env()
        .init();
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
