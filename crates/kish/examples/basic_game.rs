//! Basic game example - play moves and check game status.
//!
//! Run with: `cargo run --example basic_game`

use kish::{Board, GameStatus};

fn main() {
    // Create a new game with standard starting position
    let mut board = Board::new_default();
    println!("Starting position:");
    println!("{board}");
    println!();

    // Get legal moves
    let actions = board.actions();
    println!("White has {} legal moves:", actions.len());
    for action in actions.iter().take(5) {
        let detailed = action.to_detailed(board.turn, &board.state);
        println!("  {}", detailed.to_notation());
    }
    if actions.len() > 5 {
        println!("  ... and {} more", actions.len() - 5);
    }
    println!();

    // Play a few moves
    println!("Playing some moves:");
    let mut move_count = 0;
    while board.status() == GameStatus::InProgress && move_count < 10 {
        let actions = board.actions();
        if actions.is_empty() {
            break;
        }

        // Pick the first move
        let action = &actions[0];
        let detailed = action.to_detailed(board.turn, &board.state);
        println!("  {}: {}", board.turn, detailed.to_notation());

        // Apply move and swap turn
        board.apply_(action);
        board.swap_turn_();
        move_count += 1;
    }

    println!();
    println!("Position after {move_count} moves:");
    println!("{board}");

    // Check game status
    match board.status() {
        GameStatus::InProgress => println!("Game is still in progress"),
        GameStatus::Draw => println!("Game ended in a draw"),
        GameStatus::Won(team) => println!("Game won by {team}"),
    }
}
