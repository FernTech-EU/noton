//! Carnet, Noton's theme: warm paper surfaces, one vermilion accent, an
//! ink-blue focus ring that never shares a colour with selection, and a real
//! dark palette. Values are the design handoff in `docs/design/Handoff.dc.html`.
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use teksilo::prelude::*;
use teksilo::text::{FontFaceSpec, VecFontRegistrar};
use teksilo::tokens::{
    ColorTokens, FontWeight, LayoutTokens, Shadow, ShapeTokens, TextStyle, TypographyTokens,
};

/// The UI face. Note content uses [`CONTENT_FAMILY`].
pub const UI_FAMILY: &str = "Instrument Sans";
pub const CONTENT_FAMILY: &str = "Literata";
pub const MONO_FAMILY: &str = "JetBrains Mono";
/// Padding inside panes and popovers (the handoff's `spacing.panel_padding`).
pub const PANEL_PADDING: f32 = 12.;

/// The user's appearance choice. `System` follows the desktop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

fn hex(value: &str) -> Color {
    Color::from_hex(value)
}

fn light_colors() -> ColorTokens {
    ColorTokens {
        surface_main: hex("#F3EFE7"),
        surface_content: hex("#FBF9F4"),
        surface_raised: hex("#FFFEFB"),
        surface_sunken: hex("#ECE7DD"),
        surface_hover: hex("#E9E3D7"),
        surface_pressed: hex("#DFD8CA"),
        surface_selected: hex("#F7E4DA"),
        surface_selected_inactive: hex("#E6E0D5"),
        surface_alt_row: hex("#F8F5EF"),
        surface_disabled: hex("#EFEBE3"),
        text_primary: hex("#211E1A"),
        text_secondary: hex("#5F584E"),
        text_disabled: hex("#9C9488"),
        text_on_accent: hex("#FFFFFF"),
        text_link: hex("#A33A17"),
        text_link_hover: hex("#7E2A11"),
        text_link_visited: hex("#7E2A11"),
        text_error: hex("#B42318"),
        text_warning: hex("#7A4F00"),
        text_success: hex("#2F6B3A"),
        accent: hex("#B03D1A"),
        accent_hover: hex("#963315"),
        accent_pressed: hex("#7E2A11"),
        accent_disabled: hex("#D9BFB2"),
        accent_subtle_bg: hex("#F7E4DA"),
        border: hex("#DAD3C6"),
        // Handoff: #8F877A, darkened to keep 3:1 on the selected-row fill.
        border_strong: hex("#877F72"),
        border_focused: hex("#2456A6"),
        border_error: hex("#B42318"),
        border_warning: hex("#A86A00"),
        border_disabled: hex("#E4DDD0"),
        divider: hex("#E4DDD0"),
        divider_strong: hex("#DAD3C6"),
        status_info_fg: hex("#2456A6"),
        status_info_bg: hex("#E3ECF8"),
        status_success_fg: hex("#2F6B3A"),
        status_success_bg: hex("#E3F0E3"),
        status_warning_fg: hex("#7A4F00"),
        status_warning_bg: hex("#FBEFD2"),
        status_error_fg: hex("#B42318"),
        status_error_bg: hex("#FBE4E0"),
        selection_bg_active: hex("#F5D0BF"),
        selection_text_active: hex("#211E1A"),
        selection_bg_inactive: hex("#E6DFD3"),
        selection_text_inactive: hex("#211E1A"),
        search_match_bg: hex("#FBE7A1"),
        search_match_current_bg: hex("#F2B84B"),
        scrollbar_track: hex("#EFEAE1"),
        scrollbar_track_hover: hex("#E6E0D5"),
        scrollbar_thumb: hex("#C9C1B3"),
        scrollbar_thumb_hover: hex("#A89F90"),
        scrollbar_thumb_pressed: hex("#8F877A"),
        tooltip_bg: hex("#2A2622"),
        tooltip_text: hex("#F5F1EA"),
        tooltip_border: hex("#2A2622"),
        tooltip_shortcut: hex("#C9C1B3"),
        editor_bg: hex("#FFFEFB"),
        editor_fg: hex("#211E1A"),
        editor_caret: hex("#211E1A"),
        editor_current_line_bg: hex("#FFFEFB"),
        editor_gutter_fg: hex("#8F877A"),
        editor_selection_bg: hex("#F5D0BF"),
        editor_code_block_bg: hex("#F1ECE3"),
        editor_code_block_fg: hex("#211E1A"),
        focus_ring: hex("#2456A6"),
        focus_ring_error: hex("#B42318"),
        scrim: hex("#211E1A61"),
        text_on_error: hex("#FFFFFF"),
        text_on_error_container: hex("#7A1A10"),
        surface_error_container: hex("#FBE4E0"),
        surface_container: hex("#ECE7DD"),
        surface_container_raised: hex("#FFFEFB"),
        surface_container_sunken: hex("#E6E0D5"),
        ..ColorTokens::light_default()
    }
}

fn dark_colors() -> ColorTokens {
    ColorTokens {
        surface_main: hex("#1C1A17"),
        surface_content: hex("#23201C"),
        surface_raised: hex("#2C2824"),
        surface_sunken: hex("#171512"),
        surface_hover: hex("#34302A"),
        surface_pressed: hex("#3E3932"),
        surface_selected: hex("#4A2A1E"),
        surface_selected_inactive: hex("#36312B"),
        surface_alt_row: hex("#201D1A"),
        surface_disabled: hex("#25221E"),
        text_primary: hex("#EDE7DD"),
        text_secondary: hex("#B3AA9C"),
        text_disabled: hex("#6F685E"),
        text_on_accent: hex("#1C1A17"),
        text_link: hex("#F3A07F"),
        text_link_hover: hex("#F59A77"),
        text_link_visited: hex("#E06B42"),
        text_error: hex("#FF8A7A"),
        text_warning: hex("#E9B949"),
        text_success: hex("#7BC68A"),
        accent: hex("#F0825A"),
        accent_hover: hex("#F59A77"),
        accent_pressed: hex("#E06B42"),
        accent_disabled: hex("#5A3A2C"),
        accent_subtle_bg: hex("#3F261C"),
        border: hex("#3A352E"),
        // Handoff: #7A7266, lightened to keep 3:1 on the selected-row fill.
        border_strong: hex("#877F72"),
        border_focused: hex("#8DB4F2"),
        border_error: hex("#FF8A7A"),
        border_warning: hex("#E9B949"),
        border_disabled: hex("#302C26"),
        divider: hex("#302C26"),
        divider_strong: hex("#3A352E"),
        status_info_fg: hex("#9CC0F5"),
        status_info_bg: hex("#1E2A3C"),
        status_success_fg: hex("#8FD19E"),
        status_success_bg: hex("#1E2E22"),
        status_warning_fg: hex("#EBC15A"),
        status_warning_bg: hex("#36290F"),
        status_error_fg: hex("#FF9C8F"),
        status_error_bg: hex("#3D1E1A"),
        selection_bg_active: hex("#6B3522"),
        selection_text_active: hex("#EDE7DD"),
        selection_bg_inactive: hex("#3A342D"),
        selection_text_inactive: hex("#EDE7DD"),
        search_match_bg: hex("#5A4A12"),
        search_match_current_bg: hex("#7A600F"),
        scrollbar_track: hex("#201D1A"),
        scrollbar_track_hover: hex("#2C2824"),
        scrollbar_thumb: hex("#4D463D"),
        scrollbar_thumb_hover: hex("#6A6256"),
        scrollbar_thumb_pressed: hex("#7A7266"),
        tooltip_bg: hex("#EDE7DD"),
        tooltip_text: hex("#1C1A17"),
        tooltip_border: hex("#EDE7DD"),
        tooltip_shortcut: hex("#5F584E"),
        editor_bg: hex("#2C2824"),
        editor_fg: hex("#EDE7DD"),
        editor_caret: hex("#EDE7DD"),
        editor_current_line_bg: hex("#2C2824"),
        editor_gutter_fg: hex("#7A7266"),
        editor_selection_bg: hex("#6B3522"),
        editor_code_block_bg: hex("#23201C"),
        editor_code_block_fg: hex("#EDE7DD"),
        focus_ring: hex("#8DB4F2"),
        focus_ring_error: hex("#FF8A7A"),
        scrim: hex("#00000099"),
        text_on_error: hex("#1C1A17"),
        text_on_error_container: hex("#FFD2CB"),
        surface_error_container: hex("#3D1E1A"),
        surface_container: hex("#23201C"),
        surface_container_raised: hex("#2C2824"),
        surface_container_sunken: hex("#171512"),
        ..ColorTokens::dark_default()
    }
}

fn shadow(offset_y: f32, blur: f32, spread: f32, color: &str) -> Shadow {
    Shadow {
        offset_x: 0.,
        offset_y,
        blur,
        spread,
        color: hex(color),
    }
}

fn shape(dark: bool) -> ShapeTokens {
    let base = if dark {
        ShapeTokens::dark_default()
    } else {
        ShapeTokens::light_default()
    };
    let [xs, sm, md, lg] = if dark {
        ["#0000004D", "#00000066", "#00000080", "#00000099"]
    } else {
        ["#211E1A0F", "#211E1A1A", "#211E1A24", "#211E1A38"]
    };
    ShapeTokens {
        radius_control: 6.,
        radius_popup: 10.,
        radius_pill: 999.,
        border_width: 1.,
        focus_ring_width: 2.,
        focus_ring_offset: 2.,
        shadow_xs: shadow(1., 1., 0., xs),
        shadow_sm: shadow(1., 3., 0., sm),
        shadow_md: shadow(4., 12., -2., md),
        shadow_lg: shadow(12., 32., -6., lg),
        ..base
    }
}

fn style(family: &str, size: f32, line: f32, weight: FontWeight) -> TextStyle {
    TextStyle {
        family: family.into(),
        size,
        weight,
        line_height: line / size,
        letter_spacing: 0.,
    }
}

fn typography() -> TypographyTokens {
    TypographyTokens {
        body: style(UI_FAMILY, 13., 18., FontWeight::REGULAR),
        body_bold: style(UI_FAMILY, 13., 18., FontWeight::SEMI_BOLD),
        small: style(UI_FAMILY, 12., 16., FontWeight::REGULAR),
        small_bold: style(UI_FAMILY, 12., 16., FontWeight::SEMI_BOLD),
        tiny: style(UI_FAMILY, 11., 14., FontWeight::MEDIUM),
        mono: style(MONO_FAMILY, 12., 16., FontWeight::REGULAR),
    }
}

/// The Carnet theme in its light or dark variant.
pub fn carnet(dark: bool) -> Theme {
    let base = if dark {
        teksilo::presets::intui::dark()
    } else {
        teksilo::presets::intui::light()
    };
    Theme {
        id: ThemeId::new(if dark {
            "noton.carnet.dark"
        } else {
            "noton.carnet.light"
        }),
        colors: if dark { dark_colors() } else { light_colors() },
        layout: LayoutTokens {
            control_gap: 6.,
            section_gap: 20.,
        },
        typography: typography(),
        shape: shape(dark),
        ..base
    }
}

/// Resolve an appearance choice against the desktop's current preference.
pub fn resolve(appearance: Appearance, system_dark: bool) -> Theme {
    carnet(match appearance {
        Appearance::System => system_dark,
        Appearance::Light => false,
        Appearance::Dark => true,
    })
}

/// Whether the desktop currently prefers a dark theme.
pub fn system_dark() -> bool {
    matches!(
        teksilo::platform::os_theme::query_color_scheme(),
        teksilo::tokens::ColorSchemePreference::Dark
    )
}

/// The theme for the user's preferences. Reduced motion set to On removes
/// every animation duration; System and Off leave Teksilo following the OS.
pub fn for_preferences(appearance: Appearance, reduce_motion: Option<bool>) -> Theme {
    let mut theme = resolve(appearance, system_dark());
    if reduce_motion == Some(true) {
        let zero = std::time::Duration::ZERO;
        let m = &mut theme.motion;
        m.duration_fast = zero;
        m.duration_normal = zero;
        m.duration_slow = zero;
        m.duration_collapse = zero;
    }
    theme
}

/// A heading or paragraph style of note content (the editor ramp).
#[derive(Debug, Clone, Copy)]
pub struct ContentStyle {
    pub size: f32,
    pub line: f32,
    pub weight: FontWeight,
}

/// The note title in the note header.
pub const NOTE_TITLE: ContentStyle = ContentStyle {
    size: 21.,
    line: 28.,
    weight: FontWeight::SEMI_BOLD,
};

/// The note title as a [`TextStyle`], for widgets that take one.
pub fn note_title_style() -> TextStyle {
    style(
        CONTENT_FAMILY,
        NOTE_TITLE.size,
        NOTE_TITLE.line,
        NOTE_TITLE.weight,
    )
}

/// Registers Instrument Sans and Literata, upright and italic. JetBrains Mono
/// is already bundled by Teksilo.
pub fn font_registrar() -> VecFontRegistrar {
    let face = |data: &'static [u8]| FontFaceSpec {
        data: Arc::new(data),
        is_default: false,
        default_size_px: 13.,
    };
    VecFontRegistrar::new(vec![
        face(include_bytes!("../assets/fonts/InstrumentSans.ttf")),
        face(include_bytes!("../assets/fonts/InstrumentSans-Italic.ttf")),
        face(include_bytes!("../assets/fonts/Literata-Variable.ttf")),
        face(include_bytes!(
            "../assets/fonts/Literata-Italic-Variable.ttf"
        )),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG AA for the pairs the design lists: 4.5:1 for text, 3:1 for the
    /// focus ring and the strong border against the surfaces they sit on.
    #[test]
    fn carnet_meets_aa_contrast_in_both_themes() {
        for dark in [false, true] {
            let c = carnet(dark).colors;
            for surface in [
                c.surface_main,
                c.surface_content,
                c.surface_raised,
                c.surface_selected,
            ] {
                for text in [c.text_primary, c.text_secondary] {
                    assert!(text.contrast_ratio(surface) >= 4.5, "dark={dark}");
                }
                assert!(
                    c.border_focused.contrast_ratio(surface) >= 3.,
                    "dark={dark}"
                );
                assert!(c.border_strong.contrast_ratio(surface) >= 3., "dark={dark}");
            }
            assert!(
                c.text_on_accent.contrast_ratio(c.accent) >= 4.5,
                "dark={dark}"
            );
            assert!(c.text_link.contrast_ratio(c.surface_content) >= 4.5);
            assert!(c.text_error.contrast_ratio(c.status_error_bg) >= 4.5);
            assert!(c.tooltip_text.contrast_ratio(c.tooltip_bg) >= 4.5);
        }
    }

    #[test]
    fn appearance_resolves_system_to_the_desktop() {
        assert!(resolve(Appearance::System, true).is_dark());
        assert!(!resolve(Appearance::System, false).is_dark());
        assert!(resolve(Appearance::Dark, false).is_dark());
        assert!(!resolve(Appearance::Light, true).is_dark());
    }
}
