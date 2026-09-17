use std::{env, path::Path};

fn main() {
    let root = env::args().nth(1).unwrap_or_else(|| "ui".to_string());
    let contract =
        slint_contract::collect_agent_contract(Path::new(&root)).unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(1);
        });
    serde_json::to_writer_pretty(std::io::stdout(), &contract).unwrap_or_else(|error| {
        eprintln!("failed to serialize Slint contract: {error}");
        std::process::exit(1);
    });
    println!();
}
