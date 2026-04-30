//! T1.0.6.31 — icon asset contract tests.
//!
//! The app and tray icons are release artifacts, not decoration. These
//! tests keep placeholder PNGs and single-layer Windows ICO files from
//! slipping back into the bundle.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn icons_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("icons")
}

fn png_dimensions(path: &Path) -> (u32, u32, u8, u8) {
    let bytes = fs::read(path).expect("PNG should be readable");
    assert!(
        bytes.len() > 256,
        "{} should not be a tiny placeholder",
        path.display()
    );
    assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&bytes[12..16], b"IHDR");
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("width bytes"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("height bytes"));
    let bit_depth = bytes[24];
    let color_type = bytes[25];
    (width, height, bit_depth, color_type)
}

#[test]
fn png_icon_assets_have_real_dimensions_and_alpha() {
    let icons = icons_dir();
    for (name, expected) in [
        ("32x32.png", (32, 32)),
        ("128x128.png", (128, 128)),
        ("128x128@2x.png", (256, 256)),
        ("tray-template.png", (64, 64)),
    ] {
        let (width, height, bit_depth, color_type) = png_dimensions(&icons.join(name));
        assert_eq!((width, height), expected, "{name} dimensions drifted");
        assert_eq!(bit_depth, 8, "{name} must be 8-bit per channel");
        assert_eq!(color_type, 6, "{name} must be RGBA");
    }
}

#[test]
fn windows_ico_contains_expected_multi_size_png_layers() {
    let bytes = fs::read(icons_dir().join("icon.ico")).expect("icon.ico should be readable");
    assert!(bytes.len() > 4096, "icon.ico should not be a placeholder");
    assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0);
    assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 1);

    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    assert_eq!(count, 7, "icon.ico should contain seven image layers");

    let mut sizes = BTreeSet::new();
    for index in 0..count {
        let offset = 6 + index * 16;
        let width = match bytes[offset] {
            0 => 256,
            value => u16::from(value),
        };
        let height = match bytes[offset + 1] {
            0 => 256,
            value => u16::from(value),
        };
        let bit_count = u16::from_le_bytes([bytes[offset + 6], bytes[offset + 7]]);
        let image_len = u32::from_le_bytes(
            bytes[offset + 8..offset + 12]
                .try_into()
                .expect("image length bytes"),
        ) as usize;
        let image_offset = u32::from_le_bytes(
            bytes[offset + 12..offset + 16]
                .try_into()
                .expect("image offset bytes"),
        ) as usize;

        assert_eq!(width, height, "ICO layer must be square");
        assert_eq!(bit_count, 32, "ICO layer must be 32-bit RGBA");
        assert!(
            image_offset + image_len <= bytes.len(),
            "ICO layer points outside file"
        );
        assert_eq!(
            &bytes[image_offset..image_offset + 8],
            b"\x89PNG\r\n\x1a\n",
            "ICO layer should store PNG data"
        );
        sizes.insert(width);
    }

    assert_eq!(
        sizes,
        BTreeSet::from([16, 24, 32, 48, 64, 128, 256]),
        "ICO layer sizes changed"
    );
}

#[test]
fn macos_icns_contains_expected_png_layers() {
    let bytes = fs::read(icons_dir().join("icon.icns")).expect("icon.icns should be readable");
    assert!(bytes.len() > 4096, "icon.icns should not be a placeholder");
    assert_eq!(&bytes[0..4], b"icns");
    let total_len = u32::from_be_bytes(bytes[4..8].try_into().expect("icns length bytes"));
    assert_eq!(
        total_len as usize,
        bytes.len(),
        "ICNS length header drifted"
    );

    let mut offset = 8;
    let mut types = BTreeSet::new();
    while offset < bytes.len() {
        let icon_type = std::str::from_utf8(&bytes[offset..offset + 4]).expect("ASCII icon type");
        let entry_len = u32::from_be_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .expect("entry length bytes"),
        ) as usize;
        assert!(entry_len > 8, "ICNS entry {icon_type} should contain data");
        assert!(
            offset + entry_len <= bytes.len(),
            "ICNS entry {icon_type} points outside file"
        );
        assert_eq!(
            &bytes[offset + 8..offset + 16],
            b"\x89PNG\r\n\x1a\n",
            "ICNS entry {icon_type} should store PNG data"
        );
        types.insert(icon_type.to_owned());
        offset += entry_len;
    }

    assert_eq!(
        types,
        BTreeSet::from([
            "ic07".to_owned(),
            "ic08".to_owned(),
            "ic09".to_owned(),
            "ic10".to_owned(),
            "icp4".to_owned(),
            "icp5".to_owned(),
            "icp6".to_owned(),
        ]),
        "ICNS layer types changed"
    );
}
