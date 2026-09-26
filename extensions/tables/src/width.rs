//! How many columns a string takes in a monospace font. Counts one per
//! character, two for East Asian wide and fullwidth characters and most
//! emoji, none for combining marks, variation selectors and zero-width
//! joiners. The ranges are approximate and cover the common cases.

pub fn width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

pub fn char_width(c: char) -> usize {
    let cp = u32::from(c);
    if cp < 0x20 || (0x7F..0xA0).contains(&cp) || is_zero_width(cp) {
        0
    } else if is_wide(cp) {
        2
    } else {
        1
    }
}

fn is_zero_width(cp: u32) -> bool {
    matches!(
        cp,
        0x0300..=0x036F   // combining diacritical marks
        | 0x0483..=0x0489
        | 0x0591..=0x05BD
        | 0x0610..=0x061A
        | 0x064B..=0x065F
        | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF
        | 0x200B..=0x200F // zero width space, joiners, marks
        | 0x2028..=0x202E
        | 0x2060..=0x2064
        | 0x20D0..=0x20FF
        | 0xFE00..=0xFE0F // variation selectors
        | 0xFE20..=0xFE2F
        | 0xFEFF
        | 0xE0100..=0xE01EF
    )
}

fn is_wide(cp: u32) -> bool {
    matches!(
        cp,
        0x1100..=0x115F   // Hangul Jamo
        | 0x2E80..=0x303E // CJK radicals, punctuation
        | 0x3041..=0x33FF // Hiragana, Katakana, compatibility
        | 0x3400..=0x4DBF // CJK extension A
        | 0x4E00..=0x9FFF // CJK unified
        | 0xA000..=0xA4CF // Yi
        | 0xAC00..=0xD7A3 // Hangul syllables
        | 0xF900..=0xFAFF // CJK compatibility ideographs
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60 // fullwidth forms
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F // emoji: symbols, emoticons
        | 0x1F680..=0x1F6FF // transport
        | 0x1F900..=0x1F9FF // supplemental symbols
        | 0x1FA70..=0x1FAFF
        | 0x20000..=0x3FFFD // CJK extensions B and beyond
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_wide_and_zero_width_characters() {
        assert_eq!(width("Name"), 4);
        assert_eq!(width("日本語"), 6);
        assert_eq!(width("한글"), 4);
        assert_eq!(width("e\u{301}"), 1);
        assert_eq!(width("🍐"), 2);
        assert_eq!(width(""), 0);
    }
}
