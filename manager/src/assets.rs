use rocket::http::ContentType;
use rocket::response::{self, Responder};
use rocket::{Request, Response};
use rust_embed::RustEmbed;
use std::io::Cursor;

#[derive(RustEmbed)]
#[folder = "frontend/public/"]
pub struct Assets;

pub struct Asset {
    content: Vec<u8>,
    content_type: ContentType,
}

impl Asset {
    pub fn get(path: &str) -> Option<Self> {
        let file = Assets::get(path)?;
        let content_type = mime_guess::from_path(path)
            .first()
            .map(|mime| {
                let mime_str = mime.to_string();
                match mime_str.as_str() {
                    "text/html" => ContentType::HTML,
                    "text/css" => ContentType::CSS,
                    "application/javascript" => ContentType::JavaScript,
                    "text/javascript" => ContentType::JavaScript,
                    "image/png" => ContentType::PNG,
                    "image/jpeg" => ContentType::JPEG,
                    "image/svg+xml" => ContentType::SVG,
                    _ => ContentType::Binary,
                }
            })
            .unwrap_or(ContentType::Binary);

        Some(Asset {
            content: file.data.to_vec(),
            content_type,
        })
    }
}

impl<'r> Responder<'r, 'static> for Asset {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        Response::build()
            .header(self.content_type)
            .sized_body(self.content.len(), Cursor::new(self.content))
            .ok()
    }
}

#[derive(Responder)]
pub enum AssetResponse {
    #[response(status = 200)]
    Asset(Asset),
    #[response(status = 404)]
    NotFound(()),
}

pub fn get_asset(path: &str) -> AssetResponse {
    match Asset::get(path) {
        Some(asset) => AssetResponse::Asset(asset),
        None => AssetResponse::NotFound(()),
    }
}
