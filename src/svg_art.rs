use resvg::{tiny_skia, usvg};

pub const WIDTH: u32 = 260;
pub const HEIGHT: u32 = 140;
pub const MAX_SOURCE_BYTES: usize = 32 * 1024;
const MAX_VIEWBOX_DIMENSION: f32 = 4096.0;

fn parse(source: &str) -> Result<usvg::Tree, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(format!("SVG must be at most {MAX_SOURCE_BYTES} bytes"));
    }
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| format!("invalid SVG: {error}"))?;
    let size = tree.size();
    if size.width() > MAX_VIEWBOX_DIMENSION || size.height() > MAX_VIEWBOX_DIMENSION {
        return Err(format!(
            "SVG viewBox dimensions must not exceed {MAX_VIEWBOX_DIMENSION}"
        ));
    }
    if tree.root().children().is_empty() {
        return Err("SVG must contain vector artwork".into());
    }
    if contains_images(tree.root()) {
        return Err(
            "SVG stickers support vector shapes only; SVG text and images are not allowed".into(),
        );
    }
    Ok(tree)
}

fn contains_images(group: &usvg::Group) -> bool {
    group.children().iter().any(|node| match node {
        usvg::Node::Image(_) | usvg::Node::Text(_) => true,
        usvg::Node::Group(group) => contains_images(group),
        usvg::Node::Path(_) => false,
    })
}

pub fn validate(source: &str) -> Result<(), String> {
    parse(source).map(|_| ())
}

pub fn rasterize(source: &str) -> Result<Vec<u8>, String> {
    let tree = parse(source)?;
    let source_size = tree.size();
    let scale = (WIDTH as f32 / source_size.width()).min(HEIGHT as f32 / source_size.height());
    let tx = (WIDTH as f32 - source_size.width() * scale) / 2.0;
    let ty = (HEIGHT as f32 - source_size.height() * scale) / 2.0;
    let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(tx, ty);
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH, HEIGHT)
        .ok_or_else(|| "could not allocate SVG sticker texture".to_owned())?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Macroquad's normal alpha blending expects straight (not premultiplied) RGB.
    let mut rgba = pixmap.data().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = pixel[3] as u32;
        if alpha == 0 {
            pixel[..3].fill(0);
        } else {
            for channel in &mut pixel[..3] {
                *channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 260 140"><path d="M10 10h240v120H10z" fill="#e55"/></svg>"##;

    #[test]
    fn parses_and_rasterizes_bounded_vector_art() {
        validate(SIMPLE).unwrap();
        validate(&format!("<?xml version=\"1.0\"?>{SIMPLE}")).unwrap();
        let rgba = rasterize(SIMPLE).unwrap();
        assert_eq!(rgba.len(), (WIDTH * HEIGHT * 4) as usize);
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn rejects_malformed_oversized_and_raster_embedded_art() {
        assert!(validate("<svg>").is_err());
        assert!(validate(&format!("<svg>{}</svg>", " ".repeat(MAX_SOURCE_BYTES))).is_err());
        let with_image = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="data:image/png;base64,AAAA" width="10" height="10"/></svg>"##;
        assert!(validate(with_image).is_err());
    }
}
