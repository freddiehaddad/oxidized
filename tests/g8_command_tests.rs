use oxidized::core::buffer::Buffer;
use oxidized::core::mode::Position;
use unicode_segmentation::UnicodeSegmentation;

#[test]
fn g8_ascii_manual_grapheme() {
    let mut buffer = Buffer::new(1, 100);
    for ch in "A".chars() {
        buffer.insert_char(ch);
    }
    buffer.cursor = Position::new(0, 0);
    let line = &buffer.lines[0];
    let grapheme = line.graphemes(true).next().unwrap();
    assert_eq!(grapheme, "A");
    assert_eq!(grapheme.as_bytes(), &[0x41]);
}

#[test]
fn g8_multibyte_graphemes_manual() {
    let mut buffer = Buffer::new(1, 100);
    for ch in "☃😀".chars() {
        buffer.insert_char(ch);
    }
    buffer.cursor = Position::new(0, 0);
    let line = &buffer.lines[0];
    let mut iter = line.graphemes(true);
    let snowman = iter.next().unwrap();
    assert_eq!(snowman, "☃");
    assert_eq!(snowman.as_bytes(), &[0xE2, 0x98, 0x83]);
    let emoji = iter.next().unwrap();
    assert_eq!(emoji, "😀");
    assert_eq!(emoji.as_bytes(), &[0xF0, 0x9F, 0x98, 0x80]);
}

#[test]
fn g8_combining_sequence_manual() {
    let mut buffer = Buffer::new(1, 100);
    for ch in "a\u{0301}".chars() {
        buffer.insert_char(ch);
    }
    buffer.cursor = Position::new(0, 0);
    let line = &buffer.lines[0];
    let grapheme = line.graphemes(true).next().unwrap();
    // Depending on normalization, this may be two code points in one grapheme
    let codepoints: Vec<u32> = grapheme.chars().map(|c| c as u32).collect();
    assert_eq!(codepoints, vec![0x61, 0x301]);
    // Bytes should be 61 CC 81
    let bytes: Vec<u8> = grapheme.as_bytes().to_vec();
    assert_eq!(bytes, vec![0x61, 0xCC, 0x81]);
}
