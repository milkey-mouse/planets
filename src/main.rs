use vulkano::{app_info_from_cargo_toml, instance::ApplicationInfo};
use winit::{dpi::LogicalSize, event::ScanCode};

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

const QUIT_SCANCODE: ScanCode = 16; // Q

fn main() {
    let (instance, _debug_callback) = create_instance();
    WindowThread::with(instance.clone(), move |window| {
        AudioThread::with(|mut sink| {
            let mut render = Render::new(&window);

            sink.play(None, music::vlem(sink.as_ref()));

            let events = window.events();
            events.keybinds().add(QUIT_SCANCODE);
            while !events.closed() && !events.keybinds().released(QUIT_SCANCODE) {
                window.update();
                render.update();
            }
        });
    });
}
