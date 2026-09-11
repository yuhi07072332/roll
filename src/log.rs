//! A minimal logger only for debug purposes

use std::env;

#[cfg(debug_assertions)]
pub static mut LOGGER_ENABLED: bool = false;

#[cfg(debug_assertions)]
pub fn init_logger() {
    use std::env::VarError;

    match env::var("ROLL_DEBUG") {
        Err(VarError::NotPresent) => (),
        _ => unsafe {
            LOGGER_ENABLED = true;
        },
    }
}

macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
        unsafe{
        if crate::log::LOGGER_ENABLED {
        eprintln!($($arg)*);
        }
        }
        }
    };
}

pub(crate) use debug;
