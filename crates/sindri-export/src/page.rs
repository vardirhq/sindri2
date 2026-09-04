//! The page that loads the host.
//!
//! Generated rather than copied, because the two things that differ between one
//! deployment and the next — what the game is called and where it is served
//! from — are the two things a hand-copied page gets wrong.

/// The page, with `{{name}}`, `{{base}}` and `{{build}}` still in it.
pub const PAGE_TEMPLATE: &str = include_str!("page.html");

/// The `wasm-pack` module the site's one host is built as.
///
/// Named here rather than spelled at each call, because the page and the writer
/// have to agree on it and a second spelling is a page that imports a module
/// nobody built.
pub const HOST_MODULE: &str = "sindri_gather";

/// The page for a project, served from `base_path`.
///
/// `base_path` is normalised to start and end with `/`, because
/// `<base href>` means something different without the trailing one: `/repo`
/// resolves `pkg/x.js` against the site root and 404s, while `/repo/` resolves
/// it inside the project. That is the entire GitHub Pages subpath problem, and
/// it is fixed here rather than in a deployment note nobody reads.
#[must_use]
pub fn page_for(name: &str, base_path: &str) -> String {
    page_for_host(name, base_path, HOST_MODULE, "")
}

/// The same page, naming the JavaScript `wasm-pack` produced.
///
/// The module is named after the host crate, which the export cannot know: it
/// reads a project, and a project does not say which binary will run it. So it
/// is a parameter with a default rather than a guess.
/// `build` identifies what was deployed, and is what makes the host cacheable.
///
/// Empty for a local export, which is served fresh anyway and where a changing
/// id in the output would make two exports of one project differ. In CI it is
/// the commit, which is the only thing that answers "is the thing I am looking
/// at the thing I just merged" -- the question that cost three rounds of
/// debugging a fault that was already fixed, because the assets were
/// content-addressed and fresh while the host beside them came from a cache.
#[must_use]
pub fn page_for_host(name: &str, base_path: &str, host_module: &str, build: &str) -> String {
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
    let build = escape(build.trim());
    // No id means no query at all, rather than a bare `?v=`: a URL that carries
    // an empty parameter is a second spelling of the same file, and a cache
    // that has one has not got the other.
    let cachebust = if build.is_empty() {
        String::new()
    } else {
        format!("?v={build}")
    };
    PAGE_TEMPLATE
        .replace("{{name}}", &escape(name))
        .replace("{{base}}", &escape(&base))
        .replace("{{host}}", &escape(host_module))
        .replace("{{cachebust}}", &cachebust)
        .replace("{{build}}", &build)
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
        let page = page_for_host("Game", "/", "my_game", "");
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
        assert!(
            page.contains("!navigator.gpu"),
            "no missing-WebGPU capability guard"
        );
        assert!(page.contains("WebGPU"), "no WebGPU message");
        assert!(page.contains("sindri:failed"), "no failure channel");
    }
}

#[cfg(test)]
mod build_stamp_tests {
    use super::{page_for, page_for_host};

    #[test]
    fn a_build_id_versions_the_host_and_the_wasm_it_loads() {
        // Both, because the query on the module does not reach the file the
        // module fetches: versioning only the JavaScript leaves the actual code
        // cacheable, which is the whole fault this exists to close.
        let page = page_for_host("Orbital", "/sindri2/", "sindri_gather", "a1b2c3d");
        assert!(page.contains("./pkg/sindri_gather.js?v=a1b2c3d"), "{page}");
        assert!(
            page.contains("./pkg/sindri_gather_bg.wasm?v=a1b2c3d"),
            "{page}"
        );
    }

    #[test]
    fn the_build_is_on_the_page_where_somebody_can_read_it_off_a_phone() {
        let page = page_for_host("Orbital", "/", "sindri_gather", "a1b2c3d");
        assert!(page.contains(r#"id="sindri-build">a1b2c3d<"#), "{page}");
    }

    #[test]
    fn no_build_id_leaves_no_query_at_all() {
        // Not a bare `?v=`: that is a second URL for the same file, so a cache
        // holding one has not got the other and the export would be its own
        // cache-miss on every deployment that forgot the flag.
        let page = page_for("Orbital", "/");
        assert!(page.contains("./pkg/sindri_gather.js\""), "{page}");
        assert!(!page.contains("?v="), "{page}");
    }

    #[test]
    fn the_trace_is_off_unless_the_url_asks_for_it() {
        // A diagnostic that showed itself to players would be a worse fault
        // than the one it was added to find.
        let page = page_for("Orbital", "/");
        assert!(page.contains(r#"has("input-debug")"#), "{page}");
        assert!(
            page.contains("#sindri-input {\n      display: none;"),
            "{page}"
        );
    }
}
