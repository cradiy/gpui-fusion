use core_foundation::base::TCFType;
use core_graphics::font::CGFont;
use core_text::font::CTFont;
use font_kit::font::Font;
use foreign_types::ForeignType;

pub(crate) fn native_font(font: &Font) -> CTFont {
    let native = font.native_font();
    // SAFETY: Both bindings wrap CTFontRef. Retain it independently of font-kit.
    unsafe { CTFont::wrap_under_get_rule(native.as_concrete_TypeRef().cast()) }
}

pub(crate) fn from_native_font(font: &CTFont) -> Font {
    // SAFETY: The retained font remains alive while font-kit clones its handle.
    unsafe {
        let native = font_kit::loaders::core_text::NativeFont::wrap_under_get_rule(
            font.as_concrete_TypeRef().cast(),
        );
        Font::from_native_font(&native)
    }
}

pub(crate) fn from_graphics_font(font: CGFont) -> Font {
    // SAFETY: Both bindings own a CGFontRef. Transfer its existing retain to
    // font-kit's binding without copying or releasing the native font.
    Font::from_core_graphics_font(unsafe { ForeignType::from_ptr(font.into_ptr().cast()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_handles_survive_binding_and_owner_changes() {
        let original = core_text::font::new_from_name("Helvetica", 16.).unwrap();
        let name = original.postscript_name();
        let font = from_native_font(&original);
        drop(original);
        assert!(font.glyph_for_char('A').is_some());

        let native = native_font(&font);
        drop(font);
        assert_eq!(native.postscript_name(), name);

        let font = from_graphics_font(native.copy_to_CGFont());
        drop(native);
        let glyph = font.glyph_for_char('A').unwrap();
        assert!(font.advance(glyph).unwrap().x() > 0.);
        assert_eq!(native_font(&font).postscript_name(), name);
    }
}
