//! Help overlay generated from the active keymap, so it cannot drift.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered(area, 60, 70);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(app.i18n.t("help_title"));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines = Vec::new();
    for (action, key) in app.keymap.bindings() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>10}  ", key.display()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(app.i18n.t(action.i18n_key())),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        app.i18n.t("help_hint"),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(lines), inner);
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
