use anyhow::{Result, ensure};
use image::{DynamicImage, ImageFormat, ImageReader, Limits, RgbaImage};
use std::{io::Cursor, sync::Arc};

const MAX_INPUT: usize = 16 * 1024 * 1024;
const MAX_EDGE: u32 = 4096;
const COVER_EDGE: u32 = 512;

/// An immutable, shared cover prepared for system media controls.
/// Prepare on a worker: decoding, resizing and desktop temporary-file I/O are synchronous.
#[derive(Clone)]
pub struct MediaArtwork(Arc<ArtworkData>);

struct ArtworkData {
    png: Vec<u8>,
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
    _file: tempfile::NamedTempFile,
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
    file_url: String,
}

impl MediaArtwork {
    /// Decodes PNG or JPEG and fits it within 512 x 512 pixels without upscaling.
    /// Inputs are limited to 16 MiB encoded, 4096 pixels per edge and 64 MiB decoded.
    pub fn from_encoded(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= MAX_INPUT, "media artwork exceeds 16 MiB");
        let format = image::guess_format(bytes)?;
        ensure!(
            matches!(format, ImageFormat::Png | ImageFormat::Jpeg),
            "media artwork must be PNG or JPEG"
        );
        let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_EDGE);
        limits.max_image_height = Some(MAX_EDGE);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        Self::from_image(reader.decode()?)
    }

    /// Prepares tightly packed RGBA8 pixels, for example an application-extracted video frame.
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        ensure!(
            width > 0 && height > 0 && width <= MAX_EDGE && height <= MAX_EDGE,
            "invalid media artwork dimensions"
        );
        ensure!(
            pixels.len() == width as usize * height as usize * 4,
            "invalid media artwork RGBA length"
        );
        let image = RgbaImage::from_raw(width, height, pixels).expect("validated RGBA length");
        Self::from_image(DynamicImage::ImageRgba8(image))
    }

    fn from_image(image: DynamicImage) -> Result<Self> {
        ensure!(
            image.width() > 0 && image.height() > 0,
            "empty media artwork"
        );
        let image = if image.width() > COVER_EDGE || image.height() > COVER_EDGE {
            image.thumbnail(COVER_EDGE, COVER_EDGE)
        } else {
            image
        };
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image.into_rgba8()).write_to(&mut png, ImageFormat::Png)?;
        let png = png.into_inner();
        #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
        let (file, file_url) = {
            use std::io::Write;
            let mut file = tempfile::Builder::new()
                .prefix("gpui-media-")
                .suffix(".png")
                .tempfile()?;
            file.write_all(&png)?;
            file.flush()?;
            let url = url::Url::from_file_path(file.path())
                .map_err(|_| anyhow::anyhow!("invalid media artwork path"))?;
            (file, url.to_string())
        };
        Ok(Self(Arc::new(ArtworkData {
            png,
            #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
            _file: file,
            #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
            file_url,
        })))
    }

    /// Normalized PNG data for platform media bridges.
    pub fn png(&self) -> &[u8] {
        &self.0.png
    }

    #[cfg(any(target_os = "android", target_family = "wasm"))]
    pub fn png_base64(&self) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(self.png())
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
    pub(super) fn file_url(&self) -> &str {
        &self.0.file_url
    }
}
impl PartialEq for MediaArtwork {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.png() == other.png()
    }
}
impl Eq for MediaArtwork {}
impl std::fmt::Debug for MediaArtwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaArtwork")
            .field("png_bytes", &self.png().len())
            .finish_non_exhaustive()
    }
}
