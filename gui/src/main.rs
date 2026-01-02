use color_eyre::Result;
use world_weaver::{Gui, load_config};

pub fn main() -> Result<()> {
    pretty_env_logger::init();
    let cfg = load_config()?;
    iced::application(move || Gui::new(cfg.clone()), Gui::update, Gui::view).run()?;
    Ok(())
}
