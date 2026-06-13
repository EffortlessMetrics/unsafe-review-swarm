#![forbid(unsafe_code)]

fn main() {
    let mut args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "unsafe-review") {
        let _removed = args.remove(1);
    }
    match unsafe_review_cli::run(args) {
        Ok(()) => {}
        Err(unsafe_review_cli::RunFailure::PolicyViolation(msg)) => {
            eprintln!("cargo-unsafe-review: policy: {msg}");
            std::process::exit(1);
        }
        Err(unsafe_review_cli::RunFailure::Tool(msg)) => {
            eprintln!("cargo-unsafe-review: {msg}");
            std::process::exit(2);
        }
    }
}
