//! Custom position example - set up specific board states.
//!
//! Run with: `cargo run --example custom_position`

use kish::{Board, Square, Team};

fn main() {
    // Create a custom position with specific pieces
    // White king on D4, white pawn on E3
    // Black pawns on D5 and F6
    let board = Board::from_squares(
        Team::White,
        &[Square::D4, Square::E3],
        &[Square::D5, Square::F6],
        &[Square::D4], // D4 is a king
    );

    println!("Custom position:");
    println!("{board}");
    println!();

    // Query pieces using bitboards
    let white_count = board.state.pieces[0].count_ones();
    let black_count = board.state.pieces[1].count_ones();
    let king_count = board.state.kings.count_ones();

    println!("White pieces: {white_count}");
    println!("Black pieces: {black_count}");
    println!("Kings: {king_count}");
    println!();

    // Get legal moves
    let actions = board.actions();
    println!("Legal moves for {}:", board.turn);
    for action in &actions {
        let detailed = action.to_detailed(board.turn, &board.state);
        let mut details = Vec::new();

        if detailed.is_capture() {
            details.push(format!("captures {}", detailed.path_len() - 1));
        }
        if detailed.is_promotion() {
            details.push("promotes".to_string());
        }

        let suffix = if details.is_empty() {
            String::new()
        } else {
            format!(" ({})", details.join(", "))
        };

        println!("  {}{}", detailed.to_notation(), suffix);
    }
}
