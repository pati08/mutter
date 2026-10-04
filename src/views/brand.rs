//! The `Mutt` + `ER` wordmark.

use dioxus::prelude::*;

const ER_SVG: Asset = asset!("/assets/er.svg");

/// Renders "Mutt" followed by the `ER` tile. `size_class` sets the text size
/// (e.g. `text-xl`); the tile scales with it.
///
/// Tailwind's preflight makes images `display: block`, which knocks the tile off
/// the text line, so `display: inline-block` is forced here. The tile is centered
/// on the text's cap height (the `ER` glyphs are smaller than the caps, so
/// baseline-aligning the tile would leave it hanging below the baseline).
#[component]
pub fn Brand(size_class: String) -> Element {
    rsx! {
        span { class: "font-semibold {size_class}",
            "Mutt"
            img {
                src: ER_SVG,
                style: "display: inline-block; height: 1.12em; width: 1.12em; vertical-align: -0.206em; margin-left: 0.08em;",
                alt: "er",
            }
        }
    }
}
