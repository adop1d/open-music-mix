//! Fixture-driven resolver tests: the yt-dlp fixture stands in for the real binary.

use std::time::Duration;

use omm::process::{RealResolver, ResolveError, Resolver};
use omm::queue::Locator;

fn fixture() -> RealResolver {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/yt-dlp");
    RealResolver::with_program(path)
}

fn temp(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("omm-resolver-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn fixture_resolves_a_url() {
    let resolved = fixture()
        .resolve(&Locator::YoutubeId("dQw4w9WgXcQ".to_string()))
        .unwrap();
    assert_eq!(resolved.url, "https://media.invalid/audio/dQw4w9WgXcQ");
    assert_eq!(resolved.video_id.as_deref(), Some("dQw4w9WgXcQ"));
}

#[test]
fn fixture_resolves_a_query() {
    let resolved = fixture()
        .resolve(&Locator::YoutubeQuery("the cure lullaby".to_string()))
        .unwrap();
    assert_eq!(resolved.url, "https://media.invalid/audio/dQw4w9WgXcQ");
    assert_eq!(resolved.video_id, None);
}

#[test]
fn fixture_reports_no_results() {
    let error = fixture()
        .resolve(&Locator::YoutubeQuery("missing song".to_string()))
        .unwrap_err();
    assert!(matches!(error, ResolveError::NoResults(_)), "{error:?}");
}

#[test]
fn fixture_reports_unreachable() {
    let error = fixture()
        .resolve(&Locator::YoutubeQuery("unreachable host".to_string()))
        .unwrap_err();
    assert!(matches!(error, ResolveError::Unreachable(_)), "{error:?}");
}

#[test]
fn fixture_reports_missing_binary() {
    let resolver = RealResolver::with_program("/nonexistent/omm-yt-dlp");
    let error = resolver
        .resolve(&Locator::YoutubeQuery("anything".to_string()))
        .unwrap_err();
    assert!(matches!(error, ResolveError::MissingBinary(_)), "{error:?}");
}

#[test]
fn fixture_finds_a_candidate() {
    let candidate = fixture().search_candidate("plainsong").unwrap();
    assert_eq!(candidate.video_id, "dQw4w9WgXcQ");
    assert_eq!(candidate.duration, Some(Duration::from_secs(213)));
}

#[test]
fn fixture_writes_a_thumbnail() {
    let directory = temp("thumb");
    let bytes = fixture()
        .thumbnail("dQw4w9WgXcQ", &directory)
        .expect("thumbnail bytes");
    assert_eq!(bytes, b"fakejpg");
    let _ = std::fs::remove_dir_all(&directory);
}
