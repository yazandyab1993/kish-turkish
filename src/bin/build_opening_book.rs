#[path = "../opening_book.rs"]
mod opening_book;

use opening_book::{build_opening_book, DEFAULT_MAX_PLY};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let max_ply = args.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(DEFAULT_MAX_PLY);

    let training_dir = PathBuf::from("Training");
    let output = PathBuf::from("opening_book.json");
    let rejected = PathBuf::from("rejected_moves.log");

    match build_opening_book(&training_dir, max_ply, &output, &rejected) {
        Ok(stats) => {
            println!("Opening book generation completed.");
            println!("files: {}", stats.files_count);
            println!("games_read: {}", stats.games_read);
            println!("positions: {}", stats.positions);
            println!("extracted_moves: {}", stats.extracted_moves);
            println!("rejected_moves: {}", stats.rejected_moves);
            println!("output: {}", output.display());
            println!("rejected_log: {}", rejected.display());
        }
        Err(err) => {
            eprintln!("failed to build opening book: {err}");
            std::process::exit(1);
        }
    }
}
