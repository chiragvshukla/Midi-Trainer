use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::update_settings;
use crate::settings::Skill;

/// Sets how strictly the player's notes are judged. Listen only turns off waiting and judging;
/// the matcher itself keeps its place, so the pump can read the skill and skip a note.
#[tauri::command]
pub fn set_skill(
    skill: Skill,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    update_settings(&app, |settings| settings.skill = skill)?;
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_follow(skill == Skill::Follow);
    Ok(())
}
