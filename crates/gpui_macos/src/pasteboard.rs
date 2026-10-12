use std::ffi::CStr;
use std::path::PathBuf;

use crate::native::*;
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardNameFind, NSPasteboardTypePNG, NSPasteboardTypeString,
    NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSArray, NSData, NSInteger, NSString, NSUInteger};
use smallvec::SmallVec;
use strum::IntoEnumIterator as _;

use crate::ns_string;
use gpui::{
    ClipboardEntry, ClipboardItem, ClipboardString, ExternalPaths, Image, ImageFormat, hash,
};

pub struct Pasteboard {
    inner: Retained<NSPasteboard>,
    text_hash_type: Retained<NSString>,
    metadata_type: Retained<NSString>,
}

impl Pasteboard {
    pub fn general() -> Self {
        Self::new(NSPasteboard::generalPasteboard())
    }

    pub fn find() -> Self {
        Self::new(NSPasteboard::pasteboardWithName(unsafe {
            NSPasteboardNameFind
        }))
    }

    #[cfg(test)]
    pub fn unique() -> Self {
        Self::new(NSPasteboard::pasteboardWithUniqueName())
    }

    fn new(inner: Retained<NSPasteboard>) -> Self {
        Self {
            inner,
            text_hash_type: NSString::from_str("zed-text-hash"),
            metadata_type: NSString::from_str("zed-metadata"),
        }
    }

    pub fn read(&self) -> Option<ClipboardItem> {
        unsafe {
            // Check for file paths first
            let filenames: id =
                msg_send![&*self.inner, propertyListForType: ns_string("NSFilenamesPboardType")];
            if filenames != nil
                && ({
                    let value: NSUInteger = msg_send![filenames, count];
                    value
                }) > 0
            {
                let mut paths = SmallVec::new();
                for file in (&*filenames.cast::<NSArray>())
                    .iter()
                    .map(|file| objc2::rc::Retained::as_ptr(&file).cast_mut())
                {
                    let f: *const libc::c_char = msg_send![file, UTF8String];
                    let path = CStr::from_ptr(f).to_string_lossy().into_owned();
                    paths.push(PathBuf::from(path));
                }
                if !paths.is_empty() {
                    let mut entries = vec![ClipboardEntry::ExternalPaths(ExternalPaths(paths))];

                    // Also include the string representation so text editors can
                    // paste the path as text.
                    if let Some(string_item) = self.read_string_from_pasteboard() {
                        entries.push(string_item);
                    }

                    return Some(ClipboardItem { entries });
                }
            }

            // Next, check for a plain string.
            if let Some(string_entry) = self.read_string_from_pasteboard() {
                return Some(ClipboardItem {
                    entries: vec![string_entry],
                });
            }

            // Finally, try the various supported image types.
            for format in ImageFormat::iter() {
                if let Some(item) = self.read_image(format) {
                    return Some(item);
                }
            }
        }

        None
    }

    fn read_image(&self, format: ImageFormat) -> Option<ClipboardItem> {
        let ut_type: UTType = format.into();
        if !self.inner.types()?.containsObject(ut_type.inner()) {
            return None;
        }
        let bytes = self.data_for_type(ut_type.inner())?;
        let id = hash(&bytes);
        Some(ClipboardItem {
            entries: vec![ClipboardEntry::Image(Image { format, bytes, id })],
        })
    }

    fn read_string_from_pasteboard(&self) -> Option<ClipboardEntry> {
        let string_type = unsafe { NSPasteboardTypeString };
        if !self.inner.types()?.containsObject(string_type) {
            return None;
        }
        let text_bytes = self.data_for_type(string_type)?;
        let text = String::from_utf8_lossy(&text_bytes).into_owned();
        let metadata = self
            .data_for_type(&self.text_hash_type)
            .and_then(|hash_bytes| {
                let hash = u64::from_be_bytes(hash_bytes.try_into().ok()?);
                if hash != ClipboardString::text_hash(&text) {
                    return None;
                }
                String::from_utf8(self.data_for_type(&self.metadata_type)?).ok()
            });
        Some(ClipboardEntry::String(ClipboardString { text, metadata }))
    }

    fn data_for_type(&self, kind: &NSString) -> Option<Vec<u8>> {
        self.inner.dataForType(kind).map(|data| data.to_vec())
    }

    pub fn write(&self, item: ClipboardItem) {
        unsafe {
            match item.entries.as_slice() {
                [] => {
                    // Writing an empty list of entries just clears the clipboard.
                    let _: NSInteger = msg_send![&*self.inner, clearContents];
                }
                [ClipboardEntry::String(string)] => {
                    self.write_plaintext(string);
                }
                [ClipboardEntry::Image(image)] => {
                    self.write_image(image);
                }
                [ClipboardEntry::ExternalPaths(_) | ClipboardEntry::Files(_)] => {}
                _ => {
                    // Agus NB: We're currently only writing string entries to the clipboard when we have more than one.
                    //
                    // This was the existing behavior before I refactored the outer clipboard code:
                    // https://github.com/zed-industries/zed/blob/65f7412a0265552b06ce122655369d6cc7381dd6/crates/gpui/src/platform/mac/platform.rs#L1060-L1110
                    //
                    // Note how `any_images` is always `false`. We should fix that, but that's orthogonal to the refactor.

                    let mut combined = ClipboardString {
                        text: String::new(),
                        metadata: None,
                    };

                    for entry in item.entries {
                        match entry {
                            ClipboardEntry::String(text) => {
                                combined.text.push_str(&text.text());
                                if combined.metadata.is_none() {
                                    combined.metadata = text.metadata;
                                }
                            }
                            _ => {}
                        }
                    }

                    self.write_plaintext(&combined);
                }
            }
        }
    }

    fn write_plaintext(&self, string: &ClipboardString) {
        self.inner.clearContents();
        let text = NSData::with_bytes(string.text.as_bytes());
        self.inner
            .setData_forType(Some(&text), unsafe { NSPasteboardTypeString });
        if let Some(metadata) = string.metadata.as_ref() {
            let hash = NSData::with_bytes(&ClipboardString::text_hash(&string.text).to_be_bytes());
            self.inner
                .setData_forType(Some(&hash), &self.text_hash_type);
            let metadata = NSData::with_bytes(metadata.as_bytes());
            self.inner
                .setData_forType(Some(&metadata), &self.metadata_type);
        }
    }

    fn write_image(&self, image: &Image) {
        self.inner.clearContents();
        let data = NSData::with_bytes(&image.bytes);
        let kind: UTType = image.format.into();
        self.inner.setData_forType(Some(&data), kind.inner());
    }
}

impl From<ImageFormat> for UTType {
    fn from(value: ImageFormat) -> Self {
        match value {
            ImageFormat::Png => Self::png(),
            ImageFormat::Jpeg => Self::jpeg(),
            ImageFormat::Tiff => Self::tiff(),
            ImageFormat::Webp => Self::webp(),
            ImageFormat::Gif => Self::gif(),
            ImageFormat::Bmp => Self::bmp(),
            ImageFormat::Svg => Self::svg(),
            ImageFormat::Ico => Self::ico(),
            ImageFormat::Pnm => Self::pnm(),
        }
    }
}

// See https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/
pub struct UTType(Retained<NSString>);

impl UTType {
    pub fn png() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/png
        Self(unsafe { NSPasteboardTypePNG }.to_owned()) // This is a rare case where there's a built-in NSPasteboardType
    }

    pub fn jpeg() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/jpeg
        Self(NSString::from_str("public.jpeg"))
    }

    pub fn gif() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/gif
        Self(NSString::from_str("com.compuserve.gif"))
    }

    pub fn webp() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/webp
        Self(NSString::from_str("org.webmproject.webp"))
    }

    pub fn bmp() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/bmp
        Self(NSString::from_str("com.microsoft.bmp"))
    }

    pub fn svg() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/svg
        Self(NSString::from_str("public.svg-image"))
    }

    pub fn ico() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/ico
        Self(NSString::from_str("com.microsoft.ico"))
    }

    pub fn tiff() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/tiff
        Self(unsafe { NSPasteboardTypeTIFF }.to_owned()) // This is a rare case where there's a built-in NSPasteboardType
    }

    pub fn pnm() -> Self {
        //https://en.wikipedia.org/w/index.php?title=Netpbm&oldid=1336679433 under Uniform Type Identifier
        Self(NSString::from_str("public.pbm"))
    }

    fn inner(&self) -> &NSString {
        &self.0
    }
}

#[cfg(test)]
mod tests {

    use std::ffi::c_void;

    use gpui::{ClipboardEntry, ClipboardItem, ClipboardString, ImageFormat};

    use super::*;
    use objc2::class;

    #[test]
    fn pasteboard_metadata_survives_autorelease_pools() {
        let pasteboard = objc2::rc::autoreleasepool(|_| Pasteboard::unique());
        let item = ClipboardItem {
            entries: vec![ClipboardEntry::String(ClipboardString {
                text: "retained clipboard".to_string(),
                metadata: Some("metadata".to_string()),
            })],
        };
        objc2::rc::autoreleasepool(|_| pasteboard.write(item.clone()));
        objc2::rc::autoreleasepool(|_| assert_eq!(pasteboard.read(), Some(item)));
    }

    unsafe fn simulate_external_file_copy(pasteboard: &Pasteboard, paths: &[&str]) {
        unsafe {
            let ns_paths: Vec<id> = paths.iter().map(|p| ns_string(p)).collect();
            let ns_array = crate::native::ns_array(&ns_paths);

            let mut types = vec![ns_string("NSFilenamesPboardType")];
            types.push(
                (NSPasteboardTypeString as *const NSString)
                    .cast_mut()
                    .cast(),
            );

            let types_array = crate::native::ns_array(&types);
            let _: NSInteger = msg_send![&*pasteboard.inner, declareTypes: types_array, owner: nil];

            let _: BOOL = msg_send![&*pasteboard.inner, setPropertyList: ns_array, forType: ns_string("NSFilenamesPboardType")];

            let joined = paths.join("\n");
            let bytes: id = msg_send![class!(NSData), dataWithBytes: joined.as_ptr() as *const c_void, length: joined.len() as u64];
            let _: BOOL =
                msg_send![&*pasteboard.inner, setData: bytes, forType: NSPasteboardTypeString];
        }
    }

    #[test]
    fn test_string() {
        let pasteboard = Pasteboard::unique();
        assert_eq!(pasteboard.read(), None);

        let item = ClipboardItem::new_string("1".to_string());
        pasteboard.write(item.clone());
        assert_eq!(pasteboard.read(), Some(item));

        let item = ClipboardItem {
            entries: vec![ClipboardEntry::String(
                ClipboardString::new("2".to_string()).with_json_metadata(vec![3, 4]),
            )],
        };
        pasteboard.write(item.clone());
        assert_eq!(pasteboard.read(), Some(item));

        let text_from_other_app = "text from other app";
        unsafe {
            let bytes: id = msg_send![class!(NSData), dataWithBytes: text_from_other_app.as_ptr() as *const c_void, length: text_from_other_app.len() as u64];
            let _: BOOL =
                msg_send![&*pasteboard.inner, setData: bytes, forType: NSPasteboardTypeString];
        }
        assert_eq!(
            pasteboard.read(),
            Some(ClipboardItem::new_string(text_from_other_app.to_string()))
        );
    }

    #[test]
    fn test_read_external_path() {
        let pasteboard = Pasteboard::unique();

        unsafe {
            simulate_external_file_copy(&pasteboard, &["/test.txt"]);
        }

        let item = pasteboard.read().expect("should read clipboard item");

        // Test both ExternalPaths and String entries exist
        assert_eq!(item.entries.len(), 2);

        // Test first entry is ExternalPaths
        match &item.entries[0] {
            ClipboardEntry::ExternalPaths(ep) => {
                assert_eq!(ep.paths(), &[PathBuf::from("/test.txt")]);
            }
            other => panic!("expected ExternalPaths, got {:?}", other),
        }

        // Test second entry is String
        match &item.entries[1] {
            ClipboardEntry::String(s) => {
                assert_eq!(s.text(), "/test.txt");
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn test_read_external_paths_with_spaces() {
        let pasteboard = Pasteboard::unique();
        let paths = ["/some file with spaces.txt"];

        unsafe {
            simulate_external_file_copy(&pasteboard, &paths);
        }

        let item = pasteboard.read().expect("should read clipboard item");

        match &item.entries[0] {
            ClipboardEntry::ExternalPaths(ep) => {
                assert_eq!(ep.paths(), &[PathBuf::from("/some file with spaces.txt")]);
            }
            other => panic!("expected ExternalPaths, got {:?}", other),
        }
    }

    #[test]
    fn test_read_multiple_external_paths() {
        let pasteboard = Pasteboard::unique();
        let paths = ["/file.txt", "/image.png"];

        unsafe {
            simulate_external_file_copy(&pasteboard, &paths);
        }

        let item = pasteboard.read().expect("should read clipboard item");
        assert_eq!(item.entries.len(), 2);

        // Test both ExternalPaths and String entries exist
        match &item.entries[0] {
            ClipboardEntry::ExternalPaths(ep) => {
                assert_eq!(
                    ep.paths(),
                    &[PathBuf::from("/file.txt"), PathBuf::from("/image.png"),]
                );
            }
            other => panic!("expected ExternalPaths, got {:?}", other),
        }

        match &item.entries[1] {
            ClipboardEntry::String(s) => {
                assert_eq!(s.text(), "/file.txt\n/image.png");
                assert_eq!(s.metadata, None);
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn test_read_image() {
        let pasteboard = Pasteboard::unique();

        // Smallest valid PNG: 1x1 transparent pixel
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x62, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE5, 0x27, 0xDE, 0xFC, 0x00, 0x00,
            0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        unsafe {
            let ns_png_type = (NSPasteboardTypePNG as *const NSString).cast_mut().cast();
            let types_array = crate::native::ns_array(&[ns_png_type]);
            let _: NSInteger = msg_send![&*pasteboard.inner, declareTypes: types_array, owner: nil];

            let data: id = msg_send![class!(NSData), dataWithBytes: png_bytes.as_ptr() as *const c_void, length: png_bytes.len() as u64];
            let _: BOOL = msg_send![&*pasteboard.inner, setData: data, forType: ns_png_type];
        }

        let item = pasteboard.read().expect("should read PNG image");

        // Test Image entry exists
        assert_eq!(item.entries.len(), 1);
        match &item.entries[0] {
            ClipboardEntry::Image(img) => {
                assert_eq!(img.format, ImageFormat::Png);
                assert_eq!(img.bytes, png_bytes);
            }
            other => panic!("expected Image, got {:?}", other),
        }
    }
}
