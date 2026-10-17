//! Home screen view (recommendations).
//!
//! Recommendation sections use Search-style cards in a responsive grid.
//! Tab / Shift-Tab move between cards; Up/Down (or j/k) move within one.
//! Enter plays a track or opens a playlist. Empty sections are omitted, and
//! paging keeps the selected card visible without shrinking its rows away.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, TableState};
use ytmusic_api::{HomeSection, HomeSectionItem, Track};

use super::{PageState, Theme, borderless_table, section_title, table_header, table_row};
use crate::formatting::format_duration;
use crate::layout::{Orientation, detect_orientation};

/// What an Enter keypress on the home view resolves to.
///
/// Returned by [`HomeView::activate_selected`] so the caller (the main loop)
/// can translate it into an [`crate::app::AppCommand`]. Mirrors Python's
/// `_handle_item_selection` dispatch: a track plays; a playlist would navigate
/// to the playlist view (not yet ported — see [`HomeAction::OpenPlaylist`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeAction {
    /// Play this track (Python `_play_track`). Carries the full [`Track`] (not
    /// just the `video_id`) so the runtime can seed the queue and the player
    /// bar with the title/artist/album/duration without a second lookup.
    Play(Track),
    /// Open the playlist with this `PlaylistInfo` (Python `_open_playlist`).
    ///
    /// Carries the full info so the playlist view can label its track list and
    /// fetch by `playlist_id` (Python passed the whole `PlaylistInfo` to
    /// `show_track_list`).
    OpenPlaylist(ytmusic_api::PlaylistInfo),
}

/// The home recommendations view: a fetch state plus a `(section, item)`
/// cursor over the loaded sections.
///
/// The cursor is only meaningful in [`PageState::Loaded`]; it is kept on the
/// struct (not inside the enum) so it survives a re-render and is reset
/// deliberately by [`HomeView::set_sections`].
#[derive(Debug, Clone)]
pub struct HomeView {
    state: PageState<Vec<HomeSection>>,
    /// Index into the *non-empty* sections of the loaded payload.
    section_idx: usize,
    /// Index into the active section's items.
    item_idx: usize,
}

impl Default for HomeView {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeView {
    /// A fresh home view in the loading state with the cursor at the origin.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: PageState::Loading,
            section_idx: 0,
            item_idx: 0,
        }
    }

    /// The current fetch state (for tests and the main loop).
    #[must_use]
    pub fn state(&self) -> &PageState<Vec<HomeSection>> {
        &self.state
    }

    /// Replace the loaded sections, dropping empty ones, and reset the cursor.
    ///
    /// Empty sections are filtered here (Python skipped them in `_render_sections`
    /// and made their tables non-focusable), so every retained section has at
    /// least one item and the cursor can always land on a real item.
    pub fn set_sections(&mut self, sections: Vec<HomeSection>) {
        let non_empty: Vec<HomeSection> = sections
            .into_iter()
            .filter(|s| !s.items.is_empty())
            .collect();
        self.state = PageState::Loaded(non_empty);
        self.section_idx = 0;
        self.item_idx = 0;
    }

    /// Transition into the error state with a classified message.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.state = PageState::Error(message.into());
    }

    /// The non-empty sections, or an empty slice when not loaded.
    fn sections(&self) -> &[HomeSection] {
        self.state.loaded().map_or(&[], Vec::as_slice)
    }

    /// The item currently under the cursor, if any.
    #[must_use]
    pub fn selected_item(&self) -> Option<&HomeSectionItem> {
        let section = self.sections().get(self.section_idx)?;
        section.items.get(self.item_idx)
    }

    /// The item under the cursor as a [`PopupItem`] for the action popup
    /// (a track or a playlist). `None` when nothing is selected.
    #[must_use]
    pub fn selected_popup_item(&self) -> Option<super::popup::PopupItem> {
        match self.selected_item()? {
            HomeSectionItem::Track(track) => Some(super::popup::PopupItem::Track(track.clone())),
            HomeSectionItem::Playlist(playlist) => {
                Some(super::popup::PopupItem::Playlist(playlist.clone()))
            }
        }
    }

    // -- Navigation (ported from home.py) ----------------------------------

    /// Move the cursor down one item within the active section (Python Down /
    /// `j` on a `NavDataTable`). Clamps at the last item; does not wrap (Textual
    /// tables clamp at their ends).
    pub fn select_next_item(&mut self) {
        let Some(section) = self.sections().get(self.section_idx) else {
            return;
        };
        let last = section.items.len().saturating_sub(1);
        if self.item_idx < last {
            self.item_idx += 1;
        }
    }

    /// Move the cursor up one item within the active section (Up / `k`).
    /// Clamps at the first item.
    pub fn select_previous_item(&mut self) {
        self.item_idx = self.item_idx.saturating_sub(1);
    }

    /// Move focus to the next section, wrapping at the end (Python Tab →
    /// `_focus_adjacent_section(forward=True)`, which does
    /// `(current + 1) % len(focusable)`). The item cursor resets to the top of
    /// the newly focused section, matching a freshly focused Textual table.
    pub fn focus_next_section(&mut self) {
        let count = self.sections().len();
        if count == 0 {
            return;
        }
        self.section_idx = (self.section_idx + 1) % count;
        self.item_idx = 0;
    }

    /// Move focus to the previous section, wrapping at the start (Shift-Tab →
    /// `(current - 1) % len(focusable)` with Python's modulo-of-negative
    /// wrap-around). The item cursor resets to the top.
    pub fn focus_previous_section(&mut self) {
        let count = self.sections().len();
        if count == 0 {
            return;
        }
        // Rust's `%` would underflow on `0 - 1`; add `count` first to mirror
        // Python's `(current_idx - 1) % len` which wraps to `len - 1`.
        self.section_idx = (self.section_idx + count - 1) % count;
        self.item_idx = 0;
    }

    /// Resolve an Enter keypress on the current selection into a [`HomeAction`].
    ///
    /// Returns `None` when nothing is selected (no data, or an empty view).
    /// Mirrors Python's `_handle_item_selection`: a track yields
    /// [`HomeAction::Play`]; a playlist yields [`HomeAction::OpenPlaylist`].
    #[must_use]
    pub fn activate_selected(&self) -> Option<HomeAction> {
        match self.selected_item()? {
            HomeSectionItem::Track(track) => Some(HomeAction::Play(track.clone())),
            HomeSectionItem::Playlist(playlist) => Some(HomeAction::OpenPlaylist(playlist.clone())),
        }
    }

    // -- Rendering ---------------------------------------------------------

    /// Render the home view into `area`.
    ///
    /// In [`PageState::Loading`] / [`PageState::Error`] a single status line is
    /// drawn (the `Loading...` label or the classified error). In
    /// [`PageState::Loaded`] sections appear as cards with the active row highlighted.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let sections = self.sections();
        if sections.is_empty() {
            self.render_status(frame, area, theme);
            return;
        }
        self.render_sections(frame, area, theme, sections);
    }

    /// Draw the loading / error / "no recommendations" status line.
    fn render_status(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        // Loaded-but-empty shows the Python "No recommendations available"
        // message; otherwise the PageState's own status line (Loading / Error).
        let text = match &self.state {
            PageState::Loaded(_) => "No recommendations available",
            other => other.status_line().unwrap_or(""),
        };
        let style = match &self.state {
            PageState::Error(_) => Style::default().fg(theme.primary),
            _ => Style::default().fg(theme.text_muted),
        };
        let paragraph = Paragraph::new(Line::from(Span::styled(text.to_owned(), style)))
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(paragraph, area);
    }

    /// Search-style cards, paged so the focused section always stays visible.
    fn render_sections(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        theme: &Theme,
        sections: &[HomeSection],
    ) {
        let columns = if area.width >= 104
            && detect_orientation(area.width, area.height) == Orientation::Horizontal
        {
            2
        } else {
            1
        };
        let rows = sections
            .len()
            .div_ceil(columns)
            .min(usize::from((area.height / 10).max(1)));
        let page_size = columns * rows;
        let first = self.section_idx / page_size * page_size;
        let row_areas = Layout::vertical(vec![Constraint::Fill(1); rows])
            .spacing(1)
            .split(area);
        for (row_index, row_area) in row_areas.iter().enumerate() {
            let cells = Layout::horizontal(vec![Constraint::Fill(1); columns])
                .spacing(1)
                .split(*row_area);
            for (column_index, cell) in cells.iter().enumerate() {
                let index = first + row_index * columns + column_index;
                if let Some(section) = sections.get(index) {
                    self.render_one_section(
                        frame,
                        *cell,
                        theme,
                        section,
                        index == self.section_idx,
                    );
                }
            }
        }
    }

    /// Use the same border/title/highlight treatment as a Search pane.
    fn render_one_section(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        theme: &Theme,
        section: &HomeSection,
        is_active: bool,
    ) {
        if area.height == 0 {
            return;
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if is_active {
                theme.accent
            } else {
                theme.primary_background
            }))
            .style(Style::default().bg(theme.surface))
            .title(section_title(theme, &section.title, is_active));
        let table_area = block.inner(area);
        frame.render_widget(block, area);

        // Borderless DataTable: Title / Artist / Info / Duration columns,
        // focused styling on the active section only.
        let header = table_header(theme, &HOME_COLUMNS, is_active);
        let rows = section
            .items
            .iter()
            .map(|item| table_row(theme, &item_columns(item), is_active))
            .collect();
        let table = borderless_table(theme, header, rows, home_widths(), is_active);

        let mut state = TableState::default();
        if is_active {
            state.select(Some(self.item_idx));
        }
        frame.render_stateful_widget(table, table_area, &mut state);
    }
}

/// Column labels for a home section table (matching the SVG header row).
const HOME_COLUMNS: [&str; 3] = ["Title", "Artist / Info", "Duration"];

/// Let titles and artists use the available width, keeping duration compact.
fn home_widths() -> Vec<Constraint> {
    vec![
        Constraint::Fill(3),
        Constraint::Fill(2),
        Constraint::Length(10),
    ]
}

/// Format one home item into the three column strings (Title / Artist-Info /
/// Duration). Tracks fill all three; playlists put the track count in the
/// middle "Info" column and leave duration blank.
fn item_columns(item: &HomeSectionItem) -> Vec<String> {
    match item {
        HomeSectionItem::Track(track) => {
            let duration = format_duration(track.duration_seconds);
            let duration = if duration == "—" {
                String::new()
            } else {
                duration
            };
            vec![track.title.clone(), track.artist.clone(), duration]
        }
        HomeSectionItem::Playlist(playlist) => {
            let info = if playlist.track_count > 0 {
                format!("{} tracks", playlist.track_count)
            } else {
                "Playlist".to_owned()
            };
            vec![playlist.title.clone(), info, String::new()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ytmusic_api::{PlaylistInfo, Track};

    // -- fixtures ----------------------------------------------------------

    fn track(id: &str, title: &str, artist: &str) -> HomeSectionItem {
        HomeSectionItem::Track(Track::new(id, title, artist, "", 90.0, ""))
    }

    fn playlist(id: &str, title: &str, count: u32) -> HomeSectionItem {
        HomeSectionItem::Playlist(PlaylistInfo::new(id, title, "", count, ""))
    }

    fn two_section_view() -> HomeView {
        let mut view = HomeView::new();
        view.set_sections(vec![
            HomeSection {
                title: "Quick picks".to_owned(),
                items: vec![
                    track("aaa", "First Song", "Artist A"),
                    track("bbb", "Second Song", "Artist B"),
                ],
            },
            HomeSection {
                title: "Listen again".to_owned(),
                items: vec![playlist("PL1", "My Mix", 25)],
            },
        ]);
        view
    }

    /// Flatten a TestBackend buffer to one string (row text concatenated) so a
    /// test can assert that a substring was rendered somewhere on screen.
    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let width = buffer.area().width as usize;
        let mut out = String::new();
        for (i, cell) in buffer.content().iter().enumerate() {
            out.push_str(cell.symbol());
            if (i + 1) % width == 0 {
                out.push('\n');
            }
        }
        out
    }

    fn render_to_string(view: &HomeView, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        terminal
            .draw(|frame| view.render(frame, frame.area(), &theme))
            .unwrap();
        buffer_text(&terminal)
    }

    // -- set_sections / filtering ------------------------------------------

    #[test]
    fn late_home_section_stays_visible_when_tabbed_to() {
        let mut view = HomeView::new();
        view.set_sections(
            (0..20)
                .map(|i| HomeSection {
                    title: format!("Section {i}"),
                    items: vec![playlist("RDmix", "My Supermix", 20)],
                })
                .collect(),
        );
        for _ in 0..19 {
            view.focus_next_section();
        }
        let text = render_to_string(&view, 80, 12);
        assert!(
            text.contains("Section 19") && text.contains("My Supermix"),
            "selected section not visible: {text}"
        );
    }

    #[test]
    fn home_cards_use_two_columns_when_wide_and_stack_when_narrow() {
        let view = two_section_view();
        let wide = render_to_string(&view, 140, 20);
        let first_line = wide.lines().next().unwrap();
        assert!(first_line.contains("Quick picks") && first_line.contains("Listen again"));
        assert!(first_line.find("Listen again").unwrap() > 70);
        let narrow = render_to_string(&view, 70, 20);
        assert!(!narrow.lines().next().unwrap().contains("Listen again"));
        assert!(narrow.contains("Listen again"));
    }

    #[test]
    fn wider_cards_give_long_titles_more_room() {
        let mut view = two_section_view();
        let title = "A long title that used to be cut off";
        view.set_sections(vec![HomeSection {
            title: "Long titles".into(),
            items: vec![track("v1", title, "Artist")],
        }]);
        assert!(render_to_string(&view, 180, 20).contains(title));
    }

    #[test]
    fn set_sections_drops_empty_sections() {
        let mut view = HomeView::new();
        view.set_sections(vec![
            HomeSection {
                title: "Empty".to_owned(),
                items: vec![],
            },
            HomeSection {
                title: "Has items".to_owned(),
                items: vec![track("x", "Song", "Artist")],
            },
        ]);
        assert_eq!(view.sections().len(), 1);
        assert_eq!(view.sections()[0].title, "Has items");
    }

    #[test]
    fn set_sections_resets_cursor() {
        let mut view = two_section_view();
        view.focus_next_section();
        view.select_next_item();
        // Re-loading puts the cursor back at the origin.
        view.set_sections(vec![HomeSection {
            title: "S".to_owned(),
            items: vec![track("a", "A", "B")],
        }]);
        assert_eq!(view.section_idx, 0);
        assert_eq!(view.item_idx, 0);
    }

    // -- within-section navigation -----------------------------------------

    #[test]
    fn down_moves_within_section_and_clamps_at_end() {
        let mut view = two_section_view();
        // Section 0 has two items.
        view.select_next_item();
        assert_eq!(view.item_idx, 1);
        // Clamp: another Down stays on the last item.
        view.select_next_item();
        assert_eq!(view.item_idx, 1);
    }

    #[test]
    fn up_moves_within_section_and_clamps_at_top() {
        let mut view = two_section_view();
        view.select_next_item();
        view.select_previous_item();
        assert_eq!(view.item_idx, 0);
        // Clamp at the top.
        view.select_previous_item();
        assert_eq!(view.item_idx, 0);
    }

    // -- section navigation (Tab / Shift-Tab wrap — matches Python) ---------

    #[test]
    fn tab_moves_to_next_section() {
        let mut view = two_section_view();
        assert_eq!(view.section_idx, 0);
        view.focus_next_section();
        assert_eq!(view.section_idx, 1);
    }

    #[test]
    fn tab_wraps_from_last_to_first() {
        let mut view = two_section_view();
        view.focus_next_section(); // -> 1
        view.focus_next_section(); // wraps -> 0
        assert_eq!(view.section_idx, 0);
    }

    #[test]
    fn shift_tab_wraps_from_first_to_last() {
        let mut view = two_section_view();
        assert_eq!(view.section_idx, 0);
        view.focus_previous_section(); // wraps -> last (1)
        assert_eq!(view.section_idx, 1);
    }

    #[test]
    fn changing_section_resets_item_cursor() {
        let mut view = two_section_view();
        view.select_next_item();
        assert_eq!(view.item_idx, 1);
        view.focus_next_section();
        assert_eq!(view.item_idx, 0);
    }

    #[test]
    fn navigation_is_a_noop_when_not_loaded() {
        let mut view = HomeView::new(); // Loading, no sections.
        view.focus_next_section();
        view.select_next_item();
        assert_eq!(view.section_idx, 0);
        assert_eq!(view.item_idx, 0);
        assert!(view.selected_item().is_none());
    }

    // -- activation (Enter) ------------------------------------------------

    /// The `video_id` of an [`HomeAction::Play`], for terse id assertions.
    fn played_id(action: Option<HomeAction>) -> Option<String> {
        match action {
            Some(HomeAction::Play(track)) => Some(track.video_id),
            _ => None,
        }
    }

    #[test]
    fn enter_on_track_yields_play_with_video_id() {
        let view = two_section_view();
        assert_eq!(played_id(view.activate_selected()), Some("aaa".to_owned()));
    }

    #[test]
    fn enter_on_playlist_yields_open_playlist() {
        let mut view = two_section_view();
        view.focus_next_section(); // section 1 is the playlist section
        match view.activate_selected() {
            Some(HomeAction::OpenPlaylist(info)) => assert_eq!(info.playlist_id, "PL1"),
            other => panic!("expected OpenPlaylist(PL1), got {other:?}"),
        }
    }

    #[test]
    fn enter_after_moving_selects_the_right_track() {
        let mut view = two_section_view();
        view.select_next_item(); // second track in section 0
        assert_eq!(played_id(view.activate_selected()), Some("bbb".to_owned()));
    }

    // -- rendering (TestBackend) -------------------------------------------

    #[test]
    fn loaded_render_shows_section_titles_and_items() {
        let view = two_section_view();
        let text = render_to_string(&view, 60, 20);
        assert!(
            text.contains("Quick picks"),
            "missing section title:\n{text}"
        );
        assert!(text.contains("Listen again"), "missing 2nd title:\n{text}");
        assert!(text.contains("First Song"), "missing track title:\n{text}");
        assert!(text.contains("Artist A"), "missing artist:\n{text}");
        assert!(text.contains("My Mix"), "missing playlist title:\n{text}");
    }

    #[test]
    fn loaded_render_shows_playlist_track_count() {
        let view = two_section_view();
        let text = render_to_string(&view, 60, 20);
        assert!(text.contains("25 tracks"), "missing track count:\n{text}");
    }

    #[test]
    fn loaded_render_highlights_active_section_selected_row_with_primary() {
        // The borderless DataTable marks the selection with a primary-colored
        // row background (no `▶` glyph, matching the SVG cursor).
        let view = two_section_view();
        let terminal = render_to_terminal(&view, 60, 20);
        let theme = Theme::default();
        let buffer = terminal.backend().buffer();
        let has_primary_row = buffer.content().iter().any(|c| c.bg == theme.primary);
        assert!(
            has_primary_row,
            "active section's selected row not highlighted with primary"
        );
        // And the old `▶` cursor glyph is gone.
        let text = render_to_string(&view, 60, 20);
        assert!(!text.contains('▶'), "stray cursor glyph in table:\n{text}");
    }

    /// Render the view to a TestBackend terminal (for cell-style assertions).
    fn render_to_terminal(view: &HomeView, w: u16, h: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        terminal
            .draw(|frame| view.render(frame, frame.area(), &theme))
            .unwrap();
        terminal
    }

    #[test]
    fn loaded_render_has_search_style_cards_on_surface() {
        let view = two_section_view();
        let terminal = render_to_terminal(&view, 60, 20);
        let theme = Theme::default();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].symbol(), "┌");
        assert_eq!(buffer[(0, 0)].fg, theme.accent);
        // The surface fill is present.
        assert!(
            buffer.content().iter().any(|c| c.bg == theme.surface),
            "home missing surface background"
        );
    }

    #[test]
    fn active_section_header_uses_header_bg() {
        // The focused (first) section's column-header row uses the brighter
        // focused header background; an inactive section uses the dimmer panel
        // background.
        let view = two_section_view();
        let terminal = render_to_terminal(&view, 60, 20);
        let theme = Theme::default();
        let buffer = terminal.backend().buffer();
        // Row 1 is the first section's header (row 0 is its title).
        assert_eq!(
            buffer[(2, 1)].bg,
            theme.header_bg,
            "active section header is not header_bg"
        );
    }

    #[test]
    fn loading_render_shows_loading_label() {
        let view = HomeView::new(); // default Loading state
        let text = render_to_string(&view, 40, 5);
        assert!(
            text.contains("Loading..."),
            "missing loading label:\n{text}"
        );
    }

    #[test]
    fn error_render_shows_classified_message() {
        let mut view = HomeView::new();
        view.set_error("Session expired — run: ytmusic-tui auth");
        let text = render_to_string(&view, 60, 5);
        assert!(
            text.contains("Session expired"),
            "missing error message:\n{text}"
        );
    }

    #[test]
    fn loaded_but_empty_render_shows_no_recommendations() {
        let mut view = HomeView::new();
        view.set_sections(vec![]); // all filtered out -> empty loaded
        let text = render_to_string(&view, 60, 5);
        assert!(
            text.contains("No recommendations available"),
            "missing empty message:\n{text}"
        );
    }
}
