use super::*;

pub(super) fn apply_intent<R, S>(
    read_repo: &R,
    state_repo: &S,
    lang: &LanguageTag,
    state: &mut TuiState,
    intent: Intent,
    terminal_width: u16,
    terminal_height: u16,
) -> Result<bool>
where
    R: QuranReadRepository,
    S: ProgressRepository + BookmarkRepository,
{
    match intent {
        Intent::Quit => return Ok(true),
        Intent::NextAyah => {
            let canceled_pause = stop_paused_playback_for_navigation(state);
            if state.selected_ayah_idx + 1 < state.ayahs.len() {
                state.selected_ayah_idx += 1;
                if let Some(ayah) = state.current_ayah() {
                    state_repo.set_progress(ayah_ref_from_raw(ayah.surah_no, ayah.ayah_no)?)?;
                    if canceled_pause {
                        state.status = format!(
                            "Audio pause dibatalkan. Ayat aktif {}:{}",
                            ayah.surah_no, ayah.ayah_no
                        );
                    }
                }
            } else if canceled_pause {
                state.status = "Audio pause dibatalkan.".to_string();
            }
        }
        Intent::PrevAyah => {
            let canceled_pause = stop_paused_playback_for_navigation(state);
            if state.selected_ayah_idx > 0 {
                state.selected_ayah_idx -= 1;
                if let Some(ayah) = state.current_ayah() {
                    state_repo.set_progress(ayah_ref_from_raw(ayah.surah_no, ayah.ayah_no)?)?;
                    if canceled_pause {
                        state.status = format!(
                            "Audio pause dibatalkan. Ayat aktif {}:{}",
                            ayah.surah_no, ayah.ayah_no
                        );
                    }
                }
            } else if canceled_pause {
                state.status = "Audio pause dibatalkan.".to_string();
            }
        }
        Intent::NextSurah => {
            state.stop_playback();
            if state.selected_surah_idx + 1 < state.surahs.len() {
                state.selected_surah_idx += 1;
                state.surah_cursor_idx = state.selected_surah_idx;
                state.selected_ayah_idx = 0;
                state.load_surah(read_repo, lang)?;
            }
        }
        Intent::PrevSurah => {
            state.stop_playback();
            if state.selected_surah_idx > 0 {
                state.selected_surah_idx -= 1;
                state.surah_cursor_idx = state.selected_surah_idx;
                state.selected_ayah_idx = 0;
                state.load_surah(read_repo, lang)?;
            }
        }
        Intent::TogglePlayback => {
            state.start_or_toggle_playback()?;
        }
        Intent::AudioNext => {
            if let Some(player) = &mut state.player {
                if player.advance()? {
                    if let Some(ayah) = player.current_ayah() {
                        state.status = format!(
                            "PLAY {}:{} | qari {}",
                            ayah.surah_no, ayah.ayah_no, state.active_qari
                        );
                    }
                }
            }
        }
        Intent::AudioPrev => {
            if let Some(player) = &mut state.player {
                if player.prev()? {
                    if let Some(ayah) = player.current_ayah() {
                        state.status = format!(
                            "PLAY {}:{} | qari {}",
                            ayah.surah_no, ayah.ayah_no, state.active_qari
                        );
                    }
                }
            }
        }
        Intent::AudioStop => {
            state.stop_playback();
        }
        Intent::AudioRepeat => {
            if let Some(player) = &mut state.player {
                player.restart_current()?;
            }
        }
        Intent::AudioSpeedDown => {
            if let Some(player) = &mut state.player {
                let next = (player.playback_rate() - 0.05).clamp(0.75, 1.25);
                player.set_playback_rate(next);
                state.status = format!("Playback speed {:.2}x", next);
            }
        }
        Intent::AudioSpeedUp => {
            if let Some(player) = &mut state.player {
                let next = (player.playback_rate() + 0.05).clamp(0.75, 1.25);
                player.set_playback_rate(next);
                state.status = format!("Playback speed {:.2}x", next);
            }
        }
        Intent::CycleQari => {
            state.active_qari = next_qari(&state.active_qari);
            if let Some(player) = &mut state.player {
                player.set_qari(state.active_qari.clone())?;
            }
            state.status = format!(
                "Qari aktif {} ({})",
                state.active_qari,
                qari_name(&state.active_qari)
            );
        }
        Intent::FocusNextPane => {
            if state.sidebar_collapsed {
                state.focus = PaneFocus::AyahReader;
                state.status =
                    "Panel surat tersembunyi. Tekan Ctrl+B untuk membuka daftar surat.".to_string();
            } else {
                state.focus = if state.focus == PaneFocus::SurahCards {
                    PaneFocus::AyahReader
                } else {
                    PaneFocus::SurahCards
                };
                state.status = match state.focus {
                    PaneFocus::SurahCards => {
                        "Fokus panel surat. Ketik untuk filter, j/k navigasi, Enter membuka."
                            .to_string()
                    }
                    PaneFocus::AyahReader => {
                        "Fokus panel baca. Gunakan j/k untuk pindah ayat.".to_string()
                    }
                };
            }
        }
        Intent::SurahCursorUp => {
            let filtered = state.filtered_surah_indices();
            if filtered.is_empty() {
                state.status = "Filter surat tidak menemukan hasil.".to_string();
            } else if let Some(pos) = filtered
                .iter()
                .position(|&idx| idx == state.surah_cursor_idx)
                && pos > 0
            {
                state.surah_cursor_idx = filtered[pos - 1];
            }
        }
        Intent::SurahCursorDown => {
            let filtered = state.filtered_surah_indices();
            if filtered.is_empty() {
                state.status = "Filter surat tidak menemukan hasil.".to_string();
            } else if let Some(pos) = filtered
                .iter()
                .position(|&idx| idx == state.surah_cursor_idx)
                && pos + 1 < filtered.len()
            {
                state.surah_cursor_idx = filtered[pos + 1];
            }
        }
        Intent::SurahCursorSelect => {
            if state.surah_cursor_idx != state.selected_surah_idx {
                state.stop_playback();
                state.selected_surah_idx = state.surah_cursor_idx;
                state.selected_ayah_idx = 0;
                state.load_surah(read_repo, lang)?;
            }
        }
        Intent::SurahFilterType(c) => {
            if !c.is_control() {
                state.surah_filter.push(c);
                state.clamp_surah_cursor_to_filter();
                let hits = state.filtered_surah_indices().len();
                state.status = if hits == 0 {
                    format!("Filter '{}' tidak ada hasil.", state.surah_filter)
                } else {
                    format!("Filter surat: '{}' ({} hasil)", state.surah_filter, hits)
                };
            }
        }
        Intent::SurahFilterBackspace => {
            state.surah_filter.pop();
            state.clamp_surah_cursor_to_filter();
            let hits = state.filtered_surah_indices().len();
            state.status = if state.surah_filter.is_empty() {
                "Filter surat dibersihkan.".to_string()
            } else {
                format!("Filter surat: '{}' ({} hasil)", state.surah_filter, hits)
            };
        }
        Intent::SurahFilterClear => {
            state.surah_filter.clear();
            state.surah_cursor_idx = state.selected_surah_idx;
            state.status = "Filter surat dibersihkan.".to_string();
        }
        Intent::ToggleSidebar => {
            if state.sidebar_collapsed {
                if can_show_sidebar_in_frame(Rect::new(0, 0, terminal_width, terminal_height)) {
                    state.sidebar_collapsed = false;
                    state.focus = PaneFocus::SurahCards;
                    state.status =
                        "Panel surat dibuka. Ketik untuk filter, Enter untuk membuka surat."
                            .to_string();
                } else {
                    state.focus = PaneFocus::AyahReader;
                    state.status = "Ukuran terminal terlalu kecil untuk panel surat.".to_string();
                }
            } else {
                state.sidebar_collapsed = true;
                state.focus = PaneFocus::AyahReader;
                state.status =
                    "Mode baca penuh aktif. Tekan Ctrl+B untuk membuka panel surat.".to_string();
            }
        }
        Intent::ToggleHelp => {
            state.mode = UiMode::HelpOverlay;
        }
        Intent::OpenPalette => {
            state.mode = UiMode::CommandPalette;
            state.palette_input.clear();
            state.palette_selected_idx = 0;
        }
        Intent::OpenSearch => {
            state.mode = UiMode::SearchInline;
            state.search_input.clear();
            state.search_results.clear();
            state.selected_search_idx = 0;
        }
        Intent::OpenBookmarks => {
            state.bookmarks = state_repo.list_bookmarks()?;
            state.selected_bookmark_idx = 0;
            state.mode = UiMode::BookmarkOverlay;
        }
        Intent::AddBookmark => {
            if let Some(ayah) = state.current_ayah() {
                let id = state_repo.add_bookmark(
                    ayah_ref_from_raw(ayah.surah_no, ayah.ayah_no)?,
                    Some("from-modern-tui"),
                )?;
                state.status = format!(
                    "Bookmark #{} ditambahkan ({}:{})",
                    id.value(),
                    ayah.surah_no,
                    ayah.ayah_no
                );
            }
        }
        Intent::RemoveCurrentAyahBookmarks => {
            remove_current_ayah_bookmarks(state_repo, state)?;
        }
        Intent::CloseOverlay => {
            state.mode = UiMode::Reading;
        }
        Intent::PaletteMoveUp => {
            state.palette_selected_idx = state.palette_selected_idx.saturating_sub(1);
        }
        Intent::PaletteMoveDown => {
            let len = state.filtered_palette().len();
            if len > 0 {
                state.palette_selected_idx = (state.palette_selected_idx + 1).min(len - 1);
            }
        }
        Intent::PaletteBackspace => {
            state.palette_input.pop();
            state.palette_selected_idx = 0;
        }
        Intent::PaletteSubmit => {
            let items = state.filtered_palette();
            if let Some(item) = items.get(state.palette_selected_idx).copied() {
                execute_palette_action(
                    read_repo,
                    state_repo,
                    lang,
                    state,
                    item.action,
                    terminal_width,
                    terminal_height,
                )?;
            }
            state.mode = UiMode::Reading;
        }
        Intent::PaletteType(c) => {
            if !c.is_control() {
                state.palette_input.push(c);
                state.palette_selected_idx = 0;
            }
        }
        Intent::SearchMoveUp => {
            state.selected_search_idx = state.selected_search_idx.saturating_sub(1);
        }
        Intent::SearchMoveDown => {
            if !state.search_results.is_empty() {
                state.selected_search_idx =
                    (state.selected_search_idx + 1).min(state.search_results.len() - 1);
            }
        }
        Intent::SearchBackspace => {
            state.search_input.pop();
        }
        Intent::SearchSubmit => {
            if state.search_results.is_empty() {
                let query = state.search_input.trim();
                if query.is_empty() {
                    state.status = "Query kosong".to_string();
                } else {
                    state.search_results =
                        read_repo.search(query, true, true, SearchLimit::new(8)?)?;
                    state.selected_search_idx = 0;
                    state.status = format!("{} hasil ditemukan", state.search_results.len());
                }
            } else if let Some(hit) = state.search_results.get(state.selected_search_idx) {
                let surah_no = hit.surah_no;
                let ayah_no = hit.ayah_no;
                state.jump_to(surah_no, ayah_no);
                state.load_surah(read_repo, lang)?;
                state.status = format!("Lompat ke {}:{}", surah_no, ayah_no);
                state.mode = UiMode::Reading;
            }
        }
        Intent::SearchType(c) => {
            if !c.is_control() {
                state.search_input.push(c);
            }
        }
        Intent::BookmarkDelete => {
            delete_selected_bookmark(state_repo, state)?;
        }
        Intent::BookmarkMoveUp => {
            state.selected_bookmark_idx = state.selected_bookmark_idx.saturating_sub(1);
        }
        Intent::BookmarkMoveDown => {
            if !state.bookmarks.is_empty() {
                state.selected_bookmark_idx =
                    (state.selected_bookmark_idx + 1).min(state.bookmarks.len() - 1);
            }
        }
        Intent::BookmarkJump => {
            if let Some(b) = state.bookmarks.get(state.selected_bookmark_idx) {
                let id = b.id;
                let surah_no = b.surah_no;
                let ayah_no = b.ayah_no;
                state.jump_to(surah_no, ayah_no);
                state.load_surah(read_repo, lang)?;
                state.status = format!("Lompat ke bookmark #{} ({}:{})", id, surah_no, ayah_no);
            }
            state.mode = UiMode::Reading;
        }
        Intent::CycleTheme => {
            state.next_theme();
            state.status = format!("Tema diubah ke: {}", state.current_theme().name);
        }
    }

    if !can_show_sidebar_in_frame(Rect::new(0, 0, terminal_width, terminal_height)) {
        state.sidebar_collapsed = true;
        state.focus = PaneFocus::AyahReader;
    }

    Ok(false)
}

fn stop_paused_playback_for_navigation(state: &mut TuiState) -> bool {
    let paused = state
        .player
        .as_ref()
        .is_some_and(|player| player.is_paused());
    if paused {
        state.stop_playback();
    }
    paused
}

fn remove_current_ayah_bookmarks<S>(state_repo: &S, state: &mut TuiState) -> Result<()>
where
    S: ProgressRepository + BookmarkRepository,
{
    let Some(ayah) = state.current_ayah() else {
        state.status = "Tidak ada ayat aktif untuk unbookmark.".to_string();
        return Ok(());
    };
    let surah_no = ayah.surah_no;
    let ayah_no = ayah.ayah_no;

    let matches: Vec<_> = state_repo
        .list_bookmarks()?
        .into_iter()
        .filter(|bookmark| bookmark.surah_no == surah_no && bookmark.ayah_no == ayah_no)
        .collect();

    if matches.is_empty() {
        state.status = format!(
            "Belum ada bookmark pada ayat aktif {}:{}.",
            surah_no, ayah_no
        );
        return Ok(());
    }

    let removed = matches.len();
    for bookmark in matches {
        state_repo.remove_bookmark(BookmarkId::new(bookmark.id)?)?;
    }

    if state.mode == UiMode::BookmarkOverlay {
        state.bookmarks = state_repo.list_bookmarks()?;
        state.selected_bookmark_idx = state
            .selected_bookmark_idx
            .min(state.bookmarks.len().saturating_sub(1));
    }

    state.status = format!(
        "{} bookmark dihapus dari ayat {}:{}.",
        removed, surah_no, ayah_no
    );
    Ok(())
}

fn delete_selected_bookmark<S>(state_repo: &S, state: &mut TuiState) -> Result<()>
where
    S: ProgressRepository + BookmarkRepository,
{
    let Some(bookmark) = state.bookmarks.get(state.selected_bookmark_idx).cloned() else {
        state.status = "Belum ada bookmark untuk dihapus.".to_string();
        return Ok(());
    };

    state_repo.remove_bookmark(BookmarkId::new(bookmark.id)?)?;
    state.bookmarks = state_repo.list_bookmarks()?;
    state.selected_bookmark_idx = state
        .selected_bookmark_idx
        .min(state.bookmarks.len().saturating_sub(1));
    state.status = format!(
        "Bookmark #{} dihapus ({}:{}).",
        bookmark.id, bookmark.surah_no, bookmark.ayah_no
    );
    Ok(())
}

fn execute_palette_action<R, S>(
    read_repo: &R,
    state_repo: &S,
    lang: &LanguageTag,
    state: &mut TuiState,
    action: PaletteAction,
    terminal_width: u16,
    terminal_height: u16,
) -> Result<()>
where
    R: QuranReadRepository,
    S: ProgressRepository + BookmarkRepository,
{
    match action {
        PaletteAction::ToggleSidebar => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::ToggleSidebar,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::OpenBookmarks => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::OpenBookmarks,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::OpenSearch => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::OpenSearch,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::ToggleHelp => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::ToggleHelp,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::TogglePlayback => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::TogglePlayback,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::StopPlayback => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::AudioStop,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::AddBookmark => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::AddBookmark,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::NextSurah => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::NextSurah,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::PrevSurah => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::PrevSurah,
                terminal_width,
                terminal_height,
            )?;
        }
        PaletteAction::Quit => {
            state.status = "Gunakan q untuk keluar".to_string();
        }
        PaletteAction::CycleTheme => {
            apply_intent(
                read_repo,
                state_repo,
                lang,
                state,
                Intent::CycleTheme,
                terminal_width,
                terminal_height,
            )?;
        }
    }
    Ok(())
}

pub(super) fn sync_audio_tick<S>(state_repo: &S, state: &mut TuiState) -> Result<()>
where
    S: ProgressRepository + BookmarkRepository,
{
    let mut finished = false;
    if let Some(player) = &mut state.player {
        match player.tick()? {
            PlayerTick::NoChange | PlayerTick::AyahStarted(_) => {}
            PlayerTick::Finished => {
                finished = true;
            }
        }

        if let Some(ayah) = player.current_ayah() {
            state.selected_ayah_idx = ayah.ayah_no.saturating_sub(1) as usize;
            state.status = format!(
                "PLAY {}:{} | qari {}",
                ayah.surah_no, ayah.ayah_no, state.active_qari
            );
            state_repo.set_progress(ayah_ref_from_raw(ayah.surah_no, ayah.ayah_no)?)?;
        }
    }

    if finished {
        state.player = None;
        state.status = "Audio selesai".to_string();
    }

    Ok(())
}

fn next_qari(current: &str) -> String {
    match current {
        "01" => "02".to_string(),
        "02" => "03".to_string(),
        "03" => "04".to_string(),
        "04" => "05".to_string(),
        "05" => "06".to_string(),
        _ => "01".to_string(),
    }
}

pub(super) fn ayah_ref_from_raw(surah_no: u16, ayah_no: u16) -> Result<AyahRef> {
    Ok(AyahRef::new(
        SurahNumber::new(surah_no)?,
        AyahNumber::new(ayah_no)?,
    ))
}
