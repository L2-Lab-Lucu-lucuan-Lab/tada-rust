use super::*;

pub(super) fn map_key_to_intent(state: &TuiState, key: KeyEvent) -> Option<Intent> {
    match state.mode {
        UiMode::Reading => map_reading_key(state, key),
        UiMode::CommandPalette => map_palette_key(key),
        UiMode::SearchInline => map_search_key(key),
        UiMode::BookmarkOverlay => map_bookmark_key(key),
        UiMode::HelpOverlay => map_help_key(key),
    }
}

fn map_reading_key(state: &TuiState, key: KeyEvent) -> Option<Intent> {
    let sidebar_filter_mode = state.focus == PaneFocus::SurahCards && !state.sidebar_collapsed;

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('k') {
        return Some(Intent::OpenPalette);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
        return Some(Intent::ToggleSidebar);
    }

    match key.code {
        KeyCode::Char('q') => Some(Intent::Quit),
        KeyCode::Char('j') | KeyCode::Down => {
            if state.focus == PaneFocus::SurahCards && !state.sidebar_collapsed {
                Some(Intent::SurahCursorDown)
            } else {
                Some(Intent::NextAyah)
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if state.focus == PaneFocus::SurahCards && !state.sidebar_collapsed {
                Some(Intent::SurahCursorUp)
            } else {
                Some(Intent::PrevAyah)
            }
        }
        KeyCode::Enter => {
            if state.focus == PaneFocus::SurahCards && !state.sidebar_collapsed {
                Some(Intent::SurahCursorSelect)
            } else {
                None
            }
        }
        KeyCode::Backspace => {
            if sidebar_filter_mode {
                Some(Intent::SurahFilterBackspace)
            } else {
                None
            }
        }
        KeyCode::Esc => {
            if sidebar_filter_mode && !state.surah_filter.is_empty() {
                Some(Intent::SurahFilterClear)
            } else {
                None
            }
        }
        KeyCode::Char('n') => Some(Intent::NextSurah),
        KeyCode::Char('p') => Some(Intent::PrevSurah),
        KeyCode::Char('f') => Some(Intent::AddBookmark),
        KeyCode::Char('/') => Some(Intent::OpenSearch),
        KeyCode::Char(' ') => Some(Intent::TogglePlayback),
        KeyCode::Char(']') => Some(Intent::AudioNext),
        KeyCode::Char('[') => Some(Intent::AudioPrev),
        KeyCode::Char('s') => Some(Intent::AudioStop),
        KeyCode::Char('r') => Some(Intent::AudioRepeat),
        KeyCode::Char(',') => Some(Intent::AudioSpeedDown),
        KeyCode::Char('.') => Some(Intent::AudioSpeedUp),
        KeyCode::Char('Q') => Some(Intent::CycleQari),
        KeyCode::Tab => Some(Intent::FocusNextPane),
        KeyCode::F(1) => Some(Intent::ToggleHelp),
        KeyCode::Char(c) => {
            if sidebar_filter_mode
                && !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
                && !c.is_control()
            {
                Some(Intent::SurahFilterType(c))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn map_palette_key(key: KeyEvent) -> Option<Intent> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('k') {
        return Some(Intent::CloseOverlay);
    }

    match key.code {
        KeyCode::Esc => Some(Intent::CloseOverlay),
        KeyCode::Up => Some(Intent::PaletteMoveUp),
        KeyCode::Down => Some(Intent::PaletteMoveDown),
        KeyCode::Backspace => Some(Intent::PaletteBackspace),
        KeyCode::Enter => Some(Intent::PaletteSubmit),
        KeyCode::Char(c) => Some(Intent::PaletteType(c)),
        _ => None,
    }
}

fn map_search_key(key: KeyEvent) -> Option<Intent> {
    match key.code {
        KeyCode::Esc => Some(Intent::CloseOverlay),
        KeyCode::Up => Some(Intent::SearchMoveUp),
        KeyCode::Down => Some(Intent::SearchMoveDown),
        KeyCode::Backspace => Some(Intent::SearchBackspace),
        KeyCode::Enter => Some(Intent::SearchSubmit),
        KeyCode::Char(c) => Some(Intent::SearchType(c)),
        _ => None,
    }
}

fn map_bookmark_key(key: KeyEvent) -> Option<Intent> {
    match key.code {
        KeyCode::Esc => Some(Intent::CloseOverlay),
        KeyCode::Up => Some(Intent::BookmarkMoveUp),
        KeyCode::Down => Some(Intent::BookmarkMoveDown),
        KeyCode::Enter => Some(Intent::BookmarkJump),
        _ => None,
    }
}

fn map_help_key(key: KeyEvent) -> Option<Intent> {
    match key.code {
        KeyCode::Esc | KeyCode::F(1) => Some(Intent::CloseOverlay),
        _ => None,
    }
}
