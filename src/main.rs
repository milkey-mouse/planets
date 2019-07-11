use vulkano::{app_info_from_cargo_toml, instance::ApplicationInfo};
use winit::dpi::LogicalSize;

mod assets;
mod shaders;
mod util;

mod audio;
mod render;
mod window;

use audio::{music, AudioThread};
use render::{create_instance, Render};
use window::WindowThread;

pub fn get_app_info() -> ApplicationInfo<'static> {
    ApplicationInfo {
        engine_name: Some("Newton".into()),
        ..app_info_from_cargo_toml!()
    }
}

const DEFAULT_WINDOW_SIZE: Option<LogicalSize> = Some(LogicalSize {
    width: 1280.0,
    height: 720.0,
});

fn main() {
    let (instance, _debug_callback) = create_instance();
    WindowThread::with(instance.clone(), move |window| {
        AudioThread::with(|mut sink| {
            let mut render = Render::new(&window);

            sink.play(None, music::vlem(sink.as_ref()));

            let events = window.events();

            let quit_key = events.key_state().bind(16).into_inner(); // Q
            while !(events.closed() || quit_key.released()) {
                window.update();
                render.update();
            }
        });
    });
}
