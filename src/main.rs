use std::process::ExitCode;

fn main() -> ExitCode {
    if let Some(err) = roll::run().err() {
        roll::print_error(&err.to_string());
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
