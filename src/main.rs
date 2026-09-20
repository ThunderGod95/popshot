// SPDX-License-Identifier: MPL-2.0

mod activation;
mod app;
mod cache;
mod capture;
mod clipboard;
mod i18n;
mod output_selection;
mod overlay;
mod ui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Runtime::new()?;

    let service = std::env::args().any(|arg| arg == "--gapplication-service");

    let Some(connection) = runtime.block_on(activation::start(service))? else {
        return Ok(());
    };

    runtime.spawn_blocking(cache::purge);
    runtime.block_on(activation::register_portals(&connection))?;

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    i18n::init(&requested_languages);

    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .exit_on_close(false);

    cosmic::app::run::<app::AppModel>(settings, app::Flags)?;

    Ok(())
}
