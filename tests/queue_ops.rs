//! Direct queue mutation tests (task 3.1).

use omm::queue::{Locator, Queue, Source, TrackMeta};

mod common;

fn meta(title: &str) -> TrackMeta {
    TrackMeta::new(title, "The Cure")
}

fn locator(title: &str) -> Locator {
    Locator::MpdUri(format!("{title}.flac"))
}

#[test]
fn queue_mutations_append_insert_reorder_and_clear() {
    let mut queue = Queue::new();
    let a = queue.append(
        Source::Local,
        meta("A"),
        Locator::MpdUri("a.flac".to_string()),
    );
    let b = queue.append(
        Source::Youtube,
        meta("B"),
        Locator::YoutubeId("vid00000001".to_string()),
    );
    assert_eq!(queue.len(), 2);
    assert_ne!(a, b);

    queue.insert(
        1,
        Source::Spotify,
        meta("C"),
        Locator::SpotifyTrack("c".to_string()),
    );
    let titles: Vec<_> = queue.entries.iter().map(|entry| entry.title()).collect();
    assert_eq!(titles, ["A", "C", "B"]);

    assert!(queue.move_entry(2, 0));
    let titles: Vec<_> = queue.entries.iter().map(|entry| entry.title()).collect();
    assert_eq!(titles, ["B", "A", "C"]);
    assert!(!queue.move_entry(0, 9));

    queue.set_position(1);
    let removed = queue.remove(1);
    assert_eq!(removed.removed.as_ref().unwrap().title(), "A");
    assert!(removed.was_current);
    assert_eq!(removed.next_position, Some(1));

    queue.clear();
    assert!(queue.is_empty());
    assert_eq!(queue.position, None);
    assert_eq!(queue.selection, 0);
}

#[test]
fn filtering_is_case_insensitive_and_ordered() {
    let mut queue = Queue::new();
    for (title, artist) in [
        ("Friday I'm in Love", "The Cure"),
        ("Bohemian Rhapsody", "Queen"),
        ("Lullaby", "The Cure"),
    ] {
        queue.append(
            Source::Local,
            TrackMeta::new(title, artist),
            Locator::MpdUri(format!("{title}.flac")),
        );
    }
    assert_eq!(queue.filtered_indices("cure"), vec![0, 2]);
    assert_eq!(queue.filtered_indices("QUEEN"), vec![1]);
    assert!(queue.filtered_indices("nothing").is_empty());
}

#[test]
fn remove_outcomes_cover_missing_and_non_current() {
    let mut queue = Queue::new();
    queue.append(Source::Local, meta("A"), locator("a"));
    queue.append(Source::Local, meta("B"), locator("b"));
    queue.set_position(0);
    let first = queue.remove(0);
    assert!(first.was_current);
    assert_eq!(first.next_position, Some(0));
    let missing = queue.remove(5);
    assert!(missing.removed.is_none());
    assert!(!missing.was_current);
}
