use cosmic::{
    Task,
    iced::{
        advanced::layout::Limits,
        core::window::Id,
        platform_specific::shell::commands::layer_surface::{
            Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
        },
        runtime::platform_specific::wayland::layer_surface::{
            IcedOutput, SctkLayerSurfaceSettings,
        },
    },
};

pub fn open<Message: 'static>(id: Id) -> Task<cosmic::Action<Message>> {
    get_layer_surface(SctkLayerSurfaceSettings {
        id,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        anchor: Anchor::all(),
        output: IcedOutput::Active,
        layer: Layer::Overlay,
        namespace: "snip-selection".into(),
        size: Some((None, None)),
        size_limits: Limits::NONE.min_width(1.0).min_height(1.0),
        exclusive_zone: -1,
        ..Default::default()
    })
}

pub fn close<Message: 'static>(id: Id) -> Task<cosmic::Action<Message>> {
    destroy_layer_surface(id)
}
