//! Columns through the Arrow PyCapsule interface (`__arrow_c_stream__`,
//! `__arrow_c_array__`): Polars and pyarrow arrays, pandas Arrow-backed
//! series, DuckDB results, … are read from their Arrow buffers in one pass,
//! without NumPy or a float64 copy in between. Numeric types only (missing
//! values become NaN); anything else returns `None` for the NumPy path.

use pyo3::prelude::*;
use pyo3::types::{PyCapsule, PyCapsuleMethods, PyTuple};
use std::ffi::{c_char, c_int, c_void, CStr};

#[repr(C)]
struct ArrowSchema {
    format: *const c_char,
    name: *const c_char,
    metadata: *const c_char,
    flags: i64,
    n_children: i64,
    children: *mut *mut ArrowSchema,
    dictionary: *mut ArrowSchema,
    release: Option<unsafe extern "C" fn(*mut ArrowSchema)>,
    private_data: *mut c_void,
}

#[repr(C)]
struct ArrowArray {
    length: i64,
    null_count: i64,
    offset: i64,
    n_buffers: i64,
    n_children: i64,
    buffers: *mut *const c_void,
    children: *mut *mut ArrowArray,
    dictionary: *mut ArrowArray,
    release: Option<unsafe extern "C" fn(*mut ArrowArray)>,
    private_data: *mut c_void,
}

#[repr(C)]
struct ArrowArrayStream {
    get_schema: Option<unsafe extern "C" fn(*mut ArrowArrayStream, *mut ArrowSchema) -> c_int>,
    get_next: Option<unsafe extern "C" fn(*mut ArrowArrayStream, *mut ArrowArray) -> c_int>,
    get_last_error: Option<unsafe extern "C" fn(*mut ArrowArrayStream) -> *const c_char>,
    release: Option<unsafe extern "C" fn(*mut ArrowArrayStream)>,
    private_data: *mut c_void,
}

/// Owned C structs, released when dropped.
struct Owned<T: Release>(T);

/// The `release` callback every Arrow C struct carries.
trait Release {
    fn release(&mut self);
    /// Marks the struct at `p` as released (moved out of).
    ///
    /// # Safety
    /// `p` must point to a valid struct.
    unsafe fn clear(p: *mut Self);
}

macro_rules! release {
    ($t:ty) => {
        impl Release for $t {
            fn release(&mut self) {
                if let Some(r) = self.release.take() {
                    // SAFETY: the producer's callback for this very struct.
                    unsafe { r(self) };
                }
            }
            unsafe fn clear(p: *mut Self) {
                // SAFETY: `p` is valid (caller).
                unsafe { (*p).release = None };
            }
        }
    };
}
release!(ArrowSchema);
release!(ArrowArray);
release!(ArrowArrayStream);

impl<T: Release> Drop for Owned<T> {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// Moves the struct out of a capsule (the capsule's destructor then sees
/// a released struct and does nothing).
///
/// # Safety
/// The capsule must hold a `T` of the Arrow C interface under `name`.
unsafe fn take<T: Release>(cap: &Bound<'_, PyAny>, name: &CStr) -> PyResult<Owned<T>> {
    let cap = cap.cast::<PyCapsule>()?;
    let p = cap.pointer_checked(Some(name))?.as_ptr() as *mut T;
    // SAFETY: the capsule holds a valid `T`; moving out is the protocol.
    let v = unsafe { std::ptr::read(p) };
    unsafe { T::clear(p) };
    Ok(Owned(v))
}

/// Numeric storage of an Arrow format string.
#[derive(Clone, Copy)]
enum Kind {
    F64,
    F32,
    I64,
    I32,
    I16,
    I8,
    U64,
    U32,
    U16,
    U8,
    Bool,
}

fn kind(format: &[u8]) -> Option<Kind> {
    Some(match format {
        b"g" => Kind::F64,
        b"f" => Kind::F32,
        b"l" => Kind::I64,
        b"i" => Kind::I32,
        b"s" => Kind::I16,
        b"c" => Kind::I8,
        b"L" => Kind::U64,
        b"I" => Kind::U32,
        b"S" => Kind::U16,
        b"C" => Kind::U8,
        b"b" => Kind::Bool,
        _ => return None,
    })
}

/// Appends one array's values (nulls as NaN).
///
/// # Safety
/// `a` must be a valid array of `kind`.
unsafe fn append(a: &ArrowArray, kind: Kind, out: &mut Vec<f64>) {
    let (n, off) = (a.length as usize, a.offset as usize);
    if n == 0 || a.n_buffers < 2 {
        return;
    }
    // SAFETY: primitive arrays have a validity and a data buffer.
    let bufs = unsafe { std::slice::from_raw_parts(a.buffers, 2) };
    let valid = bufs[0] as *const u8;
    let data = bufs[1];
    let start = out.len();
    macro_rules! copy {
        ($t:ty) => {{
            // SAFETY: `off + n` values of `$t` per the Arrow spec.
            let s = unsafe { std::slice::from_raw_parts((data as *const $t).add(off), n) };
            out.extend(s.iter().map(|&v| v as f64));
        }};
    }
    match kind {
        Kind::F64 => copy!(f64),
        Kind::F32 => copy!(f32),
        Kind::I64 => copy!(i64),
        Kind::I32 => copy!(i32),
        Kind::I16 => copy!(i16),
        Kind::I8 => copy!(i8),
        Kind::U64 => copy!(u64),
        Kind::U32 => copy!(u32),
        Kind::U16 => copy!(u16),
        Kind::U8 => copy!(u8),
        Kind::Bool => {
            let bits = data as *const u8;
            out.extend((off..off + n).map(|i| {
                // SAFETY: bit-packed booleans, `off + n` bits.
                (unsafe { *bits.add(i / 8) } >> (i % 8) & 1) as f64
            }));
        }
    }
    if a.null_count != 0 && !valid.is_null() {
        for (k, v) in out[start..].iter_mut().enumerate() {
            let i = off + k;
            // SAFETY: the validity bitmap covers `off + n` bits.
            if unsafe { *valid.add(i / 8) } >> (i % 8) & 1 == 0 {
                *v = f64::NAN;
            }
        }
    }
}

fn format_of(s: &ArrowSchema) -> Option<Kind> {
    if s.format.is_null() || !s.dictionary.is_null() {
        return None;
    }
    // SAFETY: a valid schema has a NUL-terminated format string.
    kind(unsafe { CStr::from_ptr(s.format) }.to_bytes())
}

/// The values of an object exporting Arrow data, or `None` when it does
/// not (or its type is not numeric).
pub(crate) fn values(obj: &Bound<'_, PyAny>) -> PyResult<Option<Vec<f64>>> {
    if obj.hasattr("__arrow_c_stream__")? {
        let cap = obj.call_method0("__arrow_c_stream__")?;
        // SAFETY: the method returns an "arrow_array_stream" capsule.
        let mut stream = unsafe { take::<ArrowArrayStream>(&cap, c"arrow_array_stream")? };
        let st = &mut stream.0 as *mut ArrowArrayStream;
        let (Some(get_schema), Some(get_next)) = (stream.0.get_schema, stream.0.get_next) else {
            return Ok(None);
        };
        let mut schema = Owned(empty_schema());
        // SAFETY: valid stream; the schema is released by `Owned`.
        if unsafe { get_schema(st, &mut schema.0) } != 0 {
            return Ok(None);
        }
        let Some(kind) = format_of(&schema.0) else {
            return Ok(None);
        };
        let mut out = Vec::new();
        loop {
            let mut a = Owned(empty_array());
            // SAFETY: valid stream; each array is released by `Owned`.
            if unsafe { get_next(st, &mut a.0) } != 0 {
                return Ok(None);
            }
            if a.0.release.is_none() {
                break;
            }
            if out.is_empty() {
                out.reserve(a.0.length.max(0) as usize);
            }
            // SAFETY: an array of the schema's type.
            unsafe { append(&a.0, kind, &mut out) };
        }
        return Ok(Some(out));
    }
    if obj.hasattr("__arrow_c_array__")? {
        let pair = obj.call_method0("__arrow_c_array__")?;
        let pair = pair.cast::<PyTuple>()?;
        // SAFETY: the method returns (schema, array) capsules.
        let schema = unsafe { take::<ArrowSchema>(&pair.get_item(0)?, c"arrow_schema")? };
        let array = unsafe { take::<ArrowArray>(&pair.get_item(1)?, c"arrow_array")? };
        let Some(kind) = format_of(&schema.0) else {
            return Ok(None);
        };
        let mut out = Vec::with_capacity(array.0.length.max(0) as usize);
        // SAFETY: an array of the schema's type.
        unsafe { append(&array.0, kind, &mut out) };
        return Ok(Some(out));
    }
    Ok(None)
}

fn empty_schema() -> ArrowSchema {
    ArrowSchema {
        format: std::ptr::null(),
        name: std::ptr::null(),
        metadata: std::ptr::null(),
        flags: 0,
        n_children: 0,
        children: std::ptr::null_mut(),
        dictionary: std::ptr::null_mut(),
        release: None,
        private_data: std::ptr::null_mut(),
    }
}

fn empty_array() -> ArrowArray {
    ArrowArray {
        length: 0,
        null_count: 0,
        offset: 0,
        n_buffers: 0,
        n_children: 0,
        buffers: std::ptr::null_mut(),
        children: std::ptr::null_mut(),
        dictionary: std::ptr::null_mut(),
        release: None,
        private_data: std::ptr::null_mut(),
    }
}
