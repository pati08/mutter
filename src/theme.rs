//! Per-user accent colors.
//!
//! The accent is stored per account and applied through CSS custom properties, so
//! the palette can change at runtime without rebuilding Tailwind. Derived shades
//! (legible foreground, contrasting border, soft fills, a faint text tint) are
//! computed here.

/// The default accent: a neutral gray.
pub const DEFAULT_ACCENT: &str = "#c3c7cc";

/// Chill pastels offered by default; the advanced picker allows any color.
pub const PRESETS: &[&str] = &[
    "#d8c9ec", // lavender
    "#c2d8f2", // sky
    "#bfe3dc", // seafoam
    "#cbe0bd", // sage
    "#f4e2b8", // butter
    "#f7d3c4", // peach
    "#f0cbd6", // rose
    "#d3ccf0", // periwinkle
    "#e4d9c4", // sand
    "#d6dade", // stone
];

/// The dark base that body text is tinted from.
const INK_BASE: (u8, u8, u8) = (15, 23, 42);

/// The accent to preselect in the picker: the user's existing color, or a random
/// preset when they haven't chosen one yet.
pub fn initial_accent(existing: Option<&str>) -> String {
    match existing {
        Some(color) => color.to_string(),
        None => {
            let index = random_index(PRESETS.len());
            PRESETS
                .get(index)
                .copied()
                .unwrap_or(DEFAULT_ACCENT)
                .to_string()
        }
    }
}

fn random_index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }

    #[cfg(target_arch = "wasm32")]
    {
        ((js_sys::Math::random() * len as f64) as usize).min(len - 1)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos() as usize)
            .unwrap_or_default();
        nanos % len
    }
}

/// Whether `color` is a valid `#rrggbb` string. Only the server needs to validate.
#[cfg(feature = "server")]
pub fn is_valid_hex(color: &str) -> bool {
    parse_hex(color).is_some()
}

/// The inline style that exposes an accent and its derived shades as CSS variables.
///
/// Returns an empty string for an invalid color, so the caller falls back to
/// whatever the surrounding scope provides.
pub fn style_for(color: &str) -> String {
    let Some(rgb) = parse_hex(color) else {
        return String::new();
    };

    let ink = tint_toward_ink(rgb, 0.14);

    format!(
        "--accent:{};\
         --accent-fg:{};\
         --accent-border:{};\
         --accent-soft:{};\
         --accent-softer:{};\
         --accent-shadow:{};\
         --ink:{};\
         --ink-soft:{};\
         --ink-muted:{};",
        to_hex(rgb),
        foreground(rgb),
        to_hex(scale(rgb, 0.75)),
        to_hex(mix_toward_white(rgb, 0.24)),
        to_hex(mix_toward_white(rgb, 0.11)),
        rgba(rgb, 0.45),
        to_hex(ink),
        to_hex(mix_toward_white(ink, 0.75)),
        to_hex(mix_toward_white(ink, 0.55)),
    )
}

/// Black or white, whichever is legible on `rgb`.
fn foreground((r, g, b): (u8, u8, u8)) -> &'static str {
    let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;

    if luminance > 150.0 {
        "#1e293b"
    } else {
        "#ffffff"
    }
}

/// Blends a color toward the dark ink base; `weight` is how much accent to keep.
fn tint_toward_ink((r, g, b): (u8, u8, u8), weight: f32) -> (u8, u8, u8) {
    let f = |c: u8, base: u8| (c as f32 * weight + base as f32 * (1.0 - weight)) as u8;
    (f(r, INK_BASE.0), f(g, INK_BASE.1), f(b, INK_BASE.2))
}

/// Scales a color toward black.
fn scale((r, g, b): (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    let f = |c: u8| (c as f32 * factor).clamp(0.0, 255.0) as u8;
    (f(r), f(g), f(b))
}

/// Mixes a color toward white; `weight` is how much of the original to keep.
fn mix_toward_white((r, g, b): (u8, u8, u8), weight: f32) -> (u8, u8, u8) {
    let f = |c: u8| (c as f32 * weight + 255.0 * (1.0 - weight)) as u8;
    (f(r), f(g), f(b))
}

fn rgba((r, g, b): (u8, u8, u8), alpha: f32) -> String {
    format!("rgba({r}, {g}, {b}, {alpha})")
}

fn parse_hex(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

fn to_hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_a_neutral_gray_with_dark_text() {
        let style = style_for(DEFAULT_ACCENT);

        assert!(style.starts_with("--accent:#c3c7cc;"));
        assert!(style.contains("--accent-fg:#1e293b;"));
        assert!(style.contains("--accent-shadow:rgba(195, 199, 204, 0.45)"));
        assert!(style.contains("--ink:#"));
    }

    #[test]
    fn presets_normalize_and_get_dark_text() {
        for preset in PRESETS {
            let style = style_for(preset);
            assert!(
                style.contains(&format!("--accent:{preset};")),
                "{preset} should normalize to itself"
            );
            assert!(
                style.contains("--accent-fg:#1e293b;"),
                "{preset} should get dark text"
            );
        }
    }

    #[test]
    fn invalid_colors_yield_nothing() {
        assert!(style_for("not-a-color").is_empty());
        assert!(style_for("#12345").is_empty());
    }

    #[test]
    fn existing_accent_is_preselected() {
        assert_eq!(initial_accent(Some("#a9c6e8")), "#a9c6e8");
    }

    #[test]
    fn missing_accent_picks_a_listed_preset() {
        let chosen = initial_accent(None);
        assert!(
            PRESETS.contains(&chosen.as_str()),
            "{chosen} should be a preset"
        );
    }
}
