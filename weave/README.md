# Weave

Experimental declarative UI styling for Sindri.

This workspace is intentionally independent from Sindri. It owns only the language surface: parsing, selectors, declarations, media conditions, diagnostics, and eventually language tooling. The Sindri integration lives in `crates/sindri-weave` in the parent repository workspace.

The proof of concept is deliberately small and should not be treated as a compatibility promise.
