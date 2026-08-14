use js_sys::Uint8Array;
use sindri_core::{AssetId, AssetLoadErrorKind};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

use crate::{AssetBytes, AssetSource, AssetSourceError, AssetSourceFuture};

#[derive(Clone, Debug, Default)]
pub struct FetchAssetSource {
    root: String,
}

impl FetchAssetSource {
    pub fn new(root: impl Into<String>) -> Self {
        let mut root = root.into();
        if !root.is_empty() && !root.ends_with('/') {
            root.push('/');
        }
        Self { root }
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    fn url(&self, id: &AssetId) -> Result<String, AssetSourceError> {
        let mut encoded = Vec::new();
        for segment in id.as_str().split('/') {
            let segment = js_sys::encode_uri_component(segment)
                .as_string()
                .ok_or_else(|| {
                    AssetSourceError::new(
                        id.clone(),
                        self.name(),
                        AssetLoadErrorKind::InvalidData,
                        "could not encode the logical ID as a URL path",
                    )
                })?;
            encoded.push(segment);
        }
        Ok(format!("{}{}", self.root, encoded.join("/")))
    }
}

impl AssetSource for FetchAssetSource {
    fn name(&self) -> &'static str {
        "fetch"
    }

    fn load<'a>(&'a self, id: &'a AssetId) -> AssetSourceFuture<'a> {
        Box::pin(async move {
            let url = self.url(id)?;
            let window = web_sys::window().ok_or_else(|| {
                AssetSourceError::new(
                    id.clone(),
                    self.name(),
                    AssetLoadErrorKind::Network,
                    "browser Window is unavailable",
                )
            })?;
            let response = JsFuture::from(window.fetch_with_str(&url))
                .await
                .map_err(|error| js_error(id, "fetch request failed", &error))?
                .dyn_into::<Response>()
                .map_err(|error| js_error(id, "fetch returned a non-Response value", &error))?;

            if !response.ok() {
                let status = response.status();
                let kind = match status {
                    404 => AssetLoadErrorKind::NotFound,
                    401 | 403 => AssetLoadErrorKind::AccessDenied,
                    _ => AssetLoadErrorKind::Network,
                };
                return Err(AssetSourceError::new(
                    id.clone(),
                    self.name(),
                    kind,
                    format!("HTTP {status} {} for '{url}'", response.status_text()),
                )
                .with_status_code(status));
            }

            let content_type = response.headers().get("content-type").ok().flatten();
            let buffer = response
                .array_buffer()
                .map_err(|error| js_error(id, "could not request the response body", &error))?;
            let buffer = JsFuture::from(buffer)
                .await
                .map_err(|error| js_error(id, "could not read the response body", &error))?;
            let mut asset = AssetBytes::new(id.clone(), Uint8Array::new(&buffer).to_vec());
            if let Some(content_type) = content_type {
                asset = asset.with_content_type(content_type);
            }
            Ok(asset)
        })
    }
}

fn js_error(id: &AssetId, message: &str, value: &wasm_bindgen::JsValue) -> AssetSourceError {
    let detail = value.as_string().unwrap_or_else(|| format!("{value:?}"));
    AssetSourceError::new(
        id.clone(),
        "fetch",
        AssetLoadErrorKind::Network,
        format!("{message}: {detail}"),
    )
}
