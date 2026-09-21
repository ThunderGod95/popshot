use cosmic::{
    Element, Theme,
    iced::{Alignment, Border, Color, ContentFit, Length, Shadow, Vector, widget::Stack},
    widget::{self, image::Handle},
};

use crate::{
    app::Message,
    output_selection::{
        CaptureMode, Selection, freehand::FreehandSelection, rectangle::RectangleSelection,
    },
};

fn icon(name: &'static str) -> widget::icon::Handle {
    widget::icon::from_name(name).size(20).handle()
}

/// A floating surface follows the desktop theme, with enough separation from any wallpaper.
fn floating_surface(theme: &Theme) -> widget::container::Style {
    let mut style = cosmic::theme::Container::background(theme.cosmic(), false);

    style.border = Border {
        radius: theme.cosmic().corner_radii.radius_m.into(),
        width: 1.0,
        color: theme.cosmic().background(false).component.divider.into(),
    };

    style.shadow = Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
        offset: Vector::new(0.0, 8.0),
        blur_radius: 28.0,
    };

    style
}

pub fn selection<'a>(
    handle: &Handle,
    dragging: bool,
    status: &'a str,
    mode: CaptureMode,
) -> Element<'a, Message> {
    let screenshot = widget::image(handle.clone())
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Fill);

    let selector = match mode {
        CaptureMode::Freehand => Element::new(FreehandSelection {
            on_select: |points| Message::Select(Selection::Freehand(points)),
            on_drag: Message::Drag,
        }),
        _ => Element::new(RectangleSelection {
            on_select: |bounds| Message::Select(Selection::Rectangle(bounds)),
            on_drag: Message::Drag,
        }),
    };

    let mut layers = vec![screenshot.into(), selector];

    if !dragging {
        layers.push(
            widget::container(snipping_toolbar(status, mode))
                .center_x(Length::Fill)
                .padding(24)
                .into(),
        );
    }

    Stack::with_children(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn snipping_toolbar(status: &str, selected: CaptureMode) -> Element<'_, Message> {
    let mut modes = widget::row([]).spacing(6).align_y(Alignment::Center);

    for mode in CaptureMode::ALL {
        let symbol = match mode {
            CaptureMode::Rectangle => icon("screenshot-selection-symbolic"),
            CaptureMode::Fullscreen => icon("screenshot-screen-symbolic"),
            CaptureMode::Freehand => widget::icon::from_svg_bytes(
                include_bytes!("../resources/icons/freehand-symbolic.svg").as_slice(),
            )
            .symbolic(true),
        };

        let button = widget::button::icon(symbol)
            .label(mode.label())
            .selected(mode == selected)
            .class(if mode == selected {
                cosmic::theme::Button::Suggested
            } else {
                cosmic::theme::Button::Icon
            })
            .padding([10, 14])
            .on_press(Message::Mode(mode));

        modes = modes.push(button);
    }

    let controls = modes
        .push(widget::space().width(8))
        .push(widget::container(widget::divider::vertical::default()).height(24))
        .push(
            widget::button::icon(icon("window-close-symbolic"))
                .padding(10)
                .on_press(Message::Cancel),
        );

    let hint = if status.is_empty() {
        "Drag to capture an area"
    } else {
        status
    };

    widget::column([
        widget::container(controls)
            .padding(8)
            .style(floating_surface)
            .into(),
        widget::container(widget::text(hint).size(13))
            .padding([6, 12])
            .style(floating_surface)
            .into(),
    ])
    .spacing(12)
    .align_x(Alignment::Center)
    .into()
}

pub fn preview<'a>(
    handle: Option<&Handle>,
    dimensions: Option<(u32, u32)>,
    busy: bool,
    status: &'a str,
    detail: Option<&'a str>,
) -> Element<'a, Message> {
    widget::container(widget::column([
        preview_actions(handle.is_some(), busy),
        widget::divider::horizontal::default().into(),
        if handle.is_none() && status.is_empty() {
            widget::space()
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            preview_canvas(handle)
        },
        widget::divider::horizontal::default().into(),
        status_bar(dimensions, busy, status, detail),
    ]))
    .width(Length::Fill)
    .height(Length::Fill)
    .class(cosmic::theme::Container::WindowBackground)
    .into()
}

fn preview_actions(has_image: bool, busy: bool) -> Element<'static, Message> {
    let new_snip = widget::button::suggested("New Snip")
        .leading_icon(icon("screenshot-selection-symbolic"))
        .on_press_maybe((!busy).then_some(Message::NewSnip));

    let copy = widget::button::standard("Copy")
        .leading_icon(icon("edit-copy-symbolic"))
        .on_press_maybe((has_image && !busy).then_some(Message::Copy));

    let save = widget::button::standard("Save as…")
        .leading_icon(icon("document-save-as-symbolic"))
        .on_press_maybe((has_image && !busy).then_some(Message::Save));

    let actions = widget::row([
        new_snip.into(),
        widget::space().width(Length::Fill).into(),
        copy.into(),
        save.into(),
        settings_button(),
    ])
    .spacing(8)
    .align_y(Alignment::Center);

    widget::container(actions).padding([8, 12]).into()
}

fn preview_canvas(handle: Option<&Handle>) -> Element<'static, Message> {
    let content: Element<'static, Message> = match handle {
        Some(handle) => widget::image(handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Contain)
            .into(),

        None => widget::container(
            widget::column([
                widget::icon(icon("accessories-screenshot-symbolic"))
                    .size(48)
                    .into(),
                widget::text::title3("Screenshot unavailable").into(),
                widget::text("Choose New Snip to take a screenshot.").into(),
            ])
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .center(Length::Fill)
        .into(),
    };

    widget::container(content)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn status_bar<'a>(
    dimensions: Option<(u32, u32)>,
    busy: bool,
    status: &'a str,
    detail: Option<&'a str>,
) -> Element<'a, Message> {
    let symbol = if detail.is_some() {
        "dialog-warning-symbolic"
    } else if busy {
        "content-loading-symbolic"
    } else if status == "Copied to clipboard" || status == "Screenshot saved" {
        "object-select-symbolic"
    } else {
        "dialog-information-symbolic"
    };

    let feedback = widget::row([
        widget::icon(icon(symbol)).size(16).into(),
        widget::text(status).size(13).into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center);

    let feedback: Element<'_, Message> = if let Some(detail) = detail {
        widget::tooltip(
            feedback,
            widget::container(widget::text(detail).size(13)).max_width(440),
            widget::tooltip::Position::Top,
        )
        .into()
    } else {
        feedback.into()
    };

    let metadata = dimensions
        .map(|(width, height)| format!("{width} × {height}  ·  PNG"))
        .unwrap_or_default();

    widget::container(
        widget::row([
            widget::container(feedback).width(Length::Fill).into(),
            widget::text(metadata).size(12).into(),
        ])
        .spacing(24)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .into()
}

fn settings_button() -> Element<'static, Message> {
    widget::button::icon(icon("emblem-system-symbolic"))
        .label("Settings")
        .on_press(Message::Settings)
        .into()
}

pub fn settings<'a>(
    preview: Element<'a, Message>,
    show_preview: bool,
    copy_on_capture: bool,
    auto_save: bool,
    save_location: &'a str,
    show_notification: bool,
    error: Option<&'a str>,
) -> Element<'a, Message> {
    let capture = widget::settings::section()
        .title("Capture behavior")
        .add(
            widget::settings::item::builder("Copy screenshots to clipboard")
                .toggler(copy_on_capture, Message::CopyOnCapture),
        )
        .add(
            widget::settings::item::builder("Automatically save screenshots")
                .toggler(auto_save, Message::AutoSave),
        )
        .add(
            widget::settings::item::builder("Save location")
                .description(save_location)
                .control(widget::button::standard("Choose…").on_press(Message::ChooseSaveLocation)),
        )
        .add(
            widget::settings::item::builder("Show notification after capture")
                .toggler(show_notification, Message::ShowNotification),
        )
        .add(
            widget::settings::item::builder("Open editor after capture")
                .toggler(show_preview, Message::ShowPreview),
        );

    let mut content = widget::column([capture.into()]).spacing(24);

    if let Some(error) = error {
        content = content.push(widget::text(error));
    }

    widget::context_drawer(
        Some("Settings".into()),
        None,
        None,
        None,
        Message::Back,
        preview,
        content,
        400.0,
    )
    .into()
}
