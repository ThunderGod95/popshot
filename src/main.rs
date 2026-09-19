// SPDX-License-Identifier: MPL-2.0

mod app;
mod capture;
mod clipboard;
mod i18n;
mod output_selection;
mod overlay;
mod ui;

fn main() -> cosmic::iced::Result {
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    i18n::init(&requested_languages);

    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .exit_on_close(false);

    cosmic::app::run_single_instance::<app::AppModel>(settings, app::Flags)
}
