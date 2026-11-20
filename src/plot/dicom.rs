//! DICOM slices (CT, MRI, PET) for volume rendering: a directory of
//! single-frame files, or one multi-frame file, becomes a
//! [`VolumeData`] in patient millimetres with the rescale (e.g.
//! Hounsfield units) applied.
//!
//! Uncompressed transfer syntaxes are read (implicit and explicit VR,
//! little and big endian: what scanners and PACS exports mostly use);
//! compressed pixel data (JPEG, JPEG 2000, RLE) is reported as an error.

use super::volume_file::VolumeData;
use std::path::Path;

/// One DICOM image: its pixels and where it lies.
#[derive(Clone, Debug, Default)]
pub struct Slice {
    /// Rows × columns values (row-major), rescaled.
    pub values: Vec<f32>,
    /// Columns (x) and rows (y).
    pub size: [usize; 2],
    /// Frames in the file (multi-frame images; `values` holds all).
    pub frames: usize,
    /// Pixel spacing (x, y) in mm.
    pub spacing: [f64; 2],
    /// Slice thickness in mm (fallback for the slice spacing).
    pub thickness: Option<f64>,
    /// Image position (patient) of the first pixel in mm.
    pub position: Option<[f64; 3]>,
    /// Instance number (order without positions).
    pub instance: Option<i64>,
}

const EXPLICIT_LE: &str = "1.2.840.10008.1.2.1";
const IMPLICIT_LE: &str = "1.2.840.10008.1.2";
const EXPLICIT_BE: &str = "1.2.840.10008.1.2.2";
const DEFLATED: &str = "1.2.840.10008.1.2.1.99";

/// VRs with a 2-byte reserved field and a 4-byte length in explicit VR.
fn long_vr(vr: &[u8]) -> bool {
    matches!(
        vr,
        b"OB"
            | b"OD"
            | b"OF"
            | b"OL"
            | b"OV"
            | b"OW"
            | b"SQ"
            | b"SV"
            | b"UC"
            | b"UN"
            | b"UR"
            | b"UT"
            | b"UV"
    )
}

struct Parser<'a> {
    b: &'a [u8],
    pos: usize,
    explicit: bool,
    big: bool,
}

/// A data element: tag, VR (explicit only) and value (None: undefined
/// length, i.e. a sequence or encapsulated pixel data).
struct Element<'a> {
    tag: (u16, u16),
    value: Option<&'a [u8]>,
}

impl<'a> Parser<'a> {
    fn u16(&self, at: usize) -> Option<u16> {
        let a: [u8; 2] = self.b.get(at..at + 2)?.try_into().ok()?;
        Some(if self.big {
            u16::from_be_bytes(a)
        } else {
            u16::from_le_bytes(a)
        })
    }
    fn u32(&self, at: usize) -> Option<u32> {
        let a: [u8; 4] = self.b.get(at..at + 4)?.try_into().ok()?;
        Some(if self.big {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        })
    }

    fn next(&mut self) -> Option<Element<'a>> {
        let p = self.pos;
        let tag = (self.u16(p)?, self.u16(p + 2)?);
        // Item and delimitation tags have no VR.
        let (len, head) = if tag.0 == 0xFFFE {
            (self.u32(p + 4)?, 8)
        } else if self.explicit {
            let vr = self.b.get(p + 4..p + 6)?;
            if long_vr(vr) {
                (self.u32(p + 8)?, 12)
            } else {
                (self.u16(p + 6)? as u32, 8)
            }
        } else {
            (self.u32(p + 4)?, 8)
        };
        let start = p + head;
        if len == 0xFFFF_FFFF {
            self.pos = start;
            return Some(Element { tag, value: None });
        }
        let end = start.checked_add(len as usize)?;
        let value = self.b.get(start..end)?;
        self.pos = end;
        Some(Element {
            tag,
            value: Some(value),
        })
    }

    /// Skips the contents of an undefined-length element (to its
    /// sequence delimitation item), nested sequences included.
    fn skip_undefined(&mut self) -> Option<()> {
        let mut depth = 1usize;
        while depth > 0 {
            let e = self.next()?;
            match (e.tag, e.value) {
                ((0xFFFE, 0xE0DD), _) => depth -= 1,
                // An item of undefined length: its contents follow.
                ((0xFFFE, 0xE000), None) => depth += 1,
                ((0xFFFE, 0xE00D), _) => depth -= 1,
                (_, None) => depth += 1,
                _ => {}
            }
        }
        Some(())
    }
}

fn text(v: &[u8]) -> String {
    String::from_utf8_lossy(v)
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string()
}

fn decimals(v: &[u8]) -> Vec<f64> {
    text(v)
        .split('\\')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

impl Slice {
    /// Parses one DICOM file (with the `DICM` preamble, or a bare dataset
    /// in implicit VR little endian).
    pub fn parse(bytes: &[u8]) -> Result<Slice, String> {
        let (mut pos, mut syntax) = (0, IMPLICIT_LE.to_string());
        if bytes.get(128..132) == Some(b"DICM") {
            // File meta information: always explicit VR little endian.
            let mut meta = Parser {
                b: bytes,
                pos: 132,
                explicit: true,
                big: false,
            };
            loop {
                let save = meta.pos;
                match meta.u16(save) {
                    Some(2) => {}
                    _ => break,
                }
                let e = meta
                    .next()
                    .ok_or("DICOM: truncated file meta information")?;
                if e.tag == (0x0002, 0x0010) {
                    syntax = text(e.value.unwrap_or_default());
                }
            }
            pos = meta.pos;
        }
        let (explicit, big) = match syntax.as_str() {
            EXPLICIT_LE => (true, false),
            IMPLICIT_LE => (false, false),
            EXPLICIT_BE => (true, true),
            DEFLATED => return Err("DICOM: deflated datasets are not supported".into()),
            other => {
                return Err(format!(
                    "DICOM: compressed pixel data (transfer syntax {other}) is not supported; decompress first (e.g. `gdcmconv --raw`)"
                ))
            }
        };
        let mut p = Parser {
            b: bytes,
            pos,
            explicit,
            big,
        };
        let mut s = Slice {
            spacing: [1.0, 1.0],
            frames: 1,
            ..Default::default()
        };
        let (mut rows, mut cols, mut bits, mut signed) = (0usize, 0usize, 16u16, false);
        let (mut slope, mut inter) = (1.0f64, 0.0f64);
        let mut samples = 1u16;
        let mut pixels: Option<&[u8]> = None;
        while p.pos < bytes.len() {
            let Some(e) = p.next() else { break };
            let Some(v) = e.value else {
                if e.tag == (0x7FE0, 0x0010) {
                    return Err(
                        "DICOM: encapsulated (compressed) pixel data is not supported".into(),
                    );
                }
                p.skip_undefined().ok_or("DICOM: truncated sequence")?;
                continue;
            };
            let us = |v: &[u8]| -> u16 {
                let a = [
                    v.first().copied().unwrap_or(0),
                    v.get(1).copied().unwrap_or(0),
                ];
                if big {
                    u16::from_be_bytes(a)
                } else {
                    u16::from_le_bytes(a)
                }
            };
            match e.tag {
                (0x0028, 0x0010) => rows = us(v) as usize,
                (0x0028, 0x0011) => cols = us(v) as usize,
                (0x0028, 0x0002) => samples = us(v),
                (0x0028, 0x0100) => bits = us(v),
                (0x0028, 0x0103) => signed = us(v) == 1,
                (0x0028, 0x0008) => s.frames = text(v).parse().unwrap_or(1).max(1),
                (0x0028, 0x1053) => slope = decimals(v).first().copied().unwrap_or(1.0),
                (0x0028, 0x1052) => inter = decimals(v).first().copied().unwrap_or(0.0),
                (0x0028, 0x0030) => {
                    // Row spacing (y) then column spacing (x).
                    if let [r, c, ..] = decimals(v)[..] {
                        s.spacing = [c, r];
                    }
                }
                (0x0018, 0x0050) => s.thickness = decimals(v).first().copied(),
                (0x0020, 0x0032) => {
                    if let [x, y, z, ..] = decimals(v)[..] {
                        s.position = Some([x, y, z]);
                    }
                }
                (0x0020, 0x0013) => s.instance = text(v).parse().ok(),
                (0x7FE0, 0x0010) => {
                    pixels = Some(v);
                    break;
                }
                _ => {}
            }
        }
        if samples != 1 {
            return Err("DICOM: color images are not volumes".into());
        }
        let px = pixels.ok_or("DICOM: no pixel data")?;
        let n = rows
            .checked_mul(cols)
            .and_then(|n| n.checked_mul(s.frames))
            .filter(|n| *n > 0 && *n <= 1 << 30)
            .ok_or("DICOM: bad image size")?;
        let size = match bits {
            8 => 1,
            16 => 2,
            32 => 4,
            b => return Err(format!("DICOM: {b} bits per pixel is not supported")),
        };
        let raw = px
            .get(..n * size)
            .ok_or("DICOM: pixel data shorter than the image")?;
        let (sl, it) = (slope as f32, inter as f32);
        let conv = |v: f32| v * sl + it;
        s.values = match (size, signed) {
            (1, false) => raw.iter().map(|&v| conv(v as f32)).collect(),
            (1, true) => raw.iter().map(|&v| conv(v as i8 as f32)).collect(),
            (2, _) => raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| {
                    let u = if big {
                        u16::from_be_bytes([c[0], c[1]])
                    } else {
                        u16::from_le_bytes([c[0], c[1]])
                    };
                    conv(if signed { u as i16 as f32 } else { u as f32 })
                })
                .collect(),
            _ => raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| {
                    let a = [c[0], c[1], c[2], c[3]];
                    let u = if big {
                        u32::from_be_bytes(a)
                    } else {
                        u32::from_le_bytes(a)
                    };
                    conv(if signed { u as i32 as f32 } else { u as f32 })
                })
                .collect(),
        };
        s.size = [cols, rows];
        Ok(s)
    }
}

impl VolumeData {
    /// A volume from DICOM: a directory of slices (every readable image
    /// file in it; sorted by position along the slice normal, else by
    /// instance number) or a single multi-frame file.
    pub fn from_dicom(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if path.is_file() {
            let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let s = Slice::parse(&bytes)?;
            let dz = s.thickness.filter(|t| *t > 0.0).unwrap_or(1.0);
            return Ok(VolumeData {
                dims: [s.size[0], s.size[1], s.frames],
                spacing: [s.spacing[0], s.spacing[1], dz],
                origin: s.position.unwrap_or_default(),
                values: s.values,
            });
        }
        let mut files: Vec<_> = std::fs::read_dir(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        let mut slices: Vec<Slice> = files
            .iter()
            .filter_map(|f| std::fs::read(f).ok())
            .filter_map(|b| Slice::parse(&b).ok())
            .collect();
        let first = slices
            .first()
            .ok_or_else(|| format!("{}: no readable DICOM images", path.display()))?;
        let size = first.size;
        slices.retain(|s| s.size == size && s.frames == 1);
        // Sort along the axis in which the positions change most.
        let axis = (0..3)
            .max_by(|&a, &b| {
                let spread = |k: usize| {
                    let v = slices.iter().filter_map(|s| s.position.map(|p| p[k]));
                    let (lo, hi) = v.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                        (a.min(x), b.max(x))
                    });
                    hi - lo
                };
                spread(a).total_cmp(&spread(b))
            })
            .unwrap_or(2);
        if slices.iter().all(|s| s.position.is_some()) {
            slices.sort_by(|a, b| {
                a.position.unwrap_or_default()[axis]
                    .total_cmp(&b.position.unwrap_or_default()[axis])
            });
        } else {
            slices.sort_by_key(|s| s.instance.unwrap_or(0));
        }
        let n = slices.len();
        let dz = match (
            slices.first().and_then(|s| s.position),
            slices.last().and_then(|s| s.position),
        ) {
            (Some(a), Some(b)) if n > 1 && a[axis] != b[axis] => {
                ((b[axis] - a[axis]) / (n - 1) as f64).abs()
            }
            _ => slices[0].thickness.filter(|t| *t > 0.0).unwrap_or(1.0),
        };
        let mut values = Vec::with_capacity(size[0] * size[1] * n);
        for s in &slices {
            values.extend_from_slice(&s.values);
        }
        Ok(VolumeData {
            dims: [size[0], size[1], n],
            spacing: [slices[0].spacing[0], slices[0].spacing[1], dz],
            origin: slices[0].position.unwrap_or_default(),
            values,
        })
    }
}

/// A minimal DICOM file (explicit VR little endian, 16-bit signed) for
/// tests and examples.
#[doc(hidden)]
pub fn write_test_slice(
    rows: u16,
    cols: u16,
    z: f64,
    pixels: &[i16],
    slope: f64,
    inter: f64,
) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b.extend(b"DICM");
    let el = |b: &mut Vec<u8>, g: u16, e: u16, vr: &[u8; 2], v: &[u8]| {
        b.extend(g.to_le_bytes());
        b.extend(e.to_le_bytes());
        b.extend(vr);
        let mut v = v.to_vec();
        if v.len() % 2 == 1 {
            v.push(if vr == b"UI" || vr == b"OB" { 0 } else { b' ' });
        }
        if long_vr(vr) {
            b.extend([0, 0]);
            b.extend((v.len() as u32).to_le_bytes());
        } else {
            b.extend((v.len() as u16).to_le_bytes());
        }
        b.extend(v);
    };
    el(&mut b, 0x0002, 0x0010, b"UI", EXPLICIT_LE.as_bytes());
    // A sequence of undefined length to skip.
    b.extend([0x08, 0x00, 0x15, 0x11]);
    b.extend(b"SQ");
    b.extend([0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
    b.extend([0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF]);
    el(&mut b, 0x0008, 0x1150, b"UI", b"1.2.3");
    b.extend([0xFE, 0xFF, 0x0D, 0xE0, 0, 0, 0, 0]);
    b.extend([0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0]);
    el(&mut b, 0x0018, 0x0050, b"DS", b"2.5");
    el(
        &mut b,
        0x0020,
        0x0013,
        b"IS",
        format!("{}", z as i64).as_bytes(),
    );
    el(
        &mut b,
        0x0020,
        0x0032,
        b"DS",
        format!("-10\\-20\\{z}").as_bytes(),
    );
    el(&mut b, 0x0028, 0x0002, b"US", &1u16.to_le_bytes());
    el(&mut b, 0x0028, 0x0010, b"US", &rows.to_le_bytes());
    el(&mut b, 0x0028, 0x0011, b"US", &cols.to_le_bytes());
    el(&mut b, 0x0028, 0x0030, b"DS", b"0.5\\0.75");
    el(&mut b, 0x0028, 0x0100, b"US", &16u16.to_le_bytes());
    el(&mut b, 0x0028, 0x0103, b"US", &1u16.to_le_bytes());
    el(&mut b, 0x0028, 0x1052, b"DS", format!("{inter}").as_bytes());
    el(&mut b, 0x0028, 0x1053, b"DS", format!("{slope}").as_bytes());
    let px: Vec<u8> = pixels.iter().flat_map(|v| v.to_le_bytes()).collect();
    el(&mut b, 0x7FE0, 0x0010, b"OW", &px);
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_stack_into_a_volume_sorted_by_position() {
        let dir = std::env::temp_dir().join(format!("dataviz-dicom-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Written out of order; z = 0, 3, 6.
        for (name, z) in [("b.dcm", 6.0), ("a.dcm", 0.0), ("c.dcm", 3.0)] {
            let px: Vec<i16> = (0..6).map(|i| i as i16 + z as i16 * 10).collect();
            std::fs::write(dir.join(name), write_test_slice(2, 3, z, &px, 2.0, -1024.0)).unwrap();
        }
        std::fs::write(dir.join("notes.txt"), "not dicom").unwrap();
        let v = VolumeData::from_dicom(&dir).unwrap();
        assert_eq!(v.dims, [3, 2, 3]);
        assert_eq!(v.spacing, [0.75, 0.5, 3.0]);
        assert_eq!(v.origin, [-10.0, -20.0, 0.0]);
        // Rescaled: value * 2 - 1024; slice z = 3 is in the middle.
        assert_eq!(v.values[0], -1024.0);
        assert_eq!(v.values[6], (30.0 * 2.0) - 1024.0);
        assert_eq!(v.values[17], (65.0 * 2.0) - 1024.0);
        let one = VolumeData::from_dicom(dir.join("a.dcm")).unwrap();
        assert_eq!(one.dims, [3, 2, 1]);
    }

    #[test]
    fn errors_instead_of_panics() {
        let ok = write_test_slice(2, 2, 0.0, &[1, 2, 3, 4], 1.0, 0.0);
        assert!(Slice::parse(&ok).is_ok());
        for cut in [0, 50, 132, 140, 200, ok.len() - 3] {
            let _ = Slice::parse(&ok[..cut.min(ok.len())]);
        }
        assert!(Slice::parse(b"").is_err());
    }
}
