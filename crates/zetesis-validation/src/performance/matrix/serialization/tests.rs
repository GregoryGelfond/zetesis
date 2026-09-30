//! Stream views preserve every byte without changing the legacy capture view.
use super::*;
use serde_json::{Value, json};

fn decoded(value: &Value) -> Vec<u8> {
    match value["encoding"].as_str().unwrap() {
        "utf8" => value["data"].as_str().unwrap().as_bytes().to_vec(),
        "bytes" => serde_json::from_value(value["data"].clone()).unwrap(),
        other => panic!("unknown stream encoding {other}"),
    }
}

#[test]
fn every_two_byte_stream_round_trips_exactly() {
    for first in u8::MIN..=u8::MAX {
        for second in u8::MIN..=u8::MAX {
            let bytes = [first, second];
            let encoded = serde_json::to_vec(&Stream::new(&bytes)).unwrap();
            let value: Value = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(decoded(&value), bytes);
        }
    }
}

#[test]
fn utf8_controls_preserve_their_original_bytes() {
    for text in ["", "\0", "\t\n\r\u{1b}", "ζήτησις", "𝄞", "\"\\"] {
        let encoded = serde_json::to_vec(&Stream::new(text.as_bytes())).unwrap();
        let value: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["encoding"], "utf8");
        assert_eq!(decoded(&value), text.as_bytes());
    }
}

#[test]
fn malformed_utf8_uses_an_explicit_byte_view() {
    let bytes = [b'a', 0xf0, 0x80, 0x80, b'\0'];
    let value = serde_json::to_value(Stream::new(&bytes)).unwrap();
    assert_eq!(value, json!({"encoding":"bytes","data":bytes}));
}

#[test]
fn matrix_capture_view_preserves_invocation_metadata() {
    for helper_child_id in [None, Some(17)] {
        let capture = Capture {
            helper_child_id,
            executable: "/sealed/zetesis".into(),
            arguments: vec!["--json".into()],
            directory: "/private/source".into(),
            started_unix_ns: Some(7),
            elapsed_ns: Some(11),
            stop: Some(process::Stop::OutputLimit),
            exit: Some(process::Exit {
                code: None,
                signal: Some(9),
            }),
            stdout: b"prefix\0".to_vec(),
            stderr: vec![0xff],
            failure: None,
            cleanup_failure: None,
            unresolved_child: Some(13),
        };
        let mut legacy = serde_json::to_value(&capture).unwrap();
        let mut compact = serde_json::to_value(View::new(&capture)).unwrap();
        assert_eq!(decoded(&compact["stdout"]), capture.stdout());
        assert_eq!(decoded(&compact["stderr"]), capture.stderr());
        for key in ["stdout", "stderr"] {
            assert!(legacy[key].is_array());
            legacy.as_object_mut().unwrap().remove(key);
            compact.as_object_mut().unwrap().remove(key);
        }
        assert_eq!(legacy, compact);
    }
}

#[test]
fn a_matrix_capture_serializes_as_its_stream_view() {
    let capture = Capture {
        helper_child_id: None,
        executable: "/sealed/zetesis".into(),
        arguments: vec!["--json".into()],
        directory: "/private/source".into(),
        started_unix_ns: Some(7),
        elapsed_ns: Some(11),
        stop: Some(process::Stop::Completed),
        exit: Some(process::Exit {
            code: Some(0),
            signal: None,
        }),
        stdout: b"text".to_vec(),
        stderr: vec![0xff],
        failure: None,
        cleanup_failure: None,
        unresolved_child: None,
    };
    let view = serde_json::to_value(View::new(&capture)).unwrap();
    assert_eq!(serde_json::to_value(MatrixCapture(capture)).unwrap(), view);
}
