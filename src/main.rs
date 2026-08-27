use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(err) = roll::run() {
        roll::print_error(err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
