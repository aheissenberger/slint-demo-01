use std::{env, path::Path};

fn main() {
    let root = env::args().nth(1).unwrap_or_else(|| "ui".to_string());
    let ids = slint_contract::collect_agent_ids(Path::new(&root)).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    for id in ids.keys() {
        println!("{id}");
    }
}
