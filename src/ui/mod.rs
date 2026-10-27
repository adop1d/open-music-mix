//! Stateless renderers over `&App` (art keeps a small stateful protocol).

pub mod help;
pub mod keymap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap};
use ratatui_image::StatefulImage;

use crate::app::{App, LocalRow, Panel, SpotifyRow};
use crate::event::SourceStatus;
use crate::process::PlayerState;
use crate::queue::Source;

pub fn render(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let wide = area.width >= 80 && area.height >= 24;
    if wide {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
            ])
            .split(area);
        render_explorer(app, frame, chunks[0]);
        render_queue(app, frame, chunks[1]);
        render_now_playing(app, frame, chunks[2]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        render_queue(app, frame, chunks[0]);
        render_now_playing(app, frame, chunks[1]);
    }
    if app.ui.help_open {
        help::render(app, frame, area);
    }
    if app.ui.import_open {
        render_import(app, frame, area);
    }
}

fn block(app: &App, key: &str, focused: bool) -> Block<'static> {
    let style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    Block::default()
        .borders(Borders::ALL)
        .title(app.i18n.t(key))
        .border_style(style)
}

fn render_explorer(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Panel::Explorer;
    let block = block(app, "panel_explorer", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    lines.push(tab_line(app));
    if inner.height > 1 {
        lines.push(Line::from(""));
    }

    match app.tab {
        Source::Local => render_local(app, &mut lines, inner),
        Source::Youtube => render_youtube(app, &mut lines),
        Source::Spotify => render_spotify(app, &mut lines),
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn tab_line(app: &App) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, source) in Source::ALL.iter().enumerate() {
        let active = app.tab == *source;
        let label = format!("{} {}", index + 1, app.i18n.t(source.i18n_key()));
        let style = if active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if !app.is_available(*source) {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }
    Line::from(spans)
}

fn source_reason(app: &App, source: Source, lines: &mut Vec<Line>) -> bool {
    match app.statuses.get(&source) {
        Some(SourceStatus::Ready) => false,
        Some(status) => {
            if let Some(reason) = status.reason() {
                lines.push(Line::styled(
                    app.reason_message(reason),
                    Style::default().fg(Color::Red),
                ));
            }
            true
        }
        None => false,
    }
}

fn render_local(app: &App, lines: &mut Vec<Line>, inner: Rect) {
    if source_reason(app, Source::Local, lines) {
        return;
    }
    let header = format!(
        "{}  ({})",
        app.i18n.t(app.local.view.i18n_key()),
        app.i18n.t("view_artists")
    );
    lines.push(Line::styled(header, Style::default().fg(Color::Cyan)));
    let rows = app.filtered_local_rows();
    if rows.is_empty() {
        lines.push(Line::from(app.i18n.t("empty_library")));
        return;
    }
    let selection = app.local.selection.min(rows.len() - 1);
    let visible = inner.height as usize;
    let offset = selection.saturating_sub(visible.saturating_sub(1));
    for (index, row) in rows.iter().enumerate().skip(offset) {
        if lines.len() >= visible {
            break;
        }
        lines.push(local_line(
            app,
            row,
            index == selection && app.focus == Panel::Explorer,
        ));
    }
}

fn local_line(app: &App, row: &LocalRow, selected: bool) -> Line<'static> {
    let label = match row {
        LocalRow::Song(song) => {
            let artist = if song.artist.is_empty() {
                String::new()
            } else {
                format!(" — {}", song.artist)
            };
            format!("{}{artist}", song.title)
        }
        other => {
            let detail = other.detail();
            if detail.is_empty() {
                other.label()
            } else {
                format!("{} — {detail}", other.label())
            }
        }
    };
    let _ = app;
    Line::styled(label, selection_style(selected))
}

fn render_youtube(app: &App, lines: &mut Vec<Line>) {
    if source_reason(app, Source::Youtube, lines) {
        return;
    }
    lines.push(Line::styled(
        app.i18n.t("search_prompt"),
        Style::default().fg(Color::Cyan),
    ));
    let query = app.ui.search.clone().unwrap_or_default();
    lines.push(Line::from(if query.is_empty() {
        "…".to_string()
    } else {
        query
    }));
    lines.push(Line::styled(
        app.i18n.t("help_title"),
        Style::default().fg(Color::DarkGray),
    ));
}

fn render_spotify(app: &App, lines: &mut Vec<Line>) {
    if source_reason(app, Source::Spotify, lines) {
        return;
    }
    let rows = app.spotify_rows();
    if rows.is_empty() {
        lines.push(Line::from(app.i18n.t("empty_imports")));
        return;
    }
    let selection = app.spotify.selection.min(rows.len() - 1);
    for (index, row) in rows.iter().enumerate() {
        let selected = index == selection && app.focus == Panel::Explorer;
        let label = match row {
            SpotifyRow::Collection(collection) => {
                format!("{} ({} tracks)", collection.name, collection.tracks.len())
            }
            SpotifyRow::Track(track) => {
                if track.unavailable {
                    format!("{} [unavailable]", track.title)
                } else {
                    format!("{} — {}", track.title, track.artists.join(", "))
                }
            }
        };
        lines.push(Line::styled(label, selection_style(selected)));
    }
}

fn render_queue(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Panel::Queue;
    let block = block(app, "panel_queue", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = app.visible_queue_indices();
    if visible.is_empty() {
        let text = if app.queue.is_empty() {
            app.i18n.t("empty_queue")
        } else {
            app.i18n.t("empty_search")
        };
        frame.render_widget(Paragraph::new(text), inner);
        return;
    }
    let header_height = if app.ui.search.is_some() { 1 } else { 0 };
    let list_area = Rect {
        y: inner.y + header_height,
        height: inner.height.saturating_sub(header_height),
        ..inner
    };
    if header_height > 0 && inner.height > 0 {
        let query = app.ui.search.clone().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(format!("{}: {query}", app.i18n.t("search_prompt"))),
            Rect { height: 1, ..inner },
        );
    }
    let items: Vec<ListItem> = visible
        .iter()
        .map(|index| {
            let entry = &app.queue.entries[*index];
            let marker = match entry.state {
                crate::queue::EntryState::Playing => "▶ ",
                crate::queue::EntryState::Paused => "❚❚ ",
                crate::queue::EntryState::Failed(_) => "! ",
                crate::queue::EntryState::Unavailable => "× ",
                _ => "  ",
            };
            let label = format!(
                "{marker}[{}] {} — {}",
                entry.source.label(),
                entry.meta.title,
                entry.meta.artist_line()
            );
            let selected = *index == app.queue.selection && focused;
            ListItem::new(label).style(selection_style(selected))
        })
        .collect();
    frame.render_widget(List::new(items), list_area);
}

fn render_now_playing(app: &mut App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Panel::NowPlaying;
    let block = block(app, "panel_now_playing", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    if app.playback.entry_id.is_none() {
        let mut lines = vec![Line::from(app.i18n.t("now_playing_nothing"))];
        if let Some(notice) = &app.ui.notice {
            lines.push(Line::styled(
                notice.clone(),
                Style::default().fg(Color::Red),
            ));
        }
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        return;
    }

    let art_height = inner.height / 2;
    let (art_area, info_area) = if art_height >= 3 {
        (
            Some(Rect {
                height: art_height,
                ..inner
            }),
            Rect {
                y: inner.y + art_height,
                height: inner.height.saturating_sub(art_height),
                ..inner
            },
        )
    } else {
        (None, inner)
    };

    if let Some(art_area) = art_area {
        app.art.prepare(&app.art_renderer, art_area);
        if let Some(protocol) = app.art.protocol.as_mut() {
            frame.render_stateful_widget(StatefulImage::new(), art_area, protocol);
        } else {
            let placeholder = app
                .art
                .placeholder
                .clone()
                .unwrap_or_else(|| app.i18n.t("now_playing_nothing"));
            frame.render_widget(
                Paragraph::new(placeholder)
                    .style(Style::default().fg(Color::DarkGray))
                    .wrap(Wrap { trim: true }),
                art_area,
            );
        }
    }

    let title = app.playback.title.clone().unwrap_or_default();
    let source = app
        .playback
        .source
        .map(|source| app.i18n.t(source.i18n_key()))
        .unwrap_or_default();
    let meta = app
        .playback
        .entry_id
        .and_then(|id| app.queue.entry(id))
        .map(|entry| entry.meta.clone());
    let artist = meta
        .as_ref()
        .map(|meta| meta.artist_line())
        .unwrap_or_default();
    let album = meta
        .as_ref()
        .and_then(|meta| meta.album.clone())
        .unwrap_or_default();
    let state = match app.playback.state {
        PlayerState::Playing => app.i18n.t("status_playing"),
        PlayerState::Paused => app.i18n.t("status_paused"),
        PlayerState::Stopped => app.i18n.t("status_stopped"),
    };
    let mut lines = vec![
        Line::styled(
            format!("{title}  [{source}]"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(artist),
        Line::from(album),
        Line::from(format!("{state}  {}", app.i18n.t("now_playing_volume"))),
    ];
    if let Some(notice) = &app.ui.notice {
        lines.push(Line::styled(
            notice.clone(),
            Style::default().fg(Color::Red),
        ));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), info_area);

    if info_area.height >= 2 {
        let ratio = match app.playback.duration {
            Some(duration) if duration.as_secs() > 0 => {
                (app.playback.position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
        let gauge_area = Rect {
            y: info_area.y + info_area.height - 1,
            height: 1,
            ..info_area
        };
        let label = format!(
            "{} / {}",
            format_duration(app.playback.position),
            app.playback
                .duration
                .map(format_duration)
                .unwrap_or_else(|| "--:--".to_string())
        );
        let gauge = Gauge::default()
            .ratio(ratio)
            .label(label)
            .style(Style::default().fg(Color::Green));
        frame.render_widget(gauge, gauge_area);
    }
}

fn render_import(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered(area, 60, 30);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(app.i18n.t("import_title"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let mut lines = vec![
        Line::from(app.i18n.t("import_prompt")),
        Line::from(app.ui.import_input.clone()),
        Line::from(""),
    ];
    if let Some(status) = &app.ui.import_status {
        lines.push(Line::from(format!(
            "{} {} — {} / {}",
            app.i18n.t("msg_import_progress"),
            status.name,
            status.matched,
            status.total
        )));
    }
    lines.push(Line::styled(
        app.i18n.t("import_start"),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn centered(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn selection_style(selected: bool) -> Style {
    if selected {
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

pub fn format_duration(duration: std::time::Duration) -> String {
    let total = duration.as_secs();
    format!("{:02}:{:02}", total / 60, total % 60)
}
