use crossbeam_utils::atomic::AtomicCell;
use vulkano::{instance::Instance, swapchain::Surface};
use vulkano_win::VkSurfaceBuild;
use winit::{
    self,
    dpi::PhysicalSize,
    event::{
        ElementState,
        Event::{self, EventsCleared, NewEvents, UserEvent},
        KeyboardInput, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget},
    window::{Window as WinitWindow, WindowBuilder},
};

use std::{
    num::NonZeroU32,
    panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
};

mod input;

use input::Keybinds;

use crate::{get_app_info, util::IntentionalPanic, DEFAULT_WINDOW_SIZE};

pub struct WindowEvents {
    dpi_factor: AtomicCell<f64>,
    resize_to: AtomicCell<Option<(NonZeroU32, NonZeroU32)>>,
    keybinds: Keybinds,
    closed: AtomicBool,
}

impl WindowEvents {
    fn new() -> Self {
        IntentionalPanic::setup_hook();

        Self {
            dpi_factor: AtomicCell::new(1.0),
            resize_to: AtomicCell::new(None),
            keybinds: Keybinds::new(),
            closed: AtomicBool::new(false),
        }
    }

    pub fn dpi_factor(&self) -> f64 {
        self.dpi_factor.load()
    }

    pub fn resize_to(&self) -> Option<PhysicalSize> {
        self.resize_to
            .swap(None)
            .map(|s| (s.0.get(), s.1.get()).into())
    }

    pub fn keybinds(&self) -> &Keybinds {
        &self.keybinds
    }

    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn callback(&self, event: Event<()>, _wt: &EventLoopWindowTarget<()>, cf: &mut ControlFlow) {
        match event {
            UserEvent(())
            | Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::WindowEvent {
                event: WindowEvent::Destroyed,
                ..
            } => self.closed.store(true, Ordering::Release),
            Event::WindowEvent {
                event: WindowEvent::HiDpiFactorChanged(dpi_factor),
                ..
            } => self.dpi_factor.store(dpi_factor),
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                let physical: (u32, u32) = size.to_physical(self.dpi_factor.load()).into();
                self.resize_to.store(Some((
                    NonZeroU32::new(physical.0).unwrap(),
                    NonZeroU32::new(physical.1).unwrap(),
                )));
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                scancode, state, ..
                            },
                        ..
                    },
                ..
            } => match state {
                ElementState::Pressed => self.keybinds.set(scancode, true),
                ElementState::Released => self.keybinds.set(scancode, false),
            },
            EventsCleared => {}
            NewEvents(_) => {}
            //_ => {}
            e => {
                dbg!(e);
            }
        }

        if self.closed() {
            panic!(IntentionalPanic);
        }

        *cf = ControlFlow::Wait;
    }
}

pub struct WindowThread {
    events: Arc<WindowEvents>,
    event_loop: EventLoop<()>,
}

impl WindowThread {
    // some platforms such as iOS have a restriction where only the main thread can manipulate or
    // query the window, which is why this function would be needed instead of Window::spawn().
    // this function could potentially never return if panic=abort; i.e. if catch_unwind won't work
    pub fn with<F: FnOnce(Window) + Send + 'static>(instance: Arc<Instance>, f: F) {
        let (sender, receiver) = mpsc::sync_channel(1);

        thread::spawn(move || f(receiver.recv().unwrap()));

        if let Err(e) = panic::catch_unwind(move || {
            let (window, controller) = Self::new(instance);
            sender.send(controller).unwrap();

            window.run();
        }) {
            if e.downcast_ref::<IntentionalPanic>().is_none() {
                panic!(e);
            }
        }
    }

    pub fn spawn(instance: Arc<Instance>) -> Window {
        let (sender, receiver) = mpsc::sync_channel(1);

        thread::spawn(move || {
            let (window, controller) = Self::new(instance);

            sender.send(controller).unwrap();

            window.run()
        });

        receiver.recv().unwrap()
    }

    fn new(instance: Arc<Instance>) -> (Self, Window) {
        let event_loop = EventLoop::new();
        let closed = event_loop.create_proxy();

        let surface = Self::build(&event_loop, instance.clone());

        let events = Arc::new(WindowEvents::new());

        let window = Self {
            events: events.clone(),
            event_loop,
        };

        let controller = Window {
            surface,
            closed,
            events,
            instance,
        };

        (window, controller)
    }

    fn build(event_loop: &EventLoop<()>, instance: Arc<Instance>) -> Arc<Surface<WinitWindow>> {
        let mut window = WindowBuilder::new();

        if let Some(size) = DEFAULT_WINDOW_SIZE {
            window = window.with_inner_size(size);
        }

        if let Some(name) = get_app_info().application_name {
            window = window.with_title(name);
        }

        window.build_vk_surface(event_loop, instance).unwrap()
    }

    fn run(self) -> ! {
        let Self { event_loop, events } = self;

        event_loop.run(move |ev, wt, cf| events.callback(ev, wt, cf));
    }
}

pub struct Window {
    surface: Arc<Surface<WinitWindow>>,
    closed: EventLoopProxy<()>,
    events: Arc<WindowEvents>,
    instance: Arc<Instance>,
}

impl Window {
    pub fn window(&self) -> &WinitWindow {
        self.surface.window()
    }

    pub fn surface(&self) -> Arc<Surface<WinitWindow>> {
        self.surface.clone()
    }

    pub fn events(&self) -> Arc<WindowEvents> {
        self.events.clone()
    }

    pub fn instance(&self) -> Arc<Instance> {
        self.instance.clone()
    }

    pub fn dimensions(&self) -> PhysicalSize {
        if let Some((w, h)) = self.events.resize_to.load() {
            (w.get(), h.get()).into()
        } else {
            self.window()
                .inner_size()
                .to_physical(self.events.dpi_factor())
        }
    }

    pub fn update(&self) {
        self.events.keybinds.update();
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        let _ = self.closed.send_event(());
    }
}
