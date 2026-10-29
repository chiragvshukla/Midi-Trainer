use tauri::AppHandle;

use crate::commands::update_settings;
use crate::engine::scheduler::Hand;

/// Remembers which part the player takes: one hand, or a track for play-along. The piece is
/// reloaded by the frontend so the playback, gates and matcher are rebuilt for the new part.
#[tauri::command]
pub fn set_player_part(hand: Hand, track: Option<u16>, app: AppHandle) -> Result<(), String> {
    update_settings(&app, |settings| {
        settings.player_hand = hand;
        settings.player_track = track;
    })
}
