use axum::{
    body::Body,
    extract::State,
    http::{StatusCode, header},
    response::Response,
};
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use std::sync::{Arc, RwLock};

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;

pub type SharedSnapshot = Arc<RwLock<Option<Vec<u8>>>>;

pub fn shared() -> SharedSnapshot {
    Arc::new(RwLock::new(None))
}

pub fn publish(snapshot: &SharedSnapshot, rgba: Vec<u8>) {
    *snapshot.write().expect("snapshot lock") = Some(rgba);
}

pub async fn get(State(snapshot): State<SharedSnapshot>) -> Result<Response, (StatusCode, String)> {
    let rgba = snapshot
        .read()
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "snapshot unavailable".into(),
            )
        })?
        .clone()
        .ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "no rendered frame available yet".into(),
        ))?;
    if rgba.len() != WIDTH as usize * HEIGHT as usize * 4 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "cached frame has an invalid size".into(),
        ));
    }

    let png = encode_png(&rgba).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not encode snapshot: {error}"),
        )
    })?;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(png))
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not return snapshot: {error}"),
            )
        })
}

fn encode_png(rgba: &[u8]) -> image::ImageResult<Vec<u8>> {
    let row_len = WIDTH as usize * 4;
    let mut top_down = Vec::with_capacity(rgba.len());
    for row in rgba.chunks_exact(row_len).rev() {
        top_down.extend_from_slice(row);
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(&top_down, WIDTH, HEIGHT, ColorType::Rgba8)?;
    Ok(png)
}

pub fn routes(snapshot: SharedSnapshot) -> axum::Router {
    axum::Router::new()
        .route("/api/snapshot", axum::routing::get(get))
        .with_state(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_snapshot_flips_gl_readback_into_video_orientation() {
        let mut rgba = vec![0; WIDTH as usize * HEIGHT as usize * 4];
        let row = WIDTH as usize * 4;
        rgba[..4].copy_from_slice(&[255, 0, 0, 255]);
        rgba[(HEIGHT as usize - 1) * row..(HEIGHT as usize - 1) * row + 4]
            .copy_from_slice(&[0, 0, 255, 255]);
        let png = encode_png(&rgba).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(decoded.get_pixel(0, HEIGHT - 1).0, [255, 0, 0, 255]);
    }

    #[tokio::test]
    async fn snapshot_is_unavailable_until_a_frame_is_published() {
        let result = get(State(shared())).await;
        assert!(matches!(result, Err((StatusCode::SERVICE_UNAVAILABLE, _))));
    }
}
