// See the normative reference for HTML5 entities:
// https://html.spec.whatwg.org/multipage/named-characters.html#named-character-references
//
// Some entities do not require a trailing semicolon. Some of those entities
// are prefixes for multiple other entities. For example:
//   &times &times; &timesb; &timesbar; &timesd;

use std::char;
use std::cmp::min;
use std::iter::Peekable;

// Include the ENTITIES map generated by build.rs
include!(concat!(env!("OUT_DIR"), "/entities.rs"));

/// Expand all valid entities
///
/// The WHATWG HTML spec contains the normative reference for
/// [named entities](https://html.spec.whatwg.org/multipage/named-characters.html#named-character-references).
/// This is based on the [algorithm described](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state)
/// in the WHATWG spec.
///
/// **FIXME [Named character reference state special cases](https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state)**
pub fn unescape<S: AsRef<[u8]>>(escaped: S) -> String {
    let escaped = escaped.as_ref();
    let mut iter = escaped.iter().peekable();
    let mut buffer = Vec::new(); // FIXME Vec::with_capacity(escaped.len())? Shrink on return?

    while let Some(c) = iter.next() {
        if *c == b'&' {
            let mut expansion = match_entity(&mut iter);
            buffer.append(&mut expansion);
        } else {
            buffer.push(*c);
        }
    }

    String::from_utf8(buffer).unwrap()
}

const PEEK_MATCH_ERROR: &str = "iter.next() did not match previous iter.peek()";

#[allow(clippy::from_str_radix_10)]
fn match_numeric_entity<'a, I>(iter: &mut Peekable<I>) -> Vec<u8>
where
    I: Iterator<Item = &'a u8>,
{
    let c = iter.next().expect(PEEK_MATCH_ERROR);
    if *c != b'#' {
        panic!("{}", PEEK_MATCH_ERROR);
    }

    let mut best_expansion = vec![b'&', b'#'];

    let number = match iter.peek() {
        Some(&b'x') | Some(&b'X') => {
            // Hexadecimal entity
            best_expansion.push(*iter.next().expect(PEEK_MATCH_ERROR));

            let hex = consume_hexadecimal(iter);
            best_expansion.extend_from_slice(&hex);

            u32::from_str_radix(&String::from_utf8(hex).unwrap(), 16)
        }
        Some(_) => {
            // Presumably a decimal entity
            let dec = consume_decimal(iter);
            best_expansion.extend_from_slice(&dec);

            u32::from_str_radix(&String::from_utf8(dec).unwrap(), 10)
        }
        None => {
            // Iterator reached end
            return best_expansion;
        }
    };

    if let Some(&b';') = iter.peek() {
        best_expansion.push(*iter.next().expect(PEEK_MATCH_ERROR));
    } else {
        // missing-semicolon-after-character-reference: end the entity anyway.
        // https://html.spec.whatwg.org/multipage/parsing.html#parse-error-missing-semicolon-after-character-reference
    }

    if let Ok(number) = number {
        if let Some(expansion) = correct_numeric_entity(number) {
            return expansion;
        }
    }

    best_expansion
}

/// Unicode replacement character (U+FFFD �)
///
/// According to the WHATWG HTML spec, this is used as an expansion for certain
/// invalid numeric entities.
///
/// According to Unicode 12, this is “used to replace an incoming character
/// whose value is unknown or unrepresentable in Unicode.” The latest chart for
/// the Specials block is [available as a PDF](https://www.unicode.org/charts/PDF/UFFF0.pdf).
pub const REPLACEMENT_CHAR: char = '\u{fffd}';

// https://infra.spec.whatwg.org/#noncharacter
fn is_noncharacter<C: Into<u32>>(c: C) -> bool {
    matches!(
        c.into(),
        (0xFDD0..=0xFDEF)
            | 0xFFFE
            | 0xFFFF
            | 0x1FFFE
            | 0x1FFFF
            | 0x2FFFE
            | 0x2FFFF
            | 0x3FFFE
            | 0x3FFFF
            | 0x4FFFE
            | 0x4FFFF
            | 0x5FFFE
            | 0x5FFFF
            | 0x6FFFE
            | 0x6FFFF
            | 0x7FFFE
            | 0x7FFFF
            | 0x8FFFE
            | 0x8FFFF
            | 0x9FFFE
            | 0x9FFFF
            | 0xAFFFE
            | 0xAFFFF
            | 0xBFFFE
            | 0xBFFFF
            | 0xCFFFE
            | 0xCFFFF
            | 0xDFFFE
            | 0xDFFFF
            | 0xEFFFE
            | 0xEFFFF
            | 0xFFFFE
            | 0xFFFFF
            | 0x10FFFE
            | 0x10FFFF
    )
}

// https://html.spec.whatwg.org/multipage/parsing.html#parse-error-character-reference-outside-unicode-range
fn is_outside_range<C: Into<u32>>(c: C) -> bool {
    c.into() > 0x10FFFF
}

// https://infra.spec.whatwg.org/#surrogate
fn is_surrogate<C: Into<u32>>(c: C) -> bool {
    (0xD800..=0xDFFF).contains(&c.into())
}

// https://infra.spec.whatwg.org/#surrogate
fn is_control<C: Into<u32>>(c: C) -> bool {
    let c = c.into();
    (0..=0x1F).contains(&c) || (0x7F..=0x9F).contains(&c)
}

// https://infra.spec.whatwg.org/#ascii-whitespace
//
// This is the same as char::is_ascii_whitespace(), but I’m implementing it
// by hand for consistency.
fn is_ascii_whitespace<C: Into<u32>>(c: C) -> bool {
    // (horizontal) tab, line feed, form feed, carriage return, space
    matches!(c.into(), 0x09 | 0x0A | 0x0C | 0x0D | 0x20)
}

// https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state
fn correct_numeric_entity(number: u32) -> Option<Vec<u8>> {
    #[inline]
    fn char_to_vecu8(c: char) -> Option<Vec<u8>> {
        Some(c.to_string().into())
    }

    #[inline]
    fn u32_to_vecu8(c: u32) -> Option<Vec<u8>> {
        Some(char::from_u32(c).unwrap().to_string().into())
    }

    match number {
        // null-character-reference parse error:
        0x00 => char_to_vecu8(REPLACEMENT_CHAR),

        // character-reference-outside-unicode-range parse error:
        c if is_outside_range(c) => char_to_vecu8(REPLACEMENT_CHAR),

        // surrogate-character-reference parse error:
        c if is_surrogate(c) => char_to_vecu8(REPLACEMENT_CHAR),

        // noncharacter-character-reference parse error:
        c if is_noncharacter(c) => None,

        // control-character-reference parse error exceptions:
        0x80 => u32_to_vecu8(0x20AC), // EURO SIGN (€)
        0x82 => u32_to_vecu8(0x201A), // SINGLE LOW-9 QUOTATION MARK (‚)
        0x83 => u32_to_vecu8(0x0192), // LATIN SMALL LETTER F WITH HOOK (ƒ)
        0x84 => u32_to_vecu8(0x201E), // DOUBLE LOW-9 QUOTATION MARK („)
        0x85 => u32_to_vecu8(0x2026), // HORIZONTAL ELLIPSIS (…)
        0x86 => u32_to_vecu8(0x2020), // DAGGER (†)
        0x87 => u32_to_vecu8(0x2021), // DOUBLE DAGGER (‡)
        0x88 => u32_to_vecu8(0x02C6), // MODIFIER LETTER CIRCUMFLEX ACCENT (ˆ)
        0x89 => u32_to_vecu8(0x2030), // PER MILLE SIGN (‰)
        0x8A => u32_to_vecu8(0x0160), // LATIN CAPITAL LETTER S WITH CARON (Š)
        0x8B => u32_to_vecu8(0x2039), // SINGLE LEFT-POINTING ANGLE QUOTATION MARK (‹)
        0x8C => u32_to_vecu8(0x0152), // LATIN CAPITAL LIGATURE OE (Œ)
        0x8E => u32_to_vecu8(0x017D), // LATIN CAPITAL LETTER Z WITH CARON (Ž)
        0x91 => u32_to_vecu8(0x2018), // LEFT SINGLE QUOTATION MARK (‘)
        0x92 => u32_to_vecu8(0x2019), // RIGHT SINGLE QUOTATION MARK (’)
        0x93 => u32_to_vecu8(0x201C), // LEFT DOUBLE QUOTATION MARK (“)
        0x94 => u32_to_vecu8(0x201D), // RIGHT DOUBLE QUOTATION MARK (”)
        0x95 => u32_to_vecu8(0x2022), // BULLET (•)
        0x96 => u32_to_vecu8(0x2013), // EN DASH (–)
        0x97 => u32_to_vecu8(0x2014), // EM DASH (—)
        0x98 => u32_to_vecu8(0x02DC), // SMALL TILDE (˜)
        0x99 => u32_to_vecu8(0x2122), // TRADE MARK SIGN (™)
        0x9A => u32_to_vecu8(0x0161), // LATIN SMALL LETTER S WITH CARON (š)
        0x9B => u32_to_vecu8(0x203A), // SINGLE RIGHT-POINTING ANGLE QUOTATION MARK (›)
        0x9C => u32_to_vecu8(0x0153), // LATIN SMALL LIGATURE OE (œ)
        0x9E => u32_to_vecu8(0x017E), // LATIN SMALL LETTER Z WITH CARON (ž)
        0x9F => u32_to_vecu8(0x0178), // LATIN CAPITAL LETTER Y WITH DIAERESIS (Ÿ)

        // control-character-reference parse error:
        0x0D => None,
        c if is_ascii_whitespace(c) => u32_to_vecu8(c),
        c if is_control(c) => None,

        // Everything else.
        c => match char::from_u32(c) {
            Some(c) => char_to_vecu8(c),
            None => None,
        },
    }
}

macro_rules! consumer {
    ($name:ident, $($accept:pat)|+) => {
        fn $name<'a, I>(iter: &mut Peekable<I>) -> Vec<u8>
            where I: Iterator<Item = &'a u8>
        {
            let mut buffer: Vec<u8> = Vec::new();
            while let Some(c) = iter.peek() {
                match **c {
                    $($accept)|+ => {
                        buffer.push(*iter.next().expect(PEEK_MATCH_ERROR));
                    },
                    _ => { return buffer; },
                }
            }

            return buffer;
        }
    }
}

consumer!(consume_decimal, b'0'..=b'9');
consumer!(consume_hexadecimal, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F');
consumer!(consume_alphanumeric, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z');

fn match_entity<'a, I>(iter: &mut Peekable<I>) -> Vec<u8>
where
    I: Iterator<Item = &'a u8>,
{
    if let Some(&b'#') = iter.peek() {
        // Numeric entity.
        return match_numeric_entity(iter);
    }

    // Determine longest possible candidate including & and any trailing ;.
    let mut candidate = vec![b'&'];
    candidate.append(&mut consume_alphanumeric(iter));

    if let Some(&b';') = iter.peek() {
        // Actually consume the semicolon.
        candidate.push(*iter.next().expect(PEEK_MATCH_ERROR));
    }

    if candidate.len() < ENTITY_MIN_LENGTH {
        // Couldn’t possibly match.
        return candidate;
    }

    // Find longest matching entity.
    let max_len = min(candidate.len(), ENTITY_MAX_LENGTH);
    for check_len in (ENTITY_MIN_LENGTH..=max_len).rev() {
        if let Some(expansion) = ENTITIES.get(&candidate[..check_len]) {
            // Found a match.
            let mut result = Vec::with_capacity(
                expansion.len() + candidate.len() - check_len,
            );
            result.extend_from_slice(expansion);

            if check_len < candidate.len() {
                // Need to append the rest of the consumed bytes.
                result.extend_from_slice(&candidate[check_len..]);
            }

            return result;
        }
    }

    // Did not find a match.
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    test!(almost_entity, unescape("&time") == "&time");
    test!(exact_no_semicolon, unescape("&times") == "×");
    test!(exact, unescape("&times;") == "×");
    test!(entity_char, unescape("&timesa") == "×a");
    test!(entity_char_is_prefix, unescape("&timesb") == "×b");
    test!(exact_timesb, unescape("&timesb;") == "⊠");

    test!(no_entities, unescape("none") == "none");
    test!(only_ampersand, unescape("&") == "&");
    test!(empty_entity, unescape("&;") == "&;");
    test!(middle_entity, unescape(" &amp; ") == " & ");
    test!(extra_ampersands, unescape("&&amp;&") == "&&&");
    test!(two_entities, unescape("AND &amp;&AMP; and") == "AND && and");
    test!(
        long_entity,
        unescape("&aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;")
            == "&aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;"
    );

    test!(correct_hex_lowerx_lower, unescape("&#x7a;") == "z");
    test!(correct_hex_lowerx_upper, unescape("&#x7A;") == "z");
    test!(correct_hex_upperx_lower, unescape("&#X7a;") == "z");
    test!(correct_hex_upperx_upper, unescape("&#X7A;") == "z");
    test!(correct_dec, unescape("&#122;") == "z");
    test!(correct_hex_unicode, unescape("&#x21D2;") == "⇒");

    test!(hex_no_semicolon, unescape("&#x7Az") == "zz");
    test!(hex_no_semicolon_end, unescape("&#x7A") == "z");
    test!(dec_no_semicolon, unescape("&#122z") == "zz");
    test!(dec_no_semicolon_end, unescape("&#122") == "z");

    test!(hex_instead_of_dec, unescape("&#7a;") == "&#7a;");
    test!(invalid_hex_lowerx, unescape("&#xZ;") == "&#xZ;");
    test!(invalid_hex_upperx, unescape("&#XZ;") == "&#XZ;");

    test!(special_entity_null, unescape("&#0;") == "\u{fffd}");
    test!(special_entity_bullet, unescape("&#x95;") == "•");
    test!(
        special_entity_bullets,
        unescape("&#x95;&#149;&#x2022;•") == "••••"
    );
    test!(special_entity_space, unescape("&#x20") == " ");

    const ALL_SOURCE: &str =
        include_str!("../tests/corpus/all-entities-source.txt");
    const ALL_EXPANDED: &str =
        include_str!("../tests/corpus/all-entities-expanded.txt");
    test!(all_entities, unescape(ALL_SOURCE) == ALL_EXPANDED);
}
