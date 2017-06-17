use byteorder::WriteBytesExt;
use memmap::{Mmap, MmapViewSync, Protection};
use std::ffi::CString;
use std::io;
use std::io::Cursor;
use std::mem::transmute;

const ITEM_BIT_LEN: usize = 10;

pub const I32_METRIC_TYPE_CODE: u32 = 0;
pub const U32_METRIC_TYPE_CODE: u32 = 1;
pub const I64_METRIC_TYPE_CODE: u32 = 2;
pub const U64_METRIC_TYPE_CODE: u32 = 3;
pub const F32_METRIC_TYPE_CODE: u32 = 4;
pub const F64_METRIC_TYPE_CODE: u32 = 5;
pub const STRING_METRIC_TYPE_CODE: u32 = 6;

pub trait MetricType {
    fn type_code(&self) -> u32;
    fn write_to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()>;
}

macro_rules! impl_metric_type_for (
    ($typ:tt, $base_typ:tt, $type_code:expr) => (
        impl MetricType for $typ {
            
            fn type_code(&self) -> u32 {
                $type_code
            }

            fn write_to_writer<W: WriteBytesExt>(&self, mut w: &mut W) -> io::Result<()> {
                w.write_u64::<super::Endian>(
                    unsafe {
                        transmute::<$typ, $base_typ>(*self) as u64
                    }
                )
            }

        }
    )
);

impl_metric_type_for!(i32, u32, I32_METRIC_TYPE_CODE);
impl_metric_type_for!(u32, u32, U32_METRIC_TYPE_CODE);
impl_metric_type_for!(i64, u64, I64_METRIC_TYPE_CODE);
impl_metric_type_for!(u64, u64, U64_METRIC_TYPE_CODE);
impl_metric_type_for!(f32, u32, F32_METRIC_TYPE_CODE);
impl_metric_type_for!(f64, u64, F64_METRIC_TYPE_CODE);

impl MetricType for String {
    fn type_code(&self) -> u32 {
        STRING_METRIC_TYPE_CODE
    }

    fn write_to_writer<W: WriteBytesExt>(&self, mut writer: &mut W) -> io::Result<()> {
        writer.write_all(CString::new(self.as_str())?.to_bytes_with_nul())
    }
}

lazy_static! {
    static ref SCRATCH_VIEW: MmapViewSync = {
        Mmap::anonymous(super::STRING_BLOCK_LEN as usize, Protection::ReadWrite).unwrap()
            .into_view_sync()
    };
}

#[derive(Copy, Clone)]
pub enum Semamtics {
    Counter  = 1,
    Instant  = 3,
    Discrete = 4
}

pub struct Metric<T> {
    name: String,
    item: u32,
    sem: Semamtics,
    indom: u32,
    dim: u32,
    shorthelp: String,
    longhelp: String,
    val: T,
    mmap_view: MmapViewSync
}

impl<T: MetricType + Clone> Metric<T> {
    pub fn new(
        name: &str, item: u32, sem: Semamtics,
        dim: u32, init_val: T,
        shorthelp: &str, longhelp: &str) -> Self {
        
        assert!(name.len() < super::METRIC_NAME_MAX_LEN as usize);
        assert!(shorthelp.len() < super::STRING_BLOCK_LEN as usize);
        assert!(longhelp.len() < super::STRING_BLOCK_LEN as usize);

        Metric {
            name: name.to_owned(),
            item: item & ((1 << ITEM_BIT_LEN) - 1),
            sem: sem,
            indom: 0,
            dim: dim,
            shorthelp: shorthelp.to_owned(),
            longhelp: longhelp.to_owned(),
            val: init_val,
            mmap_view: unsafe { SCRATCH_VIEW.clone() }
        }
    }

    pub fn val(&self) -> T {
        self.val.clone()
    }

    pub fn set_val(&mut self, new_val: T) -> io::Result<()> {
        self.val = new_val;
        self.val.write_to_writer(unsafe { &mut self.mmap_view.as_mut_slice() })
    }
}

pub trait MMVMetric {
    fn name(&self) -> &str;
    fn item(&self) -> u32;
    fn type_code(&self) -> u32;
    fn sem(&self) -> &Semamtics;
    fn dim(&self) -> u32;
    fn indom(&self) -> u32;
    fn shorthelp(&self) -> &str;
    fn longhelp(&self) -> &str;
    fn write_value(&mut self, cursor: &mut Cursor<&mut [u8]>) -> io::Result<()>;
    fn set_mmap_view(&mut self, mmap_view: MmapViewSync);
}

impl<T: MetricType> MMVMetric for Metric<T> {
    fn name(&self) -> &str { &self.name }

    fn item(&self) -> u32 { self.item }

    fn type_code(&self) -> u32 { self.val.type_code() }

    fn sem(&self) -> &Semamtics { &self.sem }

    fn dim(&self) -> u32 { self.dim }

    fn indom(&self) -> u32 { self.indom }

    fn shorthelp(&self) -> &str { &self.shorthelp }

    fn longhelp(&self) -> &str { &self.longhelp }

    fn write_value(&mut self, cursor: &mut Cursor<&mut [u8]>) -> io::Result<()> {
        self.val.write_to_writer(cursor)
    }

    fn set_mmap_view(&mut self, mmap_view: MmapViewSync) {
        self.mmap_view = mmap_view;
    }
}
