use tauri::AppHandle;

use crate::commands::update_settings;
use crate::engine::scheduler::Hand;

/// Remembers which hand the player takes in one-hand practice. The piece is reloaded by the
/// frontend so the playback and gates are rebuilt for the new hand.
#[tauri::command]
pub fn set_player_hand(hand: Hand, app: AppHandle) -> Result<(), String> {
    update_settings(&app, |settings| settings.player_hand = hand)
}
