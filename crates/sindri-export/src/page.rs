//! The page that loads the host.
//!
//! Generated rather than copied, because the two things that differ between one
//! deployment and the next — what the game is called and where it is served
//! from — are the two things a hand-copied page gets wrong.

/// The page, with `{{name}}` and `{{base}}` still in it.
pub const PAGE_TEMPLATE: &str = include_str!("page.html");

/// The page for a project, served from `base_path`.
///
/// `base_path` is normalised to start and end with `/`, because
/// `<base href>` means something different without the trailing one: `/repo`
/// resolves `pkg/x.js` against the site root and 404s, while `/repo/` resolves
/// it inside the project. That is the entire GitHub Pages subpath problem, and
/// it is fixed here rather than in a deployment note nobody reads.
#[must_use]
pub fn page_for(name: &str, base_path: &str) -> String {
    page_for_host(name, base_path, "sindri_gather")
}

/// The same page, naming the JavaScript `wasm-pack` produced.
///
/// The module is named after the host crate, which the export cannot know: it
/// reads a project, and a project does not say which binary will run it. So it
/// is a parameter with a default rather than a guess.
#[must_use]
pub fn page_for_host(name: &str, base_path: &str, host_module: &str) -> String {
    let mut base = base_path.trim().to_owned();
    if base.is_empty() {
        base.push('/');
    }
    if !base.starts_with('/') {
        base.insert(0, '/');
    }
    if !base.ends_with('/') {
        base.push('/');
    }
    PAGE_TEMPLATE
        .replace("{{name}}", &escape(name))
        .replace("{{base}}", &escape(&base))
        .replace("{{host}}", &escape(host_module))
}

/// Text that cannot close a tag or an attribute.
///
/// A project's name comes from a file someone edited, and a name with a quote
/// in it should be a name rather than a broken page.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::page_for;

    use super::page_for_host;

    #[test]
    fn the_host_module_is_named_rather_than_guessed() {
        let page = page_for_host("Game", "/", "my_game");
        assert!(page.contains(r#"./pkg/my_game.js"#), "{page}");
        assert!(!page.contains("{{host}}"));
    }

    #[test]
    fn the_name_reaches_the_page() {
        let page = page_for("Orbital Last Stand", "/");
        assert!(page.contains("<title>Orbital Last Stand</title>"), "{page}");
        assert!(!page.contains("{{name}}"));
    }

    /// `<base href="/repo">` resolves `pkg/x.js` against the site root and
    /// 404s. The trailing slash is the whole GitHub Pages subpath problem.
    #[test]
    fn a_subpath_always_ends_in_a_slash() {
        for given in ["/game", "game", "/game/", "game/"] {
            let page = page_for("Game", given);
            assert!(
                page.contains(r#"<base href="/game/">"#),
                "{given} became something else"
            );
        }
    }

    #[test]
    fn no_base_path_is_the_site_root() {
        assert!(page_for("Game", "").contains(r#"<base href="/">"#));
    }

    /// A name with a quote in it should be a name, not a broken page.
    #[test]
    fn a_name_cannot_break_out_of_the_page() {
        let page = page_for(r#"a" onload="steal()"#, "/");
        assert!(!page.contains(r#"onload="steal()"#), "{page}");
        assert!(page.contains("&quot;"));
    }

    /// A player on a browser without WebGPU deserves a sentence, not a blank
    /// canvas.
    #[test]
    fn the_page_says_what_is_missing() {
        let page = page_for("Game", "/");
        assert!(page.contains("WebGPU"), "no WebGPU message");
        assert!(page.contains("sindri:failed"), "no failure channel");
    }
}
