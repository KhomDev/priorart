//! The `viability-gate` binary.
//!
//! Identical to `vg`. This name exists because `vg` collides with an unrelated
//! bioinformatics tool.

fn main() -> std::process::ExitCode {
    eprintln!(
        "viability-gate 0.9.0 is final and deprecated: prior-art matching moved to priorart, claim checks to realism-gate."
    );
    viability_gate::cli::main_entry_named("viability-gate")
}
