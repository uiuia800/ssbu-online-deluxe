#![allow(static_mut_refs)]

mod input_poll;
mod net;
mod perf_scaler;
mod render;
mod ui;
mod utils;

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().unwrap();

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let err_msg = format!("thread has panicked at '{}', {}\0", msg, location);
        skyline::error::show_error(
            69,
            "Skyline plugin has panicked! Please open the details and send a screenshot to the developer, then close the game.\0",
            err_msg.as_str()
        );
    }));
}

#[skyline::main(name = "ssbu-online-deluxe")]
pub fn main() {
    setup_panic_hook();

    render::install();
    net::install();
    ui::install();
    perf_scaler::install();
}
