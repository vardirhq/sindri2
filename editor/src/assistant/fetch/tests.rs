//! What acquisition must guarantee, tested without a network.

use super::*;

/// The SHA-256 of the empty input, which is the one digest worth hardcoding:
/// if this is wrong, the hashing is wrong and every other assertion here is
/// checking one bug against another.
const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("sindri-fetch-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a scratch directory");
    directory
}

fn asset(sha256: &str) -> Asset {
    Asset {
        id: "thing".to_owned(),
        version: "1".to_owned(),
        platform: "any".to_owned(),
        architecture: "any".to_owned(),
        url: "https://example.invalid/thing".to_owned(),
        sha256: sha256.to_owned(),
        size: 0,
        display_name: "Thing".to_owned(),
        description: "A thing.".to_owned(),
        license: "MIT".to_owned(),
        source: "https://example.invalid".to_owned(),
    }
}

/// Writes `bytes` where a real downloader would have.
fn writes(bytes: &'static [u8]) -> impl FnOnce(&str, &Path) -> Result<(), Trouble> {
    move |_, into| {
        fs::write(into, bytes)?;
        Ok(())
    }
}

#[test]
fn hashing_matches_a_known_digest() {
    let directory = scratch("digest");
    let file = directory.join("empty");
    fs::write(&file, b"").expect("written");
    assert_eq!(digest(&file).expect("hashed"), EMPTY);
}

#[test]
fn a_file_that_matches_verifies_and_one_that_does_not_fails() {
    let directory = scratch("verify");
    let file = directory.join("empty");
    fs::write(&file, b"").expect("written");
    assert!(verified(&file, EMPTY));
    assert!(
        verified(&file, &EMPTY.to_uppercase()),
        "case is not meaningful"
    );
    assert!(!verified(&file, &"0".repeat(64)));
    assert!(
        !verified(&directory.join("absent"), EMPTY),
        "a missing file verifies as nothing"
    );
}

/// Re-running setup has to be free, or nobody re-runs it.
#[test]
fn an_asset_already_on_disk_is_reused_without_fetching() {
    let directory = scratch("reuse");
    let destination = directory.join("thing");
    fs::write(&destination, b"").expect("written");
    let outcome = fetch(&asset(EMPTY), &destination, |_, _| {
        panic!("nothing should be downloaded when the file is already right");
    })
    .expect("reused");
    assert_eq!(outcome, Outcome::Reused);
}

#[test]
fn a_fetched_asset_that_matches_lands_at_its_name() {
    let directory = scratch("fetch");
    let destination = directory.join("thing");
    let outcome = fetch(&asset(EMPTY), &destination, writes(b"")).expect("fetched");
    assert_eq!(outcome, Outcome::Fetched);
    assert!(destination.is_file());
    assert!(verified(&destination, EMPTY));
}

/// The interesting failure. A captive portal, a truncated transfer and a
/// tampered mirror all look like a successful download to the tool that
/// performed it, which is why success is not the test.
#[test]
fn a_fetched_asset_that_does_not_match_is_refused() {
    let directory = scratch("corrupt");
    let destination = directory.join("thing");
    let trouble = fetch(
        &asset(EMPTY),
        &destination,
        writes(b"not what was asked for"),
    )
    .expect_err("a mismatch is refused");
    assert!(matches!(trouble, Trouble::Corrupt { .. }));
    assert!(
        !destination.exists(),
        "a wrong file must never reach the name everything else looks for"
    );
}

/// And the part file goes too, because resuming from wrong bytes only ever
/// produces more wrong bytes.
#[test]
fn a_refused_download_leaves_nothing_behind_to_resume_from() {
    let directory = scratch("residue");
    let destination = directory.join("thing");
    let _ = fetch(&asset(EMPTY), &destination, writes(b"wrong"));
    let leftovers: Vec<_> = fs::read_dir(&directory)
        .expect("readable")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    assert!(leftovers.is_empty(), "left {leftovers:?} behind");
}

/// The destination only ever comes into existence by a rename, after the hash
/// matched, so there is no window in which a half-written file sits at it.
#[test]
fn the_destination_does_not_exist_while_the_download_is_in_flight() {
    let directory = scratch("window");
    let destination = directory.join("thing");
    let watched = destination.clone();
    let outcome = fetch(&asset(EMPTY), &destination, move |_, into| {
        assert!(
            !watched.exists(),
            "the final name must not exist while bytes are still arriving"
        );
        fs::write(into, b"")?;
        Ok(())
    })
    .expect("fetched");
    assert_eq!(outcome, Outcome::Fetched);
}

#[test]
fn both_downloaders_are_asked_to_resume_and_to_write_where_told() {
    let into = Path::new("/tmp/example.bin");
    for tool in [Downloader::Curl, Downloader::Wget] {
        let arguments = tool.arguments("https://example.invalid/x", into);
        assert!(
            arguments
                .iter()
                .any(|argument| argument.contains("continue")),
            "{tool:?} does not resume"
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument.contains("example.bin")),
            "{tool:?} does not write where it was told"
        );
        assert_eq!(
            arguments.last().map(String::as_str),
            Some("https://example.invalid/x"),
            "{tool:?} should end with the URL"
        );
    }
}

/// curl exits zero on an HTTP error page unless told otherwise, which would
/// hand a 404 body to the hash check and report it as corruption rather than
/// as a missing file.
#[test]
fn curl_is_told_to_fail_on_an_http_error() {
    let arguments = Downloader::Curl.arguments("https://example.invalid/x", Path::new("/tmp/x"));
    assert!(arguments.iter().any(|argument| argument == "--fail"));
    assert!(arguments.iter().any(|argument| argument == "--location"));
}

#[test]
fn an_asset_suits_only_the_machine_it_was_published_for() {
    let entry = Asset {
        platform: "linux".to_owned(),
        architecture: "x86_64".to_owned(),
        ..asset(EMPTY)
    };
    assert!(entry.suits("linux", "x86_64"));
    assert!(!entry.suits("macos", "arm64"));
    assert!(!entry.suits("linux", "arm64"));
}

#[test]
fn an_any_asset_suits_every_machine() {
    let entry = asset(EMPTY);
    assert!(entry.suits("linux", "x86_64"));
    assert!(entry.suits("macos", "arm64"));
    assert!(entry.suits("windows", "x86_64"));
}

#[test]
fn this_machine_is_described_in_the_manifest_vocabulary() {
    assert!(["linux", "macos", "windows"].contains(&platform()));
    assert!(["x86_64", "arm64"].contains(&architecture()));
}
