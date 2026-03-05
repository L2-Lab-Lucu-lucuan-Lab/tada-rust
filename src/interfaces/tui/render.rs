use super::*;
use ratatui::layout::Alignment;
use ratatui::widgets::Padding;

#[derive(Debug, Clone, Copy)]
struct UiScale {
    outer_margin_x: u16,
    outer_margin_y: u16,
    split_gap: u16,
    footer_height: u16,
}

impl UiScale {
    fn from_area(area: Rect) -> Self {
        let outer_margin_x = 0;
        let outer_margin_y = 0;
        let split_gap = if area.width >= 120 { 1 } else { 0 };
        let footer_height = if area.height >= 30 { 3 } else { 2 };

        Self {
            outer_margin_x,
            outer_margin_y,
            split_gap,
            footer_height,
        }
    }
}

pub(super) fn draw_ui(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    frame.render_widget(Block::default().style(state.theme.app_bg), frame.area());

    let scale = UiScale::from_area(frame.area());
    let content = inset_rect(frame.area(), scale.outer_margin_x, scale.outer_margin_y);
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(scale.footer_height)])
        .split(content);

    draw_body(frame, root[0], state, scale);
    draw_footer(frame, root[1], state);

    match state.mode {
        UiMode::CommandPalette => draw_palette_overlay(frame, state),
        UiMode::SearchInline => draw_search_overlay(frame, state),
        UiMode::BookmarkOverlay => draw_bookmark_overlay(frame, state),
        UiMode::HelpOverlay => draw_help_overlay(frame, state),
        UiMode::Reading => {}
    }
}

fn draw_body(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState, scale: UiScale) {
    if state.sidebar_collapsed {
        draw_reader_workspace(frame, area, state);
        return;
    }

    let side_width = sidebar_width(area.width);
    let chunks = if scale.split_gap > 0 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(side_width),
                Constraint::Length(scale.split_gap),
                Constraint::Min(1),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(side_width), Constraint::Min(1)])
            .split(area)
    };

    draw_surah_sidebar(frame, chunks[0], state);

    let reader_idx = if scale.split_gap > 0 { 2 } else { 1 };
    draw_reader_workspace(frame, chunks[reader_idx], state);
}

fn sidebar_width(area_width: u16) -> u16 {
    let preferred = area_width.saturating_mul(30) / 100;
    let min_sidebar = 26;
    let max_sidebar = area_width.saturating_sub(46);
    let capped_max = max_sidebar.max(min_sidebar).min(area_width.saturating_sub(1));
    preferred.max(min_sidebar).min(capped_max)
}

fn draw_surah_sidebar(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    let title = if state.focus == PaneFocus::SurahCards {
        "Daftar Surat [Fokus]"
    } else {
        "Daftar Surat"
    };

    let outer = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(state.theme.panel)
        .border_style(if state.focus == PaneFocus::SurahCards {
            state.theme.accent
        } else {
            state.theme.frame
        })
        .padding(Padding::new(2, 2, 0, 0));
    frame.render_widget(outer.clone(), area);

    let inner = outer.inner(area);
    if inner.height < 7 {
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let search_text = if state.surah_filter.is_empty() {
        Line::from(Span::styled("Cari surat...", state.theme.muted))
    } else {
        Line::from(vec![
            Span::styled("Filter: ", state.theme.muted),
            Span::styled(&state.surah_filter, state.theme.accent),
        ])
    };
    let search = Paragraph::new(search_text).style(state.theme.frame).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1))
            .border_style(if state.focus == PaneFocus::SurahCards {
                state.theme.accent
            } else {
                state.theme.frame
            }),
    );
    frame.render_widget(search, sections[0]);

    let filtered = state.filtered_surah_indices();
    if filtered.is_empty() {
        frame.render_widget(
            Paragraph::new("Tidak ada hasil filter.")
                .style(state.theme.muted)
                .block(Block::default().borders(Borders::TOP)),
            sections[2],
        );
        return;
    }

    let card_height: u16 = 5;
    let visible_cards = ((sections[2].height as usize) / card_height as usize).max(1);
    let cursor_pos = filtered
        .iter()
        .position(|&idx| idx == state.surah_cursor_idx)
        .unwrap_or(0);
    let window = compute_visible_window(cursor_pos, filtered.len(), visible_cards);

    let mut constraints: Vec<Constraint> = (0..visible_cards)
        .map(|_| Constraint::Length(card_height))
        .collect();
    constraints.push(Constraint::Min(0));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(sections[2]);

    for slot in 0..visible_cards {
        let filtered_idx = window.start + slot;
        if filtered_idx >= window.end {
            break;
        }

        let surah_idx = filtered[filtered_idx];
        let surah = &state.surahs[surah_idx];
        let is_selected = surah_idx == state.selected_surah_idx;
        let is_cursor = surah_idx == state.surah_cursor_idx;
        let is_focus = state.focus == PaneFocus::SurahCards && is_cursor;

        let card_style = if is_focus {
            state.theme.card_focus
        } else if is_selected {
            state.theme.card_active
        } else {
            state.theme.card
        };

        let lines = vec![
            Line::from(vec![
                Span::styled(format!("{:>3}. ", surah.surah_no), state.theme.muted),
                Span::styled(&surah.name_id, state.theme.strong),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ayat", surah.ayah_count), state.theme.muted),
                Span::raw("  |  "),
                Span::styled(&surah.name_ar, state.theme.accent),
            ]),
            if is_selected {
                Line::from(Span::styled("TERBUKA", state.theme.chip))
            } else {
                Line::from(Span::styled("Enter untuk buka", state.theme.muted))
            },
        ];

        frame.render_widget(
            Paragraph::new(lines)
                .style(card_style)
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .padding(Padding::new(2, 2, 1, 0))
                        .style(card_style)
                        .border_style(if is_focus {
                            state.theme.accent
                        } else {
                            state.theme.frame
                        }),
                ),
            rows[slot],
        );
    }
}

fn draw_reader_workspace(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    let workspace = inset_rect(area, 0, 0);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(4),
            Constraint::Min(1),
        ])
        .split(workspace);

    draw_surah_summary(frame, sections[0], state);
    draw_control_bar(frame, sections[1], state);
    draw_ayah_list(frame, sections[2], state);
}

fn draw_surah_summary(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    let surah = state.current_surah();
    let progress = state
        .current_ayah()
        .map(|a| format!("{}:{}", a.surah_no, a.ayah_no))
        .unwrap_or_else(|| "-".to_string());

    let (audio_state, speed) = if let Some(player) = &state.player {
        let status = if player.is_paused() { "PAUSE" } else { "PLAY" };
        (status, format!("{:.2}x", player.playback_rate()))
    } else {
        ("STOP", "1.00x".to_string())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(state.theme.panel)
        .border_style(state.theme.frame)
        .padding(Padding::new(2, 2, 0, 0));
    frame.render_widget(block.clone(), area);

    let inner = block.inner(area);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(16)])
        .split(inner);

    let left = vec![
        Line::from(vec![
            Span::styled(
                format!("{}. {}", surah.surah_no, surah.name_id),
                state.theme.strong,
            ),
            Span::styled("  |  Surat Aktif", state.theme.muted),
        ]),
        Line::from(vec![
            Span::styled(format!("{} ayat", surah.ayah_count), state.theme.muted),
            Span::raw("  |  "),
            Span::styled(format!("posisi {}", progress), state.theme.muted),
            Span::raw("  |  "),
            Span::styled(format!("audio {} @ {}", audio_state, speed), state.theme.accent),
        ]),
        Line::from(vec![Span::styled(
            format!("Qari: {} ({})", state.active_qari, qari_name(&state.active_qari)),
            state.theme.frame,
        )]),
    ];
    frame.render_widget(
        Paragraph::new(left)
            .style(state.theme.frame)
            .wrap(Wrap { trim: true }),
        cols[0],
    );

    frame.render_widget(
        Paragraph::new(surah.name_ar.as_str())
            .style(state.theme.accent)
            .alignment(Alignment::Right),
        cols[1],
    );
}

fn draw_control_bar(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    let translit_on = state
        .current_ayah()
        .and_then(|a| a.transliteration.as_ref())
        .is_some();

    let lines = vec![
        Line::from(vec![
            Span::styled("Ayat: ", state.theme.muted),
            Span::styled("[Semua]", state.theme.card_focus),
            Span::raw("  "),
            Span::styled("Qari: ", state.theme.muted),
            Span::styled(
                format!("[{}]", qari_name(&state.active_qari)),
                state.theme.card_focus,
            ),
        ]),
        Line::from(vec![
            Span::styled("Transliterasi ", state.theme.muted),
            Span::styled(if translit_on { "[ON]" } else { "[OFF]" }, state.theme.chip),
            Span::raw("  "),
            Span::styled("Terjemahan ", state.theme.muted),
            Span::styled(
                if state.show_translation { "[ON]" } else { "[OFF]" },
                state.theme.chip,
            ),
            Span::raw("  "),
            Span::styled("[Play Audio Full]", state.theme.accent),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines).style(state.theme.frame).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .padding(Padding::new(2, 2, 0, 0))
                .style(state.theme.panel)
                .border_style(state.theme.frame),
        ),
        area,
    );
}

fn draw_ayah_list(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    let container = Block::default()
        .title("Daftar Ayat")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(state.theme.panel)
        .border_style(state.theme.frame)
        .padding(Padding::new(2, 2, 0, 0));
    frame.render_widget(container.clone(), area);

    let list_area = container.inner(area);
    if list_area.width < 20 || list_area.height < 4 {
        return;
    }

    if state.ayahs.is_empty() {
        frame.render_widget(
            Paragraph::new("Belum ada ayat untuk surah ini.")
                .style(state.theme.muted)
                .alignment(Alignment::Center),
            list_area,
        );
        return;
    }

    let card_height = ayah_card_height(state, list_area);
    let visible_cards = ((list_area.height as usize) / card_height as usize).max(1);
    let window = compute_visible_window(state.selected_ayah_idx, state.ayahs.len(), visible_cards);

    let mut constraints: Vec<Constraint> = (0..visible_cards)
        .map(|_| Constraint::Length(card_height))
        .collect();
    constraints.push(Constraint::Min(0));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(list_area);

    for slot in 0..visible_cards {
        let ayah_idx = window.start + slot;
        if ayah_idx >= window.end {
            break;
        }
        draw_ayah_card(frame, rows[slot], &state.ayahs[ayah_idx], ayah_idx == state.selected_ayah_idx, state);
    }
}

fn ayah_card_height(state: &TuiState, area: Rect) -> u16 {
    let mut height = 6;
    if state.show_translation {
        height += 1;
    }
    if area.width < 92 {
        height += 1;
    }
    height
}

fn draw_ayah_card(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    ayah: &Ayah,
    is_selected: bool,
    state: &TuiState,
) {
    let card = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(if is_selected {
            state.theme.card_focus
        } else {
            state.theme.card
        })
        .border_style(if is_selected {
            state.theme.accent
        } else {
            state.theme.frame
        })
        .padding(Padding::new(2, 2, 0, 0));
    frame.render_widget(card.clone(), area);

    let inner = card.inner(area);
    if inner.height < 3 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let tools = Line::from(vec![
        Span::styled(
            format!("[{}]", ayah.ayah_no),
            if is_selected { state.theme.chip } else { state.theme.muted },
        ),
        Span::raw("  "),
        Span::styled("[play] [next]", state.theme.muted),
    ]);
    frame.render_widget(Paragraph::new(tools).style(state.theme.frame), rows[0]);

    if rows[1].width >= 90 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
            .split(rows[1]);

        let mut left_lines = Vec::new();
        if let Some(translit) = ayah.transliteration.as_deref() {
            left_lines.push(Line::from(Span::styled(translit, state.theme.muted)));
        }
        if state.show_translation {
            left_lines.push(Line::from(""));
            left_lines.push(Line::from(Span::styled(
                ayah.translation.as_deref().unwrap_or("-"),
                state.theme.frame,
            )));
        }
        frame.render_widget(
            Paragraph::new(left_lines)
                .style(state.theme.frame)
                .wrap(Wrap { trim: true }),
            cols[0],
        );

        frame.render_widget(
            Paragraph::new(ayah.arabic_text.as_str())
                .style(state.theme.strong)
                .alignment(Alignment::Right)
                .wrap(Wrap { trim: true }),
            cols[1],
        );
    } else {
        let mut lines = vec![Line::from(Span::styled(ayah.arabic_text.as_str(), state.theme.strong))];
        if let Some(translit) = ayah.transliteration.as_deref() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(translit, state.theme.muted)));
        }
        if state.show_translation {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                ayah.translation.as_deref().unwrap_or("-"),
                state.theme.frame,
            )));
        }

        frame.render_widget(
            Paragraph::new(lines)
                .style(state.theme.frame)
                .wrap(Wrap { trim: true }),
            rows[1],
        );
    }
}

pub(super) fn filter_surah_indices(surahs: &[SurahMeta], query: &str) -> Vec<usize> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return (0..surahs.len()).collect();
    }

    surahs
        .iter()
        .enumerate()
        .filter_map(|(idx, surah)| {
            let surah_no = surah.surah_no.to_string();
            let name_latin = surah.name_id.to_lowercase();
            let name_ar = surah.name_ar.to_lowercase();
            if surah_no.contains(&needle)
                || name_latin.contains(&needle)
                || name_ar.contains(&needle)
            {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

struct VisibleWindow {
    start: usize,
    end: usize,
}

fn compute_visible_window(selected: usize, total: usize, visible: usize) -> VisibleWindow {
    if total <= visible {
        return VisibleWindow {
            start: 0,
            end: total,
        };
    }

    let half = visible / 2;
    let mut start = selected.saturating_sub(half);
    let end = (start + visible).min(total);

    if end - start < visible {
        start = end.saturating_sub(visible);
    }

    VisibleWindow { start, end }
}

fn draw_footer(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    let focus_label = if state.focus == PaneFocus::SurahCards {
        "SIDEBAR"
    } else {
        "READER"
    };

    let line = Line::from(vec![
        Span::styled("Fokus ", state.theme.muted),
        Span::styled(format!(" {} ", focus_label), state.theme.chip),
        Span::raw(" | "),
        Span::styled(&state.status, state.theme.frame),
        Span::raw(" | "),
        Span::styled("Ctrl+K actions | / search | Tab pindah fokus | q keluar", state.theme.muted),
    ]);

    let footer = Paragraph::new(line).style(state.theme.frame).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::new(2, 2, 0, 0))
            .style(state.theme.panel)
            .border_style(state.theme.frame),
    );
    frame.render_widget(footer, area);
}

fn draw_palette_overlay(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let area = centered_rect(frame.area(), 70, 55);
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    let input = Paragraph::new(format!("> {}", state.palette_input))
        .style(state.theme.strong)
        .block(
            Block::default()
                .title("Command Palette (Ctrl+K)")
                .border_type(BorderType::Rounded)
                .borders(Borders::ALL)
                .style(state.theme.panel),
        );
    frame.render_widget(input, chunks[0]);

    let items = state.filtered_palette();
    let list_items: Vec<ListItem<'_>> = items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            if idx == state.palette_selected_idx {
                ListItem::new(item.label.to_string()).style(state.theme.accent)
            } else {
                ListItem::new(item.label)
            }
        })
        .collect();

    let list = List::new(list_items).block(
        Block::default()
            .border_type(BorderType::Rounded)
            .borders(Borders::ALL)
            .style(state.theme.panel),
    );
    frame.render_widget(list, chunks[1]);
}

fn draw_search_overlay(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let area = centered_rect(frame.area(), 72, 60);
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    let input = Paragraph::new(format!("/ {}", state.search_input))
        .style(state.theme.strong)
        .block(
            Block::default()
                .title("Inline Search (Enter untuk cari/lompat)")
                .border_type(BorderType::Rounded)
                .borders(Borders::ALL)
                .style(state.theme.panel),
        );
    frame.render_widget(input, chunks[0]);

    let items: Vec<ListItem<'_>> = if state.search_results.is_empty() {
        vec![ListItem::new("Belum ada hasil. Tekan Enter untuk cari.")]
    } else {
        state
            .search_results
            .iter()
            .enumerate()
            .map(|(idx, hit)| {
                let line = format!("{}:{} {}", hit.surah_no, hit.ayah_no, hit.snippet);
                if idx == state.selected_search_idx {
                    ListItem::new(line).style(state.theme.accent)
                } else {
                    ListItem::new(line)
                }
            })
            .collect()
    };

    let results = List::new(items).block(
        Block::default()
            .border_type(BorderType::Rounded)
            .borders(Borders::ALL)
            .style(state.theme.panel),
    );
    frame.render_widget(results, chunks[1]);
}

fn draw_bookmark_overlay(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let area = centered_rect(frame.area(), 64, 60);
    frame.render_widget(Clear, area);

    let items: Vec<ListItem<'_>> = if state.bookmarks.is_empty() {
        vec![ListItem::new("Belum ada bookmark")]
    } else {
        state
            .bookmarks
            .iter()
            .enumerate()
            .map(|(idx, b)| {
                let line = format!(
                    "#{}  {}:{}  {}",
                    b.id,
                    b.surah_no,
                    b.ayah_no,
                    b.note.as_deref().unwrap_or("-")
                );
                if idx == state.selected_bookmark_idx {
                    ListItem::new(line).style(state.theme.accent)
                } else {
                    ListItem::new(line)
                }
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .title("Bookmarks (Enter untuk lompat)")
            .border_type(BorderType::Rounded)
            .borders(Borders::ALL)
            .style(state.theme.panel),
    );
    frame.render_widget(list, area);
}

fn draw_help_overlay(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let area = centered_rect(frame.area(), 68, 56);
    frame.render_widget(Clear, area);

    let text = vec![
        "Modern Keymap",
        "",
        "Ctrl+K   : Command palette",
        "Tab      : Ganti fokus Sidebar <-> Reader",
        "j / k    : Navigasi fokus aktif",
        "Ketik    : Live filter surat (saat fokus Sidebar)",
        "Backspace: Hapus karakter filter surat",
        "Esc      : Clear filter surat (saat fokus Sidebar)",
        "Enter    : Buka surat dari card terpilih",
        "n / p    : Surah berikutnya / sebelumnya",
        "/        : Inline search",
        "f        : Tambah bookmark",
        "Ctrl+B   : Tampil/sembunyi sidebar",
        "Space    : Play/Pause audio",
        "] / [    : Next / Prev ayat audio",
        "s        : Stop playback",
        "r        : Repeat ayat aktif",
        "Q        : Ganti qari (cycle)",
        ", / .    : Turun/naik speed",
        "F1       : Buka/tutup help",
        "q        : Keluar",
        "",
        "Esc untuk kembali.",
    ]
    .join("\n");

    let block = Paragraph::new(text)
        .style(state.theme.frame)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title("Help")
                .border_type(BorderType::Rounded)
                .borders(Borders::ALL)
                .style(state.theme.panel),
        );
    frame.render_widget(block, area);
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

fn inset_rect(area: Rect, margin_x: u16, margin_y: u16) -> Rect {
    let clamped_x = margin_x.min(area.width.saturating_sub(1) / 2);
    let clamped_y = margin_y.min(area.height.saturating_sub(1) / 2);
    Rect::new(
        area.x.saturating_add(clamped_x),
        area.y.saturating_add(clamped_y),
        area.width.saturating_sub(clamped_x.saturating_mul(2)).max(1),
        area.height.saturating_sub(clamped_y.saturating_mul(2)).max(1),
    )
}

pub(super) fn frame_size_hint(terminal: &Terminal<CrosstermBackend<Stdout>>) -> (u16, u16) {
    terminal
        .size()
        .map(|s| (s.width, s.height))
        .unwrap_or((120, 36))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{VisibleWindow, compute_visible_window, filter_surah_indices};
    use crate::domain::SurahMeta;

    fn mk_surah(no: u16, name_id: &str) -> SurahMeta {
        SurahMeta {
            surah_no: no,
            name_ar: name_id.to_string(),
            name_id: name_id.to_string(),
            ayah_count: 7,
            audio_full: None,
            audio_full_urls: BTreeMap::new(),
        }
    }

    #[test]
    fn filter_surah_indices_supports_number_and_name() {
        let surahs = vec![
            mk_surah(1, "Al-Fatihah"),
            mk_surah(2, "Al-Baqarah"),
            mk_surah(3, "Ali Imran"),
        ];

        assert_eq!(filter_surah_indices(&surahs, "2"), vec![1]);
        assert_eq!(filter_surah_indices(&surahs, "baq"), vec![1]);
        assert_eq!(filter_surah_indices(&surahs, "ALI"), vec![2]);
    }

    #[test]
    fn filter_surah_indices_empty_query_returns_all() {
        let surahs = vec![mk_surah(1, "A"), mk_surah(2, "B"), mk_surah(3, "C")];
        assert_eq!(filter_surah_indices(&surahs, ""), vec![0, 1, 2]);
    }

    #[test]
    fn compute_visible_window_bounds_are_valid() {
        let VisibleWindow { start, end } = compute_visible_window(8, 20, 5);
        assert!(start < end);
        assert!(end <= 20);
        assert!(end - start <= 5);
    }
}

