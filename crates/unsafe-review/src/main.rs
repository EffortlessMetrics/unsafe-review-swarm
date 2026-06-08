#![forbid(unsafe_code)]

fn main() {
    match unsafe_review_cli::run(std::env::args()) {
        Ok(()) => {}
        Err(unsafe_review_cli::RunFailure::PolicyViolation(msg)) => {
            eprintln!("unsafe-review: policy: {msg}");
            std::process::exit(1);
        }
        Err(unsafe_review_cli::RunFailure::Tool(msg)) => {
            eprintln!("unsafe-review: {msg}");
            std::process::exit(2);
        }
    }
}
