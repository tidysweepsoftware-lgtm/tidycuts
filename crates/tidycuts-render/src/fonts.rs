//! Fonts embedded in TidyCuts, all under the SIL Open Font License or the
//! Apache License (see the license files next to them).

pub struct FontDef {
    /// Name shown in the font picker and saved in projects.
    pub name: &'static str,
    pub data: &'static [u8],
}

macro_rules! font {
    ($name:expr, $file:expr) => {
        FontDef {
            name: $name,
            data: include_bytes!(concat!("../fonts/", $file)),
        }
    };
}

pub static FONTS: &[FontDef] = &[
    font!("Poppins", "Poppins-Regular.ttf"),
    font!("Poppins SemiBold", "Poppins-SemiBold.ttf"),
    font!("Poppins Bold", "Poppins-Bold.ttf"),
    font!("Poppins Black", "Poppins-Black.ttf"),
    font!("Inter", "Inter-Variable.ttf"),
    font!("Anton", "Anton-Regular.ttf"),
    font!("Bebas Neue", "BebasNeue-Regular.ttf"),
    font!("Abril Fatface", "AbrilFatface-Regular.ttf"),
    font!("DM Serif Display", "DMSerifDisplay-Regular.ttf"),
    font!("Righteous", "Righteous-Regular.ttf"),
    font!("Lobster", "Lobster-Regular.ttf"),
    font!("Pacifico", "Pacifico-Regular.ttf"),
    font!("Great Vibes", "GreatVibes-Regular.ttf"),
    font!("Permanent Marker", "PermanentMarker-Regular.ttf"),
    font!("Bangers", "Bangers-Regular.ttf"),
    font!("Black Ops One", "BlackOpsOne-Regular.ttf"),
    font!("Monoton", "Monoton-Regular.ttf"),
    font!("Press Start 2P", "PressStart2P-Regular.ttf"),
    font!("VT323", "VT323-Regular.ttf"),
    font!("Special Elite", "SpecialElite-Regular.ttf"),
];

pub fn get(name: &str) -> &'static FontDef {
    FONTS
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(name))
        .unwrap_or(&FONTS[2])
}

/// The UI font.
pub fn inter() -> &'static [u8] {
    get("Inter").data
}

pub fn poppins_semibold() -> &'static [u8] {
    get("Poppins SemiBold").data
}
