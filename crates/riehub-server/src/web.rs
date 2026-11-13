//! The embedded web app (web spec, section 1, ADR 0011). The built
//! files under `web/dist` are compiled into the binary. A placeholder
//! `web/dist/index.html` exists so this crate compiles before the
//! frontend build has run, `make web-build` overwrites it with the
//! real Vite output.

use rust_embed::RustEmbed;

/// The build output of `web/`, embedded at compile time.
#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct WebDist;

/// Serves one file of the embedded app. A path that does not match a
/// file falls back to `index.html` so client-side routes work (web
/// spec, section 5). Returns the bytes, the content type and whether
/// the file is a hashed asset that may cache forever.
pub fn serve(path: &str) -> Option<(Vec<u8>, &'static str, bool)> {
    let name = path.trim_start_matches('/');
    let resolved = match WebDist::get(name) {
        Some(file) if !name.is_empty() => (file, name.to_owned()),
        _ => (WebDist::get("index.html")?, "index.html".to_owned()),
    };
    let (file, name) = resolved;
    let immutable = name.starts_with("assets/");
    Some((file.data.to_vec(), content_type(&name), immutable))
}

/// The content type of an embedded file, by extension. The web app is
/// built by Vite, so the set of extensions is small and fixed.
fn content_type(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_html_exists_in_the_embedded_app() {
        assert!(WebDist::get("index.html").is_some());
    }

    #[test]
    fn unknown_path_falls_back_to_index_html() {
        let (bytes, kind, immutable) = serve("/some/client/route").unwrap_or_default();
        assert_eq!(kind, "text/html; charset=utf-8");
        assert!(!immutable);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn asset_paths_are_marked_immutable() {
        // The hashed file names change per build, so the test takes
        // whatever assets the embed holds. Every one of them must
        // cache forever.
        let assets: Vec<String> = WebDist::iter()
            .filter(|name| name.starts_with("assets/"))
            .map(|name| String::from(name.as_ref()))
            .collect();
        assert!(
            !assets.is_empty(),
            "the built app carries hashed assets under assets/"
        );
        for name in assets {
            let (_, _, immutable) = serve(&format!("/{name}")).unwrap_or_default();
            assert!(immutable, "{name} must be cached as immutable");
        }
    }
}
