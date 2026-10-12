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
    font!("Montserrat", "Montserrat-ExtraBold.ttf"),
    font!("Montserrat Light", "Montserrat-Light.ttf"),
    font!("Outfit", "Outfit-Bold.ttf"),
    font!("Outfit Light", "Outfit-Light.ttf"),
    font!("Sora", "Sora-Bold.ttf"),
    font!("Space Grotesk", "SpaceGrotesk-Bold.ttf"),
    font!("Unbounded", "Unbounded-Bold.ttf"),
    font!("Bricolage Grotesque", "BricolageGrotesque-ExtraBold.ttf"),
    font!("Teko", "Teko-SemiBold.ttf"),
    font!("Oswald", "Oswald-SemiBold.ttf"),
    font!("Big Shoulders Display", "BigShouldersDisplay-ExtraBold.ttf"),
    font!("Cinzel", "Cinzel-Bold.ttf"),
    font!("Caveat", "Caveat-Bold.ttf"),
    font!("JetBrains Mono", "JetBrainsMono-Bold.ttf"),
    font!("Lilita One", "LilitaOne-Regular.ttf"),
    font!("Titan One", "TitanOne-Regular.ttf"),
    font!("Passion One", "PassionOne-Black.ttf"),
    font!("Chango", "Chango-Regular.ttf"),
    font!("Bowlby One", "BowlbyOne-Regular.ttf"),
    font!("Staatliches", "Staatliches-Regular.ttf"),
    font!("Archivo Black", "ArchivoBlack-Regular.ttf"),
    font!("Krona One", "KronaOne-Regular.ttf"),
    font!("Michroma", "Michroma-Regular.ttf"),
    font!("Russo One", "RussoOne-Regular.ttf"),
    font!("Audiowide", "Audiowide-Regular.ttf"),
    font!("Rajdhani Bold", "Rajdhani-Bold.ttf"),
    font!("Chakra Petch Bold", "ChakraPetch-Bold.ttf"),
    font!("Quantico Bold", "Quantico-Bold.ttf"),
    font!("Dela Gothic One", "DelaGothicOne-Regular.ttf"),
    font!("Bungee", "Bungee-Regular.ttf"),
    font!("Bungee Inline", "BungeeInline-Regular.ttf"),
    font!("Faster One", "FasterOne-Regular.ttf"),
    font!("Rubik Glitch", "RubikGlitch-Regular.ttf"),
    font!("Londrina Solid", "LondrinaSolid-Black.ttf"),
    font!("Limelight", "Limelight-Regular.ttf"),
    font!("Stalinist One", "StalinistOne-Regular.ttf"),
    font!("Poiret One", "PoiretOne-Regular.ttf"),
    font!("Gloock", "Gloock-Regular.ttf"),
    font!("Cinzel Decorative", "CinzelDecorative-Bold.ttf"),
    font!("Allura", "Allura-Regular.ttf"),
    font!("Sacramento", "Sacramento-Regular.ttf"),
    font!("Parisienne", "Parisienne-Regular.ttf"),
    font!("Playball", "Playball-Regular.ttf"),
    font!("Courgette", "Courgette-Regular.ttf"),
    font!("Kalam Bold", "Kalam-Bold.ttf"),
    font!("Damion", "Damion-Regular.ttf"),
    font!("Norican", "Norican-Regular.ttf"),
    font!("Mr Dafoe", "MrDafoe-Regular.ttf"),
    font!("Kaushan Script", "KaushanScript-Regular.ttf"),
    font!("Marck Script", "MarckScript-Regular.ttf"),
    font!("Nanum Brush Script", "NanumBrushScript-Regular.ttf"),
    font!("Space Mono Bold", "SpaceMono-Bold.ttf"),
    font!("IBM Plex Mono Bold", "IBMPlexMono-Bold.ttf"),
    font!("Major Mono Display", "MajorMonoDisplay-Regular.ttf"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ab_glyph::{Font, FontRef};

    #[test]
    fn every_font_loads_and_has_letters_and_digits() {
        let mut names = std::collections::HashSet::new();
        for f in FONTS {
            assert!(
                names.insert(f.name.to_ascii_lowercase()),
                "{} twice",
                f.name
            );
            let font =
                FontRef::try_from_slice(f.data).unwrap_or_else(|e| panic!("{}: {e}", f.name));
            for c in ['A', 'g', '7'] {
                assert!(font.glyph_id(c).0 != 0, "{} has no '{c}'", f.name);
            }
        }
        assert!(FONTS.len() >= 70, "{} fonts", FONTS.len());
    }
}
