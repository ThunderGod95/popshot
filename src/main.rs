// SPDX-License-Identifier: MPL-2.0

mod activation;
mod app;
mod cache;
mod capture;
mod cli;
mod clipboard;
mod i18n;
mod output_selection;
mod overlay;
mod ui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{}", cli::HELP);
            return Ok(());
        }
        Err(error) => {
            eprintln!("popshot: {error}\nTry 'popshot --help'.");
            std::process::exit(2);
        }
    };

    let runtime = tokio::runtime::Runtime::new()?;

    let Some(connection) = runtime.block_on(activation::start(options.service, options.mode))?
    else {
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
