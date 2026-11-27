use std::ops::{Range, RangeInclusive};

use super::glyphs::Glyphs;
use super::rows::GLYPHS;

const SURE_ENOUGH: f64 = 0.3;
const LAST_PLANET: u32 = 6;
const SYSTEMS_ON_THE_MAP: RangeInclusive<u32> = 0x001..=0x2FF;
const PURPLE_SYSTEMS: RangeInclusive<u32> = 0x3E8..=0x429;
const NO_SUCH_HEIGHT: u32 = 0x80;
const NO_SUCH_COORDINATE: u32 = 0x800;

pub fn checked(glyphs: &Glyphs) -> Option<String> {
    accepted(glyphs).then(|| glyphs.text.clone())
}

pub fn accepted(glyphs: &Glyphs) -> bool {
    glyphs.score >= SURE_ENOUGH && plausible(&glyphs.text)
}

pub fn plausible(text: &str) -> bool {
    let digits = |range: Range<usize>| {
        let part = text.get(range)?;
        u32::from_str_radix(part, 16).ok()
    };
    let parts = (
        digits(0..1),
        digits(1..4),
        digits(4..6),
        digits(6..9),
        digits(9..12),
    );
    let (Some(planet), Some(system), Some(height), Some(z), Some(x)) = parts else {
        return false;
    };
    text.len() == GLYPHS
        && planet <= LAST_PLANET
        && (SYSTEMS_ON_THE_MAP.contains(&system) || PURPLE_SYSTEMS.contains(&system))
        && height != NO_SUCH_HEIGHT
        && z != NO_SUCH_COORDINATE
        && x != NO_SUCH_COORDINATE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::Rect;

    fn glyphs(text: &str, score: f64) -> Glyphs {
        Glyphs {
            text: text.into(),
            score,
            strip: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            voters: Vec::new(),
        }
    }

    #[test]
    fn an_address_is_given_from_its_score_and_an_address_the_game_opens() {
        assert_eq!(
            checked(&glyphs("107407647D3B", 0.3)).as_deref(),
            Some("107407647D3B")
        );
        assert!(!accepted(&glyphs("107407647D3B", 0.299)));
        assert!(accepted(&glyphs("607407647D3B", 0.95)));
        assert!(!accepted(&glyphs("707407647D3B", 0.95)));
        assert!(!accepted(&glyphs("1074?7647D3B", 0.95)));
        assert!(!accepted(&glyphs("107480647D3B", 0.95)));
        assert!(!accepted(&glyphs("107407800D3B", 0.95)));
        assert!(!accepted(&glyphs("107407647800", 0.95)));
    }

    #[test]
    fn a_system_index_is_a_star_the_map_draws_or_a_purple_one() {
        for system in ["001", "2FF", "3E8", "429"] {
            assert!(plausible(&format!("1{system}07647D3B")), "{system}");
        }
        for system in ["000", "300", "3E7", "42A", "FFF"] {
            assert!(!plausible(&format!("1{system}07647D3B")), "{system}");
        }
    }
}
