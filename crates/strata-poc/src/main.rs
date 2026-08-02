//! Compatibility executable for the reusable Strata workbench.
#![forbid(unsafe_code)]

fn main() {
    if let Err(error) = strata_workbench::run(strata_workbench::POC_IDENTITY) {
        eprintln!("Strata POC: {error}");
        std::process::exit(1);
    }
}
