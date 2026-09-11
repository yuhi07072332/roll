use std::process::ExitCode;

use roll::log;

fn main() -> ExitCode {
    #[cfg(debug_assertions)]
    log::init_logger();

    if let Err(err) = roll::run() {
        roll::print_error(err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
