//! Volume files for [`Axes::volume_data`]: NIfTI-1/NIfTI-2 (`.nii`,
//! `.nii.gz`: MRI, CT, PET, fMRI), VTK legacy structured points and image
//! data (`.vtk`: simulations, ParaView exports) and MetaImage (`.mhd`/`.mha`:
//! ITK, 3D Slicer). Values are kept as `f32` with their voxel spacing and
//! origin, so scans keep their physical proportions.
//!
//! ```no_run
//! use dataviz::prelude::*;
//! use dataviz::plot::volume_file::VolumeData;
//! let scan = VolumeData::read("brain.nii.gz").unwrap();
//! let mut fig = Figure::new(800, 700);
//! fig.axes().volume_data(&scan).colormap(Colormap::Gray);
//! fig.save("brain.png").unwrap();
//! ```

use super::axes::Axes;
use super::series::volume::Volume;
use std::path::Path;

/// A 3D grid of values with its geometry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VolumeData {
    /// `values[x + nx·(y + ny·z)]` (x fastest).
    pub values: Vec<f32>,
    /// `[nx, ny, nz]`.
    pub dims: [usize; 3],
    /// Distance between voxel centers along x, y, z (e.g. mm).
    pub spacing: [f64; 3],
    /// Position of the first voxel.
    pub origin: [f64; 3],
}

/// Largest grid read (2³⁰ voxels, 4 GB as f32).
const MAX_VOXELS: usize = 1 << 30;

impl VolumeData {
    /// Reads a file by extension: `.nii`, `.nii.gz`, `.vtk`, `.mhd`
    /// (with its raw data file), `.mha`.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if name.ends_with(".nii") || name.ends_with(".nii.gz") || name.ends_with(".gz") {
            Self::from_nifti(&bytes)
        } else if name.ends_with(".vtk") {
            Self::from_vtk(&bytes)
        } else if name.ends_with(".mhd") || name.ends_with(".mha") {
            Self::from_metaimage(&bytes, path.parent())
        } else {
            Err(format!(
                "{}: unknown volume format (.nii, .nii.gz, .vtk, .mhd, .mha)",
                path.display()
            ))
        }
    }

    /// The x, y and z ranges in physical units (for `Volume::extent`).
    pub fn extent(&self) -> [(f64, f64); 3] {
        std::array::from_fn(|i| {
            let n = self.dims[i].saturating_sub(1).max(1) as f64;
            let s = if self.spacing[i].is_finite() && self.spacing[i] != 0.0 {
                self.spacing[i].abs()
            } else {
                1.0
            };
            (self.origin[i], self.origin[i] + s * n)
        })
    }

    /// Range of the finite values.
    pub fn value_range(&self) -> Option<(f64, f64)> {
        let (lo, hi) = self
            .values
            .iter()
            .filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        (lo <= hi).then_some((lo as f64, hi as f64))
    }

    /// NIfTI-1 or NIfTI-2, uncompressed or gzip-compressed (`gzip`
    /// feature), either byte order. The scaling `scl_slope`/`scl_inter` is
    /// applied; 4D files give their first volume.
    pub fn from_nifti(bytes: &[u8]) -> Result<Self, String> {
        let data = gunzip(bytes)?;
        let b: &[u8] = &data;
        let le = |n: usize| b.get(..n).is_some();
        if !le(4) {
            return Err("NIfTI: file too short".into());
        }
        // sizeof_hdr is 348 (NIfTI-1) or 540 (NIfTI-2), in either order.
        let raw = [b[0], b[1], b[2], b[3]];
        let (v2, big) = match (i32::from_le_bytes(raw), i32::from_be_bytes(raw)) {
            (348, _) => (false, false),
            (_, 348) => (false, true),
            (540, _) => (true, false),
            (_, 540) => (true, true),
            _ => return Err("NIfTI: not a NIfTI-1/2 header".into()),
        };
        let r = Reader { b, big };
        let (dims, datatype, pixdim, offset, slope, inter, origin) = if v2 {
            let dims: Vec<i64> = (0..8)
                .map(|i| r.i64(16 + 8 * i))
                .collect::<Option<_>>()
                .ok_or("NIfTI: short header")?;
            let pix: Vec<f64> = (0..8)
                .map(|i| r.f64(104 + 8 * i))
                .collect::<Option<_>>()
                .ok_or("NIfTI: short header")?;
            (
                dims,
                r.i16(12).ok_or("NIfTI: short header")?,
                pix,
                r.i64(168).ok_or("NIfTI: short header")? as f64,
                r.f64(176).unwrap_or(1.0),
                r.f64(184).unwrap_or(0.0),
                [r.f64(392 + 24), r.f64(392 + 32 + 24), r.f64(392 + 64 + 24)],
            )
        } else {
            let dims: Vec<i64> = (0..8)
                .map(|i| r.i16(40 + 2 * i).map(i64::from))
                .collect::<Option<_>>()
                .ok_or("NIfTI: short header")?;
            let pix: Vec<f64> = (0..8)
                .map(|i| r.f32(76 + 4 * i).map(f64::from))
                .collect::<Option<_>>()
                .ok_or("NIfTI: short header")?;
            (
                dims,
                r.i16(70).ok_or("NIfTI: short header")?,
                pix,
                r.f32(108).ok_or("NIfTI: short header")? as f64,
                r.f32(112).map_or(1.0, f64::from),
                r.f32(116).map_or(0.0, f64::from),
                // qoffset_x/y/z.
                [
                    r.f32(268).map(f64::from),
                    r.f32(272).map(f64::from),
                    r.f32(276).map(f64::from),
                ],
            )
        };
        let ndim = dims[0].clamp(1, 7) as usize;
        let n = |i: usize| -> usize {
            if i <= ndim {
                dims[i].max(1) as usize
            } else {
                1
            }
        };
        let grid = [n(1), n(2), n(3)];
        let count = checked_count(grid)?;
        let ty = match datatype {
            2 => Ty::U8,
            4 => Ty::I16,
            8 => Ty::I32,
            16 => Ty::F32,
            64 => Ty::F64,
            256 => Ty::I8,
            512 => Ty::U16,
            768 => Ty::U32,
            1024 => Ty::I64,
            1280 => Ty::U64,
            t => return Err(format!("NIfTI: data type {t} is not supported")),
        };
        let start = offset.max(if v2 { 544.0 } else { 352.0 }) as usize;
        let mut values = r
            .values(start, count, ty)
            .ok_or("NIfTI: data shorter than the header says")?;
        // scl_slope = 0 means no scaling.
        if slope.is_finite() && slope != 0.0 && (slope != 1.0 || inter != 0.0) {
            let (s, i) = (
                slope as f32,
                if inter.is_finite() { inter as f32 } else { 0.0 },
            );
            values.iter_mut().for_each(|v| *v = *v * s + i);
        }
        let sp = |i: usize| {
            let p = pixdim.get(i).copied().unwrap_or(1.0);
            if p.is_finite() && p > 0.0 {
                p
            } else {
                1.0
            }
        };
        Ok(Self {
            values,
            dims: grid,
            spacing: [sp(1), sp(2), sp(3)],
            origin: origin.map(|o| o.filter(|v| v.is_finite()).unwrap_or(0.0)),
        })
    }

    /// VTK legacy file with `DATASET STRUCTURED_POINTS` (ASCII or BINARY)
    /// and point scalars (the first `SCALARS` array).
    pub fn from_vtk(bytes: &[u8]) -> Result<Self, String> {
        let mut pos = 0;
        let mut line = || -> Option<&[u8]> {
            if pos >= bytes.len() {
                return None;
            }
            let end = bytes[pos..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(bytes.len(), |e| pos + e);
            let l = &bytes[pos..end];
            pos = end + 1;
            Some(l)
        };
        let head = line().ok_or("VTK: empty file")?;
        if !head.starts_with(b"# vtk DataFile") {
            return Err("VTK: not a legacy VTK file".into());
        }
        let _title = line();
        let format = String::from_utf8_lossy(line().ok_or("VTK: no format line")?)
            .trim()
            .to_ascii_uppercase();
        let binary = format == "BINARY";
        let mut v = VolumeData {
            spacing: [1.0; 3],
            ..Default::default()
        };
        let mut ty = None;
        let mut components = 1usize;
        let mut dataset = false;
        while let Some(l) = line() {
            let text = String::from_utf8_lossy(l);
            let mut words = text.split_whitespace();
            let Some(key) = words.next().map(str::to_ascii_uppercase) else {
                continue;
            };
            let nums: Vec<f64> = words.clone().filter_map(|w| w.parse().ok()).collect();
            match key.as_str() {
                "DATASET" => {
                    let kind = words.next().unwrap_or("").to_ascii_uppercase();
                    if kind != "STRUCTURED_POINTS" {
                        return Err(format!(
                            "VTK: dataset {kind} is not a regular grid (STRUCTURED_POINTS)"
                        ));
                    }
                    dataset = true;
                }
                "DIMENSIONS" if nums.len() >= 3 => {
                    v.dims = [0, 1, 2].map(|i| nums[i].clamp(1.0, 1e9) as usize);
                }
                "SPACING" | "ASPECT_RATIO" if nums.len() >= 3 => {
                    v.spacing = [nums[0], nums[1], nums[2]]
                }
                "ORIGIN" if nums.len() >= 3 => v.origin = [nums[0], nums[1], nums[2]],
                "SCALARS" => {
                    let t = words.nth(1).unwrap_or("float").to_ascii_lowercase();
                    components = words
                        .next()
                        .and_then(|c| c.parse().ok())
                        .unwrap_or(1)
                        .clamp(1, 4);
                    ty = Some(match t.as_str() {
                        "unsigned_char" | "bit" => Ty::U8,
                        "char" => Ty::I8,
                        "short" => Ty::I16,
                        "unsigned_short" => Ty::U16,
                        "int" => Ty::I32,
                        "unsigned_int" => Ty::U32,
                        "long" | "vtktypeint64" => Ty::I64,
                        "unsigned_long" | "vtktypeuint64" => Ty::U64,
                        "double" => Ty::F64,
                        _ => Ty::F32,
                    });
                }
                "LOOKUP_TABLE" if ty.is_some() => break,
                _ => {}
            }
        }
        if !dataset {
            return Err("VTK: no DATASET STRUCTURED_POINTS".into());
        }
        let ty = ty.ok_or("VTK: no SCALARS point data")?;
        let count = checked_count(v.dims)?;
        let total = count.checked_mul(components).ok_or("VTK: grid too large")?;
        let all = if binary {
            // Legacy binary data is big-endian.
            Reader {
                b: bytes,
                big: true,
            }
            .values(pos, total, ty)
            .ok_or("VTK: data shorter than DIMENSIONS say")?
        } else {
            let text = String::from_utf8_lossy(&bytes[pos.min(bytes.len())..]);
            let vals: Vec<f32> = text
                .split_ascii_whitespace()
                .take(total)
                .map(|w| w.parse::<f32>().unwrap_or(f32::NAN))
                .collect();
            if vals.len() < total {
                return Err("VTK: fewer values than DIMENSIONS say".into());
            }
            vals
        };
        // Several components: their magnitude.
        v.values = if components == 1 {
            all
        } else {
            all.chunks_exact(components)
                .map(|c| c.iter().map(|x| x * x).sum::<f32>().sqrt())
                .collect()
        };
        Ok(v)
    }

    /// MetaImage header (`.mhd` with `ElementDataFile` next to it, or
    /// `.mha` with the data after the header; zlib-compressed data needs
    /// the `gzip` feature).
    pub fn from_metaimage(bytes: &[u8], dir: Option<&Path>) -> Result<Self, String> {
        let mut v = VolumeData {
            spacing: [1.0; 3],
            ..Default::default()
        };
        let (mut ty, mut big, mut compressed, mut file) = (None, false, false, None);
        let mut pos = 0;
        let mut ndims = 3;
        while pos < bytes.len() {
            let end = bytes[pos..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(bytes.len(), |e| pos + e);
            let l = String::from_utf8_lossy(&bytes[pos..end]).into_owned();
            pos = end + 1;
            let Some((k, val)) = l.split_once('=') else {
                continue;
            };
            let (k, val) = (k.trim(), val.trim());
            let nums: Vec<f64> = val
                .split_whitespace()
                .filter_map(|w| w.parse().ok())
                .collect();
            match k {
                "NDims" => ndims = nums.first().map_or(3, |n| *n as usize),
                "DimSize" => {
                    v.dims =
                        [0, 1, 2].map(|i| nums.get(i).map_or(1, |n| n.clamp(1.0, 1e9) as usize));
                }
                "ElementSpacing" | "ElementSize" => {
                    v.spacing = [0, 1, 2].map(|i| nums.get(i).copied().unwrap_or(1.0));
                }
                "Offset" | "Origin" | "Position" => {
                    v.origin = [0, 1, 2].map(|i| nums.get(i).copied().unwrap_or(0.0));
                }
                "ElementByteOrderMSB" | "BinaryDataByteOrderMSB" => {
                    big = val.eq_ignore_ascii_case("true")
                }
                "CompressedData" => compressed = val.eq_ignore_ascii_case("true"),
                "ElementType" => {
                    ty = Some(match val {
                        "MET_UCHAR" => Ty::U8,
                        "MET_CHAR" => Ty::I8,
                        "MET_SHORT" => Ty::I16,
                        "MET_USHORT" => Ty::U16,
                        "MET_INT" => Ty::I32,
                        "MET_UINT" => Ty::U32,
                        "MET_LONG_LONG" => Ty::I64,
                        "MET_ULONG_LONG" => Ty::U64,
                        "MET_DOUBLE" => Ty::F64,
                        "MET_FLOAT" => Ty::F32,
                        t => return Err(format!("MetaImage: element type {t} is not supported")),
                    })
                }
                "ElementDataFile" => {
                    file = Some(val.to_string());
                    break;
                }
                _ => {}
            }
        }
        if ndims == 2 {
            v.dims[2] = 1;
        }
        let ty = ty.ok_or("MetaImage: no ElementType")?;
        let file = file.ok_or("MetaImage: no ElementDataFile")?;
        let external;
        let raw: &[u8] = if file.eq_ignore_ascii_case("LOCAL") {
            &bytes[pos.min(bytes.len())..]
        } else {
            let p = dir.map_or_else(|| Path::new(&file).to_path_buf(), |d| d.join(&file));
            external = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            &external
        };
        let inflated;
        let raw = if compressed {
            inflated = inflate_zlib(raw)?;
            &inflated[..]
        } else {
            raw
        };
        let count = checked_count(v.dims)?;
        v.values = Reader { b: raw, big }
            .values(0, count, ty)
            .ok_or("MetaImage: data shorter than DimSize says")?;
        Ok(v)
    }

    /// The volume as a NIfTI-1 file (f32, little-endian), e.g. to save a
    /// computed field for other tools.
    pub fn to_nifti(&self) -> Vec<u8> {
        let mut h = vec![0u8; 352];
        let put = |h: &mut Vec<u8>, at: usize, b: &[u8]| h[at..at + b.len()].copy_from_slice(b);
        put(&mut h, 0, &348i32.to_le_bytes());
        let dims = [
            3i16,
            self.dims[0] as i16,
            self.dims[1] as i16,
            self.dims[2] as i16,
            1,
            1,
            1,
            1,
        ];
        for (i, d) in dims.iter().enumerate() {
            put(&mut h, 40 + 2 * i, &d.to_le_bytes());
        }
        put(&mut h, 70, &16i16.to_le_bytes());
        put(&mut h, 72, &32i16.to_le_bytes());
        let pix = [
            1.0f32,
            self.spacing[0] as f32,
            self.spacing[1] as f32,
            self.spacing[2] as f32,
            1.0,
            1.0,
            1.0,
            1.0,
        ];
        for (i, p) in pix.iter().enumerate() {
            put(&mut h, 76 + 4 * i, &p.to_le_bytes());
        }
        put(&mut h, 108, &352f32.to_le_bytes());
        put(&mut h, 112, &1f32.to_le_bytes());
        // qform: scanner coordinates from the origin.
        put(&mut h, 252, &1i16.to_le_bytes());
        for (i, o) in self.origin.iter().enumerate() {
            put(&mut h, 268 + 4 * i, &(*o as f32).to_le_bytes());
        }
        put(&mut h, 344, b"n+1\0");
        h.reserve(self.values.len() * 4);
        for v in &self.values {
            h.extend_from_slice(&v.to_le_bytes());
        }
        h
    }
}

#[derive(Clone, Copy)]
enum Ty {
    U8,
    I8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl Ty {
    fn size(self) -> usize {
        match self {
            Ty::U8 | Ty::I8 => 1,
            Ty::I16 | Ty::U16 => 2,
            Ty::I32 | Ty::U32 | Ty::F32 => 4,
            Ty::I64 | Ty::U64 | Ty::F64 => 8,
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    big: bool,
}

impl Reader<'_> {
    fn bytes<const N: usize>(&self, at: usize) -> Option<[u8; N]> {
        let mut a: [u8; N] = self.b.get(at..at.checked_add(N)?)?.try_into().ok()?;
        if self.big {
            a.reverse();
        }
        Some(a)
    }
    fn i16(&self, at: usize) -> Option<i16> {
        self.bytes(at).map(i16::from_le_bytes)
    }
    fn i64(&self, at: usize) -> Option<i64> {
        self.bytes(at).map(i64::from_le_bytes)
    }
    fn f32(&self, at: usize) -> Option<f32> {
        self.bytes(at).map(f32::from_le_bytes)
    }
    fn f64(&self, at: usize) -> Option<f64> {
        self.bytes(at).map(f64::from_le_bytes)
    }

    /// `count` values of type `ty` from byte `start`, as f32.
    fn values(&self, start: usize, count: usize, ty: Ty) -> Option<Vec<f32>> {
        let size = ty.size();
        let end = start.checked_add(count.checked_mul(size)?)?;
        let raw = self.b.get(start..end)?;
        let big = self.big;
        macro_rules! conv {
            ($t:ty, $n:literal) => {
                raw.chunks_exact($n)
                    .map(|c| {
                        let mut a: [u8; $n] = c.try_into().unwrap_or([0; $n]);
                        if big {
                            a.reverse();
                        }
                        <$t>::from_le_bytes(a) as f32
                    })
                    .collect()
            };
        }
        Some(match ty {
            Ty::U8 => raw.iter().map(|&v| v as f32).collect(),
            Ty::I8 => raw.iter().map(|&v| v as i8 as f32).collect(),
            Ty::I16 => conv!(i16, 2),
            Ty::U16 => conv!(u16, 2),
            Ty::I32 => conv!(i32, 4),
            Ty::U32 => conv!(u32, 4),
            Ty::I64 => conv!(i64, 8),
            Ty::U64 => conv!(u64, 8),
            Ty::F32 => conv!(f32, 4),
            Ty::F64 => conv!(f64, 8),
        })
    }
}

fn checked_count(dims: [usize; 3]) -> Result<usize, String> {
    dims[0]
        .checked_mul(dims[1])
        .and_then(|n| n.checked_mul(dims[2]))
        .filter(|n| *n <= MAX_VOXELS)
        .ok_or_else(|| format!("grid {}×{}×{} is too large", dims[0], dims[1], dims[2]))
}

/// The bytes, gunzipped when they start with the gzip magic number.
fn gunzip(bytes: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, String> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(bytes.into());
    }
    #[cfg(feature = "gzip")]
    {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(bytes)
            .take(MAX_VOXELS as u64 * 8 + 4096)
            .read_to_end(&mut out)
            .map_err(|e| format!("gzip: {e}"))?;
        Ok(out.into())
    }
    #[cfg(not(feature = "gzip"))]
    Err("gzip-compressed volumes need the `gzip` cargo feature".into())
}

fn inflate_zlib(bytes: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(feature = "gzip")]
    {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .take(MAX_VOXELS as u64 * 8)
            .read_to_end(&mut out)
            .map_err(|e| format!("zlib: {e}"))?;
        Ok(out)
    }
    #[cfg(not(feature = "gzip"))]
    {
        let _ = bytes;
        Err("compressed MetaImage data needs the `gzip` cargo feature".into())
    }
}

impl Axes {
    /// Volume rendering of a [`VolumeData`] (a NIfTI, VTK or MetaImage
    /// file) in its physical units.
    pub fn volume_data(&mut self, v: &VolumeData) -> &mut Volume {
        let [x, y, z] = v.extent();
        self.volume_f32(v.values.clone(), v.dims).extent(x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ball(n: usize) -> VolumeData {
        let mut values = Vec::new();
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    let c = |a: usize| a as f32 / (n - 1) as f32 - 0.5;
                    values.push((-(c(i) * c(i) + c(j) * c(j) + c(k) * c(k)) * 12.0).exp());
                }
            }
        }
        VolumeData {
            values,
            dims: [n, n, n],
            spacing: [0.5, 0.5, 2.0],
            origin: [-10.0, 0.0, 5.0],
        }
    }

    #[test]
    fn nifti_round_trip_both_byte_orders_and_scaling() {
        let v = ball(12);
        let bytes = v.to_nifti();
        assert_eq!(VolumeData::from_nifti(&bytes).unwrap(), v);
        // Big-endian i16 with scl_slope 0.5, scl_inter 1.
        let mut h = vec![0u8; 352];
        h[0..4].copy_from_slice(&348i32.to_be_bytes());
        for (i, d) in [3i16, 2, 2, 1].iter().enumerate() {
            h[40 + 2 * i..42 + 2 * i].copy_from_slice(&d.to_be_bytes());
        }
        h[70..72].copy_from_slice(&4i16.to_be_bytes());
        h[108..112].copy_from_slice(&352f32.to_be_bytes());
        h[112..116].copy_from_slice(&0.5f32.to_be_bytes());
        h[116..120].copy_from_slice(&1f32.to_be_bytes());
        for x in [2i16, 4, -2, 100] {
            h.extend_from_slice(&x.to_be_bytes());
        }
        let w = VolumeData::from_nifti(&h).unwrap();
        assert_eq!(w.values, vec![2.0, 3.0, 0.0, 51.0]);
        assert_eq!(w.dims, [2, 2, 1]);
        assert!(VolumeData::from_nifti(b"nope").is_err());
        assert!(VolumeData::from_nifti(&bytes[..400]).is_err());
    }

    #[cfg(feature = "gzip")]
    #[test]
    fn gzipped_nifti() {
        use std::io::Write;
        let v = ball(6);
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(&v.to_nifti()).unwrap();
        assert_eq!(VolumeData::from_nifti(&e.finish().unwrap()).unwrap(), v);
    }

    #[test]
    fn vtk_ascii_and_binary() {
        let ascii = "# vtk DataFile Version 3.0\nball\nASCII\nDATASET STRUCTURED_POINTS\nDIMENSIONS 2 2 2\nSPACING 1 1 3\nORIGIN 0 0 0\nPOINT_DATA 8\nSCALARS density float 1\nLOOKUP_TABLE default\n0 1 2 3\n4 5 6 7\n";
        let v = VolumeData::from_vtk(ascii.as_bytes()).unwrap();
        assert_eq!(v.dims, [2, 2, 2]);
        assert_eq!(v.values[7], 7.0);
        assert_eq!(v.extent()[2], (0.0, 3.0));
        let mut bin = b"# vtk DataFile Version 3.0\nb\nBINARY\nDATASET STRUCTURED_POINTS\nDIMENSIONS 2 1 1\nPOINT_DATA 2\nSCALARS s short\nLOOKUP_TABLE default\n".to_vec();
        bin.extend_from_slice(&300i16.to_be_bytes());
        bin.extend_from_slice(&(-5i16).to_be_bytes());
        assert_eq!(
            VolumeData::from_vtk(&bin).unwrap().values,
            vec![300.0, -5.0]
        );
        let poly = "# vtk DataFile Version 3.0\nx\nASCII\nDATASET POLYDATA\n";
        assert!(VolumeData::from_vtk(poly.as_bytes())
            .unwrap_err()
            .contains("POLYDATA"));
    }

    #[test]
    fn metaimage_local_data() {
        let mut m = b"ObjectType = Image\nNDims = 3\nDimSize = 2 1 1\nElementSpacing = 0.7 0.7 1.5\nElementType = MET_USHORT\nElementDataFile = LOCAL\n".to_vec();
        m.extend_from_slice(&1000u16.to_le_bytes());
        m.extend_from_slice(&7u16.to_le_bytes());
        let v = VolumeData::from_metaimage(&m, None).unwrap();
        assert_eq!(v.values, vec![1000.0, 7.0]);
        assert_eq!(v.spacing, [0.7, 0.7, 1.5]);
    }

    #[test]
    fn draws_in_physical_units() {
        let v = ball(16);
        let mut fig = crate::plot::Figure::new(300, 300);
        fig.axes().volume_data(&v);
        let px = fig.to_pixels().unwrap();
        assert!(px.rgba.chunks(4).any(|p| p[0] > 150));
    }
}
