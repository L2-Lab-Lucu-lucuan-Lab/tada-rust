use super::*;
use arabic_reshaper::ArabicReshaper;
use ratatui::layout::Alignment;
use ratatui::widgets::{Gauge, ListState, Padding};
use unicode_bidi::BidiInfo;
use unicode_normalization::UnicodeNormalization;

fn format_arabic(text: &str, max_width: Option<u16>) -> String {
    let mut reshaper = ArabicReshaper::new();

    if let Some(val) = reshaper.configuration.get_mut("delete_harakat") {
        *val = false;
    }
    if let Some(val) = reshaper.configuration.get_mut("delete_tatweel") {
        *val = false;
    }

    let normalized: String = text.nfkc().collect();
    let reshaped = reshaper.reshape(&normalized);
    let lines = if let Some(width) = max_width {
        if width > 0 {
            textwrap::wrap(&reshaped, width as usize)
        } else {
            vec![std::borrow::Cow::Borrowed(&reshaped[..])]
        }
    } else {
        vec![std::borrow::Cow::Borrowed(&reshaped[..])]
    };

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }

        let bidi_info = BidiInfo::new(line, Some(unicode_bidi::Level::rtl()));
        if let Some(para) = bidi_info.paragraphs.first() {
            let range = para.range.clone();
            result.push_str(&bidi_info.reorder_line(para, range));
        } else {
            result.push_str(line);
        }
    }

    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderDensity {
    Wide,
    Compact,
}

#[derive(Debug, Clone, Copy)]
struct ViewportMode {
    density: ReaderDensity,
    show_sidebar: bool,
    sidebar_width: u16,
}

#[derive(Debug, Clone, Copy)]
struct UiScale {
    header_height: u16,
    footer_height: u16,
}

impl UiScale {
    fn from_area(area: Rect) -> Self {
        Self {
            header_height: 2,
            footer_height: if area.height >= 28 { 4 } else { 3 },
        }
    }
}

fn reader_density(area: Rect) -> ReaderDensity {
    if area.width >= 104 && area.height >= 26 {
        ReaderDensity::Wide
    } else {
        ReaderDensity::Compact
    }
}

fn ayah_row_height(density: ReaderDensity, show_translation: bool) -> u16 {
    match (density, show_translation) {
        (ReaderDensity::Wide, true) => 8,
        (ReaderDensity::Wide, false) => 6,
        (ReaderDensity::Compact, true) => 9,
        (ReaderDensity::Compact, false) => 7,
    }
}

fn padded_arabic_text(text: &str, height: u16) -> String {
    if height == 0 || text.is_empty() {
        return text.to_string();
    }

    let line_count = text.lines().count().max(1) as u16;
    if line_count >= height {
        return text.to_string();
    }

    let spare_lines = height - line_count;
    let top_padding = if spare_lines > 1 { 1 } else { 0 };
    let bottom_padding = 0;
    let mut result = String::new();

    for _ in 0..top_padding {
        result.push('\n');
    }
    result.push_str(text);
    for _ in 0..bottom_padding {
        result.push('\n');
    }

    result
}

fn pad_line_end(mut line: Line<'static>, width: u16) -> Line<'static> {
    let target_width = width as usize;
    let content_width = line.width();
    if target_width > content_width {
        line.spans
            .push(Span::raw(" ".repeat(target_width - content_width)));
    }
    line
}

fn sidebar_width(area_width: u16) -> u16 {
    let preferred = area_width.saturating_mul(24) / 100;
    let min_sidebar = 28;
    let max_sidebar = area_width.saturating_sub(72).max(min_sidebar);
    preferred.max(min_sidebar).min(max_sidebar)
}

pub(super) fn can_show_sidebar_in_frame(area: Rect) -> bool {
    let scale = UiScale::from_area(area);
    let body_height = area
        .height
        .saturating_sub(scale.header_height + scale.footer_height);
    area.width >= 132 && body_height >= 28
}

fn viewport_mode(area: Rect, state: &TuiState) -> ViewportMode {
    ViewportMode {
        density: reader_density(area),
        show_sidebar: !state.sidebar_collapsed && can_show_sidebar_in_frame(area),
        sidebar_width: sidebar_width(area.width),
    }
}

pub(super) fn draw_ui(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    frame.render_widget(Clear, frame.area());
    frame.render_widget(Block::default().style(state.theme.app_bg), frame.area());

    let scale = UiScale::from_area(frame.area());
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(scale.header_height),
            Constraint::Min(1),
            Constraint::Length(scale.footer_height),
        ])
        .split(frame.area());

    draw_header(frame, root[0], state);
    draw_body(frame, root[1], state);
    draw_footer(frame, root[2], state);

    match state.mode {
        UiMode::CommandPalette => draw_palette_overlay(frame, state),
        UiMode::SearchInline => draw_search_overlay(frame, state),
        UiMode::BookmarkOverlay => draw_bookmark_overlay(frame, state),
        UiMode::HelpOverlay => draw_help_overlay(frame, state),
        UiMode::Reading => {}
    }
}

fn draw_header(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .style(state.theme.panel)
        .border_style(state.theme.frame)
        .padding(Padding::new(1, 1, 0, 0));
    frame.render_widget(block.clone(), area);

    let inner = block.inner(area);
    if inner.height == 0 {
        return;
    }

    let current_ref = state
        .current_ayah()
        .map(|ayah| format!("{}:{}", ayah.surah_no, ayah.ayah_no))
        .unwrap_or_else(|| "-".to_string());
    let summary = Line::from(vec![
        Span::styled(
            format!(
                "{:02}. {}",
                state.current_surah().surah_no,
                state.current_surah().name_id.to_ascii_uppercase()
            ),
            state.theme.strong,
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} ayat", state.current_surah().ayah_count),
            state.theme.muted,
        ),
        Span::raw("  "),
        Span::styled(format!("ayat aktif {current_ref}"), state.theme.accent),
    ]);
    let summary_width = summary.width().min(inner.width as usize) as u16;

    if inner.width > summary_width.saturating_add(8) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(summary_width)])
            .split(inner);
        frame.render_widget(
            Paragraph::new(pad_line_end(build_nav_line(state), cols[0].width))
                .style(state.theme.frame),
            cols[0],
        );
        frame.render_widget(
            Paragraph::new(summary)
                .style(state.theme.frame)
                .alignment(Alignment::Right),
            cols[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new(pad_line_end(build_nav_line(state), inner.width))
                .style(state.theme.frame),
            inner,
        );
    }
}

fn build_nav_line(state: &TuiState) -> Line<'static> {
    let active_reader = matches!(state.mode, UiMode::Reading)
        && (state.sidebar_collapsed || state.focus == PaneFocus::AyahReader);
    let active_surah = matches!(state.mode, UiMode::Reading)
        && !state.sidebar_collapsed
        && state.focus == PaneFocus::SurahCards;
    let active_search = matches!(state.mode, UiMode::SearchInline);
    let active_bookmark = matches!(state.mode, UiMode::BookmarkOverlay);
    let active_help = matches!(state.mode, UiMode::HelpOverlay);

    let tab_style = |active: bool| {
        if active {
            state.theme.chip
        } else {
            state.theme.muted
        }
    };

    Line::from(vec![
        Span::styled("TADA", state.theme.accent),
        Span::raw(""),
        Span::styled("RUST", state.theme.chip),
        Span::raw(" "),
        Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), state.theme.muted),
        Span::raw("  "),
        Span::styled(" READER ", tab_style(active_reader)),
        Span::raw(" "),
        Span::styled(" SURAH ", tab_style(active_surah)),
        Span::raw(" "),
        Span::styled(" SEARCH ", tab_style(active_search)),
        Span::raw(" "),
        Span::styled(" BOOKMARK ", tab_style(active_bookmark)),
        Span::raw(" "),
        Span::styled(" HELP ", tab_style(active_help)),
    ])
}

fn draw_body(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    let viewport = viewport_mode(area, state);
    if viewport.show_sidebar {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(viewport.sidebar_width),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(area);
        draw_surah_sidebar(frame, chunks[0], state);
        draw_reader_workspace(frame, chunks[2], state, viewport);
    } else {
        draw_reader_workspace(frame, area, state, viewport);
    }
}

fn draw_surah_sidebar(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    let outer = Block::default()
        .title(if state.focus == PaneFocus::SurahCards {
            "Surah Browser [Focus]"
        } else {
            "Surah Browser"
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(state.theme.panel)
        .border_style(if state.focus == PaneFocus::SurahCards {
            state.theme.accent
        } else {
            state.theme.frame
        })
        .padding(Padding::new(1, 1, 0, 0));
    frame.render_widget(outer.clone(), area);

    let inner = outer.inner(area);
    if inner.height < 7 {
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(inner);

    let filter_line = if state.surah_filter.is_empty() {
        Line::from(vec![
            Span::styled("Filter ", state.theme.muted),
            Span::styled("ketik nama/nomor surat", state.theme.frame),
        ])
    } else {
        Line::from(vec![
            Span::styled("Filter ", state.theme.muted),
            Span::styled(state.surah_filter.clone(), state.theme.accent),
        ])
    };
    frame.render_widget(
        Paragraph::new(filter_line).style(state.theme.frame).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(state.theme.frame),
        ),
        sections[0],
    );

    let filtered = state.filtered_surah_indices();
    if filtered.is_empty() {
        frame.render_widget(
            Paragraph::new("Tidak ada surat yang cocok.")
                .style(state.theme.muted)
                .alignment(Alignment::Center),
            sections[1],
        );
    } else {
        let item_height: u16 = 3;
        let visible_items = ((sections[1].height as usize) / item_height as usize).max(1);
        let cursor_pos = filtered
            .iter()
            .position(|&idx| idx == state.surah_cursor_idx)
            .unwrap_or(0);
        let window = compute_visible_window(cursor_pos, filtered.len(), visible_items);

        let mut constraints: Vec<Constraint> = (0..visible_items)
            .map(|_| Constraint::Length(item_height))
            .collect();
        constraints.push(Constraint::Min(0));
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(sections[1]);

        for slot in 0..visible_items {
            let filtered_idx = window.start + slot;
            if filtered_idx >= window.end {
                break;
            }

            let surah_idx = filtered[filtered_idx];
            let surah = &state.surahs[surah_idx];
            let is_selected = surah_idx == state.selected_surah_idx;
            let is_cursor = surah_idx == state.surah_cursor_idx;
            let is_focus = state.focus == PaneFocus::SurahCards && is_cursor;
            let style = if is_focus {
                state.theme.card_focus
            } else if is_selected {
                state.theme.card_active
            } else {
                state.theme.card
            };

            let lines = vec![
                Line::from(vec![
                    Span::styled(format!("{:>3}", surah.surah_no), state.theme.accent),
                    Span::raw(" "),
                    Span::styled(&surah.name_id, state.theme.strong),
                ]),
                Line::from(vec![
                    Span::styled(format_arabic(&surah.name_ar, None), state.theme.frame),
                    Span::raw("  "),
                    Span::styled(format!("{} ayat", surah.ayah_count), state.theme.muted),
                ]),
            ];
            frame.render_widget(
                Paragraph::new(lines)
                    .style(style)
                    .wrap(Wrap { trim: true })
                    .block(
                        Block::default()
                            .borders(Borders::BOTTOM)
                            .style(style)
                            .border_style(if is_focus {
                                state.theme.accent
                            } else {
                                state.theme.frame
                            })
                            .padding(Padding::new(1, 0, 0, 0)),
                    ),
                rows[slot],
            );
        }
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Enter", state.theme.chip),
            Span::raw(" buka  "),
            Span::styled("Tab", state.theme.chip),
            Span::raw(" fokus"),
        ]))
        .style(state.theme.frame),
        sections[2],
    );
}

fn draw_reader_workspace(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    state: &TuiState,
    viewport: ViewportMode,
) {
    let surface = Block::default()
        .style(state.theme.app_bg)
        .padding(Padding::new(1, 1, 0, 0));
    frame.render_widget(surface.clone(), area);
    let inner = surface.inner(area);
    if inner.height < 4 {
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(1)])
        .split(inner);

    draw_reader_summary(frame, sections[0], state);
    draw_ayah_list(frame, sections[1], state, viewport);
}

fn draw_reader_summary(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .style(state.theme.panel)
        .border_style(state.theme.frame);
    frame.render_widget(block.clone(), area);

    let inner = block.inner(area);
    if inner.height == 0 {
        return;
    }

    let cols = if inner.width >= 74 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(26)])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1)])
            .split(inner)
    };

    let (audio_state, speed) = if let Some(player) = &state.player {
        let status = if player.is_paused() { "PAUSE" } else { "PLAY" };
        (status, format!("{:.2}x", player.playback_rate()))
    } else {
        ("STOP", "1.00x".to_string())
    };

    let summary_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(cols[0]);
    let summary_lines = [
        pad_line_end(
            Line::from(vec![
                Span::styled("SURAH ", state.theme.muted),
                Span::styled(
                    state.current_surah().name_id.to_ascii_uppercase(),
                    state.theme.strong,
                ),
                Span::raw("  "),
                Span::styled(format!(" {} ", audio_state), state.theme.chip),
            ]),
            cols[0].width,
        ),
        pad_line_end(
            Line::from(vec![
                Span::styled(
                    format!("Qari {}", qari_name(&state.active_qari)),
                    state.theme.frame,
                ),
                Span::raw("  "),
                Span::styled(format!("speed {speed}"), state.theme.muted),
                Span::raw("  "),
                Span::styled(
                    if state.show_translation {
                        "terjemahan aktif"
                    } else {
                        "reader fokus Arabic"
                    },
                    state.theme.accent,
                ),
            ]),
            cols[0].width,
        ),
        pad_line_end(
            Line::from(vec![Span::styled(
                if state.sidebar_collapsed {
                    "Ctrl+B untuk membuka panel surat."
                } else {
                    "Panel surat terbuka. Ketik untuk filter."
                },
                state.theme.muted,
            )]),
            cols[0].width,
        ),
    ];
    for (idx, line) in summary_lines.into_iter().enumerate() {
        if idx < summary_rows.len() {
            frame.render_widget(
                Paragraph::new(line).style(state.theme.frame),
                summary_rows[idx],
            );
        }
    }

    if cols.len() > 1 {
        frame.render_widget(
            Paragraph::new(format_arabic(&state.current_surah().name_ar, None))
                .style(state.theme.strong)
                .alignment(Alignment::Right),
            cols[1],
        );
    }
}

fn draw_ayah_list(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    state: &TuiState,
    viewport: ViewportMode,
) {
    if area.width < 20 || area.height < 4 {
        return;
    }

    if state.ayahs.is_empty() {
        frame.render_widget(
            Paragraph::new("Belum ada ayat untuk surah ini.")
                .style(state.theme.muted)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let row_height = ayah_row_height(viewport.density, state.show_translation);
    let visible_rows = ((area.height as usize) / row_height as usize).max(1);
    let window = compute_visible_window(state.selected_ayah_idx, state.ayahs.len(), visible_rows);

    let mut constraints: Vec<Constraint> = (0..visible_rows)
        .map(|_| Constraint::Length(row_height))
        .collect();
    constraints.push(Constraint::Min(0));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for slot in 0..visible_rows {
        let ayah_idx = window.start + slot;
        if ayah_idx >= window.end {
            break;
        }
        draw_ayah_row(
            frame,
            rows[slot],
            &state.ayahs[ayah_idx],
            ayah_idx == state.selected_ayah_idx,
            state,
            viewport.density,
        );
    }
}

fn draw_ayah_row(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    ayah: &Ayah,
    is_selected: bool,
    state: &TuiState,
    density: ReaderDensity,
) {
    let base_style = if is_selected {
        state.theme.card_focus
    } else {
        state.theme.card
    };
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .style(base_style)
        .border_style(if is_selected {
            state.theme.accent
        } else {
            state.theme.muted
        });
    frame.render_widget(block.clone(), area);

    let inner = block.inner(area);
    if inner.height < 3 {
        return;
    }

    match density {
        ReaderDensity::Wide => draw_ayah_row_wide(frame, inner, ayah, is_selected, state),
        ReaderDensity::Compact => draw_ayah_row_compact(frame, inner, ayah, is_selected, state),
    }
}

fn draw_ayah_row_wide(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    ayah: &Ayah,
    is_selected: bool,
    state: &TuiState,
) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    draw_row_indicator(frame, cols[0], is_selected, state);

    let secondary_height = secondary_panel_height(ayah, state);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(secondary_height),
        ])
        .split(cols[1]);

    frame.render_widget(
        Paragraph::new(build_row_header(ayah, is_selected, state))
            .style(state.theme.frame)
            .wrap(Wrap { trim: true }),
        rows[0],
    );

    let arabic_text = padded_arabic_text(
        &format_arabic(&ayah.arabic_text, Some(rows[1].width.saturating_sub(1))),
        rows[1].height,
    );
    frame.render_widget(
        Paragraph::new(arabic_text)
            .style(state.theme.strong)
            .alignment(Alignment::Right)
            .wrap(Wrap { trim: true }),
        rows[1],
    );

    if secondary_height > 0 {
        frame.render_widget(
            Paragraph::new(build_secondary_lines(ayah, state))
                .style(state.theme.frame)
                .wrap(Wrap { trim: true }),
            rows[2],
        );
    }
}

fn draw_ayah_row_compact(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    ayah: &Ayah,
    is_selected: bool,
    state: &TuiState,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(secondary_panel_height(ayah, state)),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(build_row_header(ayah, is_selected, state))
            .style(state.theme.frame)
            .wrap(Wrap { trim: true }),
        rows[0],
    );

    let arabic_text = padded_arabic_text(
        &format_arabic(&ayah.arabic_text, Some(rows[1].width.saturating_sub(1))),
        rows[1].height,
    );
    frame.render_widget(
        Paragraph::new(arabic_text)
            .style(state.theme.strong)
            .alignment(Alignment::Right)
            .wrap(Wrap { trim: true }),
        rows[1],
    );

    if secondary_panel_height(ayah, state) > 0 {
        frame.render_widget(
            Paragraph::new(build_secondary_lines(ayah, state))
                .style(state.theme.frame)
                .wrap(Wrap { trim: true }),
            rows[2],
        );
    }
}

fn draw_row_indicator(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    is_selected: bool,
    state: &TuiState,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let text = std::iter::repeat(if is_selected { "|" } else { " " })
        .take(area.height as usize)
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(text).style(if is_selected {
            state.theme.accent
        } else {
            state.theme.frame
        }),
        area,
    );
}

fn build_row_header(ayah: &Ayah, is_selected: bool, state: &TuiState) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {}:{} ", ayah.surah_no, ayah.ayah_no),
            if is_selected {
                state.theme.chip
            } else {
                state.theme.muted
            },
        ),
        Span::raw(" "),
        Span::styled(
            if ayah.audio_url.is_some() {
                "audio siap"
            } else {
                "tanpa audio"
            },
            state.theme.muted,
        ),
    ])
}

fn secondary_panel_height(ayah: &Ayah, state: &TuiState) -> u16 {
    let mut lines = 0;
    if ayah.transliteration.as_deref().is_some() {
        lines += 1;
    }
    if state.show_translation {
        lines += 1;
    }
    lines
}

fn build_secondary_lines(ayah: &Ayah, state: &TuiState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(translit) = ayah.transliteration.as_deref() {
        lines.push(Line::from(Span::styled(
            translit.to_string(),
            state.theme.muted,
        )));
    }
    if state.show_translation {
        lines.push(Line::from(Span::styled(
            ayah.translation.as_deref().unwrap_or("-").to_string(),
            state.theme.frame,
        )));
    }
    lines
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
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::TOP)
        .style(state.theme.panel)
        .border_style(state.theme.frame)
        .padding(Padding::new(1, 1, 0, 0));
    frame.render_widget(block.clone(), area);

    let inner = block.inner(area);
    if inner.height == 0 {
        return;
    }

    let rows = if inner.height > 1 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1)])
            .split(inner)
    };

    let ratio = if state.ayahs.is_empty() {
        0.0
    } else {
        (state.selected_ayah_idx.saturating_add(1) as f64 / state.ayahs.len() as f64)
            .clamp(0.0, 1.0)
    };
    frame.render_widget(
        Gauge::default()
            .ratio(ratio)
            .label(format!(
                "Ayat {:>3} / {:>3}",
                state.selected_ayah_idx.saturating_add(1),
                state.ayahs.len()
            ))
            .style(state.theme.card)
            .gauge_style(state.theme.accent)
            .use_unicode(true),
        rows[0],
    );

    if rows.len() > 1 {
        let audio_badge = if let Some(player) = &state.player {
            if player.is_paused() { "PAUSE" } else { "PLAY" }
        } else {
            "STOP"
        };
        let controls = pad_line_end(
            Line::from(vec![
                Span::styled(format!(" {} ", audio_badge), state.theme.chip),
                Span::raw(" "),
                Span::styled(state.status.clone(), state.theme.frame),
                Span::raw("  "),
                Span::styled(
                    "Space play  [/] prev-next  s stop  r repeat  ,/. speed  b bookmarks  f save  u unsave  Shift+Q qari",
                    state.theme.muted,
                ),
                Span::raw("  "),
                Span::styled(
                    "Ctrl+B surat  / search  Ctrl+K actions  F1 help  q quit",
                    state.theme.accent,
                ),
            ]),
            rows[1].width,
        );
        frame.render_widget(Paragraph::new(controls).style(state.theme.frame), rows[1]);
    }
}

fn draw_palette_overlay(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let area = centered_rect(frame.area(), 74, 58);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Actions")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(state.theme.panel)
        .border_style(state.theme.accent)
        .padding(Padding::new(1, 1, 0, 0));
    frame.render_widget(block.clone(), area);

    let inner = block.inner(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(format!("> {}", state.palette_input))
            .style(state.theme.strong)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(state.theme.frame),
            ),
        sections[0],
    );

    let items = state.filtered_palette();
    let list_items: Vec<ListItem<'_>> = items
        .iter()
        .map(|item| ListItem::new(Line::from(vec![Span::raw(item.label)])))
        .collect();

    let theme = state.current_theme();
    let mut list_state = ListState::default();
    list_state.select(Some(state.palette_selected_idx));

    frame.render_stateful_widget(
        List::new(list_items)
            .block(Block::default().style(state.theme.panel))
            .highlight_style(
                Style::default()
                    .bg(theme.highlight_bg)
                    .fg(theme.highlight_fg),
            )
            .highlight_symbol("> "),
        sections[1],
        &mut list_state,
    );
}

fn draw_search_overlay(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let area = centered_rect(frame.area(), 76, 62);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Search Ayah")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(state.theme.panel)
        .border_style(state.theme.accent)
        .padding(Padding::new(1, 1, 0, 0));
    frame.render_widget(block.clone(), area);

    let inner = block.inner(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(format!("/ {}", state.search_input))
            .style(state.theme.strong)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(state.theme.frame),
            ),
        sections[0],
    );

    let (items, mut list_state) = if state.search_results.is_empty() {
        (
            vec![ListItem::new(Line::from(vec![Span::styled(
                "Belum ada hasil. Tekan Enter untuk mencari.",
                state.theme.muted,
            )]))],
            ListState::default(),
        )
    } else {
        let items: Vec<ListItem<'_>> = state
            .search_results
            .iter()
            .map(|hit| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{}:{} ", hit.surah_no, hit.ayah_no),
                        state.theme.accent,
                    ),
                    Span::raw(hit.snippet.clone()),
                ]))
            })
            .collect();
        let mut s = ListState::default();
        s.select(Some(state.selected_search_idx));
        (items, s)
    };

    let theme = state.current_theme();
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().style(state.theme.panel))
            .highlight_style(
                Style::default()
                    .bg(theme.highlight_bg)
                    .fg(theme.highlight_fg),
            ),
        sections[1],
        &mut list_state,
    );
}

fn draw_bookmark_overlay(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let area = centered_rect(frame.area(), 70, 58);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Bookmarks")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(state.theme.panel)
        .border_style(state.theme.accent)
        .padding(Padding::new(1, 1, 0, 0));
    frame.render_widget(block.clone(), area);

    let inner = block.inner(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(inner);
    let (items, mut list_state) = if state.bookmarks.is_empty() {
        (
            vec![ListItem::new(Line::from(vec![Span::styled(
                "Belum ada bookmark.",
                state.theme.muted,
            )]))],
            ListState::default(),
        )
    } else {
        let items: Vec<ListItem<'_>> = state
            .bookmarks
            .iter()
            .map(|b| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("#{} ", b.id), state.theme.accent),
                    Span::styled(
                        format!("{}:{} ", b.surah_no, b.ayah_no),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(b.note.as_deref().unwrap_or("-").to_string()),
                ]))
            })
            .collect();
        let mut s = ListState::default();
        s.select(Some(state.selected_bookmark_idx));
        (items, s)
    };

    let theme = state.current_theme();
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().style(state.theme.panel))
            .highlight_style(
                Style::default()
                    .bg(theme.highlight_bg)
                    .fg(theme.highlight_fg),
            ),
        sections[0],
        &mut list_state,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Enter", state.theme.chip),
            Span::raw(" lompat  "),
            Span::styled("d/Delete", state.theme.chip),
            Span::raw(" hapus  "),
            Span::styled("Esc", state.theme.chip),
            Span::raw(" tutup"),
        ]))
        .style(state.theme.frame),
        sections[1],
    );
}

fn draw_help_overlay(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let area = centered_rect(frame.area(), 72, 64);
    frame.render_widget(Clear, area);

    let text = [
        "Reader-first keymap",
        "",
        "j / k      : Pindah ayat aktif",
        "n / p      : Surah berikutnya / sebelumnya",
        "Ctrl+B     : Tampil/sembunyi panel surat",
        "Tab        : Pindah fokus panel surat <-> panel baca",
        "Ketik      : Filter surat saat panel surat fokus",
        "Enter      : Buka surat dari panel surat",
        "/          : Search ayat",
        "b          : Buka daftar bookmark",
        "f          : Tambah bookmark",
        "u          : Unbookmark ayat aktif",
        "Enter      : Lompat ke bookmark saat daftar bookmark terbuka",
        "d/Delete   : Hapus bookmark terpilih saat daftar bookmark terbuka",
        "Ctrl+K     : Actions / command palette",
        "Space      : Play / pause audio dari ayat aktif",
        "[ / ]      : Ayat audio sebelumnya / berikutnya",
        "s          : Stop audio",
        "r          : Ulangi ayat aktif",
        ", / .      : Turunkan / naikkan speed",
        "Shift+Q    : Ganti qari",
        "F1 / Esc   : Tutup help atau overlay",
        "q          : Keluar aplikasi",
        "",
        "Mode baca penuh adalah default. Panel surat dibuka saat dibutuhkan.",
    ]
    .join("\n");

    frame.render_widget(
        Paragraph::new(text)
            .style(state.theme.frame)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(state.theme.panel)
                    .border_style(state.theme.accent)
                    .padding(Padding::new(1, 1, 0, 0)),
            ),
        area,
    );
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

pub(super) fn frame_size_hint(terminal: &Terminal<CrosstermBackend<Stdout>>) -> (u16, u16) {
    terminal
        .size()
        .map(|s| (s.width, s.height))
        .unwrap_or((120, 36))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, env};

    use super::{
        ReaderDensity, TuiState, VisibleWindow, ayah_row_height, can_show_sidebar_in_frame,
        compute_visible_window, draw_ui, filter_surah_indices, pad_line_end, padded_arabic_text,
        reader_density,
    };
    use crate::{
        audio::AudioCache,
        domain::{Ayah, SurahMeta},
    };
    use ratatui::layout::Rect;
    use ratatui::text::Line;
    use ratatui::{Terminal, backend::TestBackend};

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

    fn mk_ayah(no: u16, arabic_text: &str, translation: &str) -> Ayah {
        Ayah {
            surah_no: 1,
            ayah_no: no,
            arabic_text: arabic_text.to_string(),
            transliteration: Some(format!("translit-{no}")),
            translation: Some(translation.to_string()),
            audio_url: Some(format!("https://example.test/{no}.mp3")),
            audio_urls: BTreeMap::new(),
        }
    }

    fn mk_state_for_full_render() -> TuiState {
        let cache_dir = env::temp_dir().join("tada-rust-render-regression-tests");
        let mut state = TuiState::new(
            vec![
                mk_surah(1, "LONGSURAHMARKER-ALPHA-BETA-GAMMA"),
                mk_surah(2, "A"),
            ],
            "dark",
            true,
            true,
            AudioCache::new(cache_dir, false, 1).expect("audio cache"),
            "01".to_string(),
        );
        state.ayahs = vec![
            mk_ayah(1, "الٓمٓ", "translation-1"),
            mk_ayah(2, "اللَّهُ لَا إِلَٰهَ إِلَّا هُوَ", "translation-2"),
        ];
        state.selected_ayah_idx = 0;
        state
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        buffer
            .content
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
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

    #[test]
    fn reader_density_switches_for_roomy_layouts() {
        assert_eq!(
            reader_density(Rect::new(0, 0, 120, 32)),
            ReaderDensity::Wide
        );
        assert_eq!(
            reader_density(Rect::new(0, 0, 90, 24)),
            ReaderDensity::Compact
        );
    }

    #[test]
    fn ayah_row_height_grows_when_translation_is_enabled() {
        assert!(
            ayah_row_height(ReaderDensity::Wide, true)
                > ayah_row_height(ReaderDensity::Wide, false)
        );
    }

    #[test]
    fn ayah_row_height_grows_in_compact_mode() {
        assert!(
            ayah_row_height(ReaderDensity::Compact, true)
                > ayah_row_height(ReaderDensity::Wide, true)
        );
    }

    #[test]
    fn sidebar_visibility_uses_full_frame_constraints() {
        assert!(can_show_sidebar_in_frame(Rect::new(0, 0, 140, 35)));
        assert!(!can_show_sidebar_in_frame(Rect::new(0, 0, 131, 35)));
        assert!(!can_show_sidebar_in_frame(Rect::new(0, 0, 140, 33)));
    }

    #[test]
    fn padded_arabic_text_adds_single_top_padding() {
        let padded = padded_arabic_text("ayat", 5);
        let lines: Vec<_> = padded.split('\n').collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines.first(), Some(&""));
        assert_eq!(lines.get(1), Some(&"ayat"));
    }

    #[test]
    fn pad_line_end_fills_remaining_width() {
        let padded = pad_line_end(Line::from("abc"), 6);
        assert_eq!(padded.width(), 6);
    }

    #[test]
    fn draw_ui_clears_old_header_and_footer_text_on_redraw() {
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut state = mk_state_for_full_render();
        state.status = "STATUSMARKER alpha beta gamma delta epsilon zeta eta theta".to_string();

        terminal
            .draw(|frame| draw_ui(frame, &state))
            .expect("draw long state");
        let first = buffer_text(&terminal);
        assert!(first.contains("LONGSURAHMARKER"));
        assert!(first.contains("STATUSMARKER"));

        state.selected_surah_idx = 1;
        state.surah_cursor_idx = 1;
        state.status = "ok".to_string();
        terminal
            .draw(|frame| draw_ui(frame, &state))
            .expect("draw short state");

        let second = buffer_text(&terminal);
        assert!(second.contains("SURAH A"));
        assert!(second.contains(" ok "));
        assert!(!second.contains("LONGSURAHMARKER"));
        assert!(!second.contains("STATUSMARKER"));
    }

    #[test]
    fn draw_ui_keeps_ayah_row_anchor_stable_across_redraws() {
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut state = mk_state_for_full_render();

        terminal
            .draw(|frame| draw_ui(frame, &state))
            .expect("draw initial");
        let first = buffer_text(&terminal);
        let first_row = first
            .lines()
            .position(|line| line.contains("1:1"))
            .expect("first ayah row marker");

        state.status = "Audio berhenti".to_string();
        terminal
            .draw(|frame| draw_ui(frame, &state))
            .expect("draw second");
        let second = buffer_text(&terminal);
        let second_row = second
            .lines()
            .position(|line| line.contains("1:1"))
            .expect("second ayah row marker");

        assert_eq!(first_row, second_row);
    }

    #[test]
    fn test_arabic_reshape_behavior() {
        use super::format_arabic;
        let text = "Hello World";
        let reshaped = format_arabic(text, None);
        assert_eq!(text, reshaped, "Non-Arabic text should remain unchanged");

        let mixed = "Hello سلام World";
        let reshaped_mixed = format_arabic(mixed, None);
        assert_ne!(mixed, reshaped_mixed, "Arabic text should be reshaped");
        assert!(
            reshaped_mixed.contains("Hello"),
            "Should still contain 'Hello'"
        );
        assert!(
            reshaped_mixed.contains("World"),
            "Should still contain 'World'"
        );

        let with_harakat = "بِسْمِ اللَّهِ";
        let reshaped_harakat = format_arabic(with_harakat, None);
        assert_ne!(
            with_harakat, reshaped_harakat,
            "Arabic with harakat should be reshaped"
        );
        assert!(!reshaped_harakat.is_empty());
    }
}
