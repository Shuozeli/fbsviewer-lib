use flatbuf_visualizer_core::{parse_hex_bytes, HexParseError};

#[test]
fn test_parse_spaced_hex() {
    let result = parse_hex_bytes("14 00 00 00").unwrap();
    assert_eq!(result, vec![0x14, 0x00, 0x00, 0x00]);
}

#[test]
fn test_parse_0x_prefix() {
    let result = parse_hex_bytes("0x14 0xFF").unwrap();
    assert_eq!(result, vec![0x14, 0xFF]);
}

#[test]
fn test_empty_returns_error() {
    let result = parse_hex_bytes("");
    assert!(matches!(result, Err(HexParseError::Empty)));
}

#[test]
fn test_invalid_byte_returns_error() {
    let result = parse_hex_bytes("ZZ");
    assert!(matches!(result, Err(HexParseError::InvalidByte { .. })));
}

#[test]
fn test_mixed_whitespace() {
    let result = parse_hex_bytes("14\t00\n00\r\n00").unwrap();
    assert_eq!(result, vec![0x14, 0x00, 0x00, 0x00]);
}
