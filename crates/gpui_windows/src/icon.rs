use anyhow::{Context as _, Result, ensure};
use std::{ffi::c_void, mem, ptr};
use windows::Win32::{
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject,
        HBITMAP, HGDIOBJ, RGBQUAD,
    },
    UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO},
};

pub(crate) struct OwnedIcon(pub HICON);

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        let _ = unsafe { DestroyIcon(self.0) };
    }
}

struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        let _ = unsafe { DeleteObject(HGDIOBJ(self.0.0)) };
    }
}

struct IconPixels {
    color: Vec<u8>,
    mask: Vec<u8>,
}

impl IconPixels {
    fn new(width: u32, height: u32, rgba: &[u8]) -> Result<Self> {
        ensure!(width > 0 && height > 0, "icon dimensions must be nonzero");
        let length = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .context("icon dimensions overflow")?;
        ensure!(rgba.len() >= length, "truncated icon pixel buffer");
        let row_bytes = (width as usize).div_ceil(32) * 4;
        let mut mask = vec![0; row_bytes * height as usize];
        let mut color = Vec::with_capacity(length);
        for (index, pixel) in rgba[..length].as_chunks::<4>().0.iter().enumerate() {
            let alpha = u16::from(pixel[3]);
            let premultiply = |channel: u8| ((u16::from(channel) * alpha + 127) / 255) as u8;
            color.extend_from_slice(&[
                premultiply(pixel[2]),
                premultiply(pixel[1]),
                premultiply(pixel[0]),
                pixel[3],
            ]);
            if alpha == 0 {
                let x = index % width as usize;
                let y = index / width as usize;
                mask[y * row_bytes + x / 8] |= 0x80 >> (x % 8);
            }
        }
        Ok(Self { color, mask })
    }
}

pub(crate) fn create_icon(width: u32, height: u32, rgba: &[u8]) -> Result<HICON> {
    let dib_width = i32::try_from(width).context("icon width exceeds GDI limits")?;
    let dib_height = i32::try_from(height).context("icon height exceeds GDI limits")?;
    let pixels = IconPixels::new(width, height, rgba)?;
    let header = BITMAPINFOHEADER {
        biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: dib_width,
        biHeight: -dib_height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let color_info = BITMAPINFO {
        bmiHeader: header,
        ..Default::default()
    };
    let mut color_bits: *mut c_void = ptr::null_mut();
    let color = Bitmap(
        unsafe { CreateDIBSection(None, &color_info, DIB_RGB_COLORS, &mut color_bits, None, 0) }
            .context("creating icon color bitmap")?,
    );
    ensure!(
        !color_bits.is_null(),
        "icon color bitmap has no pixel buffer"
    );
    unsafe {
        ptr::copy_nonoverlapping(pixels.color.as_ptr(), color_bits.cast(), pixels.color.len())
    };

    // A monochrome DIB requires both palette entries after its header.
    #[repr(C)]
    struct MaskInfo {
        header: BITMAPINFOHEADER,
        colors: [RGBQUAD; 2],
    }
    let mask_info = MaskInfo {
        header: BITMAPINFOHEADER {
            biBitCount: 1,
            ..header
        },
        colors: [
            RGBQUAD::default(),
            RGBQUAD {
                rgbBlue: 255,
                rgbGreen: 255,
                rgbRed: 255,
                rgbReserved: 0,
            },
        ],
    };
    let mut mask_bits: *mut c_void = ptr::null_mut();
    let mask = Bitmap(
        unsafe {
            CreateDIBSection(
                None,
                ptr::from_ref(&mask_info).cast(),
                DIB_RGB_COLORS,
                &mut mask_bits,
                None,
                0,
            )
        }
        .context("creating icon mask bitmap")?,
    );
    ensure!(!mask_bits.is_null(), "icon mask bitmap has no pixel buffer");
    unsafe { ptr::copy_nonoverlapping(pixels.mask.as_ptr(), mask_bits.cast(), pixels.mask.len()) };
    unsafe {
        CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            hbmMask: mask.0,
            hbmColor: color.0,
            ..Default::default()
        })
    }
    .context("creating icon")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_pixels_preserve_transparency_and_align_mask_rows() {
        let width = 35usize;
        let mut rgba = vec![255; width * 2 * 4];
        for index in [0, 34, 36] {
            rgba[index * 4 + 3] = 0;
        }
        rgba[4..8].copy_from_slice(&[200, 100, 50, 128]);
        let pixels = IconPixels::new(width as u32, 2, &rgba).unwrap();
        assert_eq!(&pixels.color[..8], &[0, 0, 0, 0, 25, 50, 100, 128]);
        assert_eq!(
            pixels.mask,
            [128, 0, 0, 0, 32, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0]
        );
        let transparent = IconPixels::new(2, 2, &[0; 16]).unwrap();
        assert_eq!(transparent.mask, [192, 0, 0, 0, 192, 0, 0, 0]);
    }
}
