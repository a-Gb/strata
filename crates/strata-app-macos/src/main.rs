//! macOS production composition root for the reusable Strata workbench.
#![forbid(unsafe_code)]

fn main() {
    if let Err(error) = strata_workbench::run(strata_workbench::PRODUCTION_IDENTITY) {
        eprintln!("Strata: {error}");
        std::process::exit(1);
    }
}
