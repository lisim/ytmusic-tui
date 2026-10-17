use ratatui::{Terminal, backend::TestBackend};
use ytmusic_api::{HomeSection, HomeSectionItem, PlaylistInfo, Track};
use ytmusic_tui::views::{Theme, home::HomeView};

fn main() {
    let track = |title: &str, artist: &str| {
        HomeSectionItem::Track(Track::new("demo", title, artist, "", 210.0, ""))
    };
    let playlist =
        |title: &str| HomeSectionItem::Playlist(PlaylistInfo::new("demo", title, "", 100, ""));
    let mut view = HomeView::new();
    view.set_sections(vec![
        HomeSection {
            title: "Quick picks".into(),
            items: vec![
                track(
                    "A favourite song with room for its full title",
                    "Favourite Artist",
                ),
                track("Something new to discover", "Another Artist"),
                track("The long way home", "A familiar band"),
            ],
        },
        HomeSection {
            title: "Mixed for you".into(),
            items: vec![
                playlist("My Supermix"),
                playlist("Your personalised rock mix"),
                playlist("Your personalised electronic mix"),
            ],
        },
        HomeSection {
            title: "From your library".into(),
            items: vec![
                playlist("Liked music"),
                playlist("Road trip favourites"),
                playlist("Weekend playlist"),
            ],
        },
        HomeSection {
            title: "Discover something new".into(),
            items: vec![playlist("Discover Mix"), playlist("New Release Mix")],
        },
        HomeSection {
            title: "Listen again".into(),
            items: vec![
                track("A song you keep coming back to", "Favourite Artist"),
                track("One more time", "Another Artist"),
            ],
        },
        HomeSection {
            title: "Fresh finds, old favourites".into(),
            items: vec![playlist("Replay Mix"), playlist("Archive Mix")],
        },
    ]);
    let mut terminal = Terminal::new(TestBackend::new(160, 36)).unwrap();
    terminal
        .draw(|frame| view.render(frame, frame.area(), &Theme::from_name("terminal")))
        .unwrap();
    let cells:Vec<_>=terminal.backend().buffer().content().iter().map(|cell|serde_json::json!({"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg)})).collect();
    println!(
        "{}",
        serde_json::json!({"width":160,"height":36,"cells":cells})
    );
}
