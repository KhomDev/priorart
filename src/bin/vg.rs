//! The `vg` binary - the short name.

fn main() -> std::process::ExitCode {
    eprintln!(
        "vg 0.9.0 is final and deprecated: prior-art matching moved to priorart, claim checks to realism-gate."
    );
    viability_gate::cli::main_entry()
}
