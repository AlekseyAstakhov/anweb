#[cfg(test)]
use crate::websocket::{Parser, frame, TEXT_OPCODE, BINARY_OPCODE};

#[test]
fn parse_one_good_frame() {
    let incoming_data = [129, 140, 211, 25, 248, 86, 155, 124, 148, 58, 188, 57, 143, 57, 161, 117, 156, 119];
    let mut parser = Parser::new();
    if let Ok(result) = parser.push(&incoming_data, 12) {
        if let Some((frame, surplus)) = result {
            assert_eq!(frame.fin(), true);
            assert_eq!(frame.opcode(), 1);
            assert_eq!(frame.raw(), [129, 140, 211, 25, 248, 86, 72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 33]);
            let expected_mask: &[u8] = &[211, 25, 248, 86];
            assert_eq!(frame.mask(), Some(expected_mask));
            assert!(frame.is_text());
            assert_eq!(frame.payload(), b"Hello world!");
            assert!(surplus.is_empty());
        } else {
            // because data contains full frame
            assert!(false);
        }
    } else {
        assert!(false);
    }
}

#[test]
fn parse_two_good_frame_and_surplus() {
    let incoming_data = [129, 131, 216, 213, 165, 109, 233, 231, 150];
    let mut parser = Parser::new();
    if let Ok(result) = parser.push(&incoming_data, 100) {
        if let Some((frame, surplus)) = result {
            assert_eq!(frame.fin(), true);
            assert_eq!(frame.opcode(), 1);
            assert_eq!(frame.raw(), [129, 131, 216, 213, 165, 109, 49, 50, 51]);
            let expected_mask: &[u8] = &[216, 213, 165, 109];
            assert_eq!(frame.mask(), Some(expected_mask));
            assert_eq!(frame.payload(), b"123");
            assert!(surplus.is_empty());

            let incoming_data = [129, 134, 6, 145, 169, 18, 103, 243, 202, 118, 99, 247, 129, 137];
            if let Ok(result) = parser.push(&incoming_data, 100) {
                if let Some((frame, surplus)) = result {
                    assert_eq!(frame.fin(), true);
                    assert_eq!(frame.opcode(), 1);
                    assert_eq!(frame.raw(), [129, 134, 6, 145, 169, 18, 97, 98, 99, 100, 101, 102]);
                    let expected_mask: &[u8] = &[6, 145, 169, 18];
                    assert_eq!(frame.mask(), Some(expected_mask));
                    assert_eq!(frame.payload(), b"abcdef");
                    assert_eq!(surplus, [129, 137]);
                } else {
                    // because data contains full frame
                    assert!(false);
                }
            } else {
                assert!(false);
            }
        } else {
            // because data contains full frame
            assert!(false);
        }
    } else {
        assert!(false);
    }
}

#[test]
fn parse_two_good_frame_together_and_surplus() {
    let incoming_data = [129, 131, 216, 213, 165, 109, 233, 231, 150, 129, 134, 6, 145, 169, 18, 103, 243, 202, 118, 99, 247, 129, 133];
    let mut parser = Parser::new();
    if let Ok(result) = parser.push(&incoming_data, 100) {
        if let Some((frame, surplus)) = result {
            assert_eq!(frame.fin(), true);
            assert_eq!(frame.opcode(), 1);
            assert_eq!(frame.raw(), [129, 131, 216, 213, 165, 109, 49, 50, 51]);
            let expected_mask: &[u8] = &[216, 213, 165, 109];
            assert_eq!(frame.mask(), Some(expected_mask));
            assert_eq!(frame.payload(), b"123");
            assert!(!surplus.is_empty());

            if let Ok(result) = parser.push(&surplus, 100) {
                if let Some((frame, surplus)) = result {
                    assert_eq!(frame.fin(), true);
                    assert_eq!(frame.opcode(), 1);
                    assert_eq!(frame.raw(), [129, 134, 6, 145, 169, 18, 97, 98, 99, 100, 101, 102]);
                    let expected_mask: &[u8] = &[6, 145, 169, 18];
                    assert_eq!(frame.mask(), Some(expected_mask));
                    assert_eq!(frame.payload(), b"abcdef");
                    assert_eq!(surplus, [129, 133]);
                } else {
                    // because data contains full frame
                    assert!(false);
                }
            } else {
                assert!(false);
            }
        } else {
            // because data contains full frame
            assert!(false);
        }
    } else {
        assert!(false);
    }
}

#[test]
fn parse_empty() {
    let incoming_data = [];
    let mut parser = Parser::new();
    if let Ok(result) = parser.push(&incoming_data, 100) {
        assert!(result.is_none());
    } else {
        assert!(false);
    }
}

#[test]
fn parse_part_of_frame() {
    let incoming_data = [129, 140, 211, 25, 248, 86];
    let mut parser = Parser::new();
    if let Ok(result) = parser.push(&incoming_data, 100) {
        assert!(result.is_none());
    } else {
        assert!(false);
    }
}

#[test]
fn parse_close_frame() {
    let incoming_data = [136, 130, 149, 71, 232, 208, 3, 233];
    let mut parser = Parser::new();
    if let Ok(result) = parser.push(&incoming_data, 100) {
        if let Some((frame, surplus)) = result {
            assert_eq!(frame.fin(), true);
            assert_eq!(frame.opcode(), 8);
            assert!(frame.is_close());
            assert!(surplus.is_empty());
        } else {
            // because data contains full frame
            assert!(false);
        }
    } else {
        assert!(false);
    }
}

#[test]
fn make_no_masked_frame_for_send() {
    assert_eq!(frame(TEXT_OPCODE, &[]), [129, 0]);
    assert_eq!(frame(BINARY_OPCODE, &[]), [130, 0]);
    assert_eq!(frame(TEXT_OPCODE, b"1"), [129, 1, 49]);
    assert_eq!(frame(BINARY_OPCODE, b"1"), [130, 1, 49]);
    assert_eq!(frame(TEXT_OPCODE, b"abcdef"), [129, 6, 97, 98, 99, 100, 101, 102]);
    assert_eq!(frame(BINARY_OPCODE, b"abcdef"), [130, 6, 97, 98, 99, 100, 101, 102]);
}

#[test]
fn payload_len_limit() {
    let incoming_data = [129, 140, 211, 25, 248, 86, 155, 124, 148, 58, 188, 57, 143, 57, 161, 117, 156, 119];
    let mut parser = Parser::new();
    if let Err(_) = parser.push(&incoming_data, 11) {
        assert!(true);
    }
}
