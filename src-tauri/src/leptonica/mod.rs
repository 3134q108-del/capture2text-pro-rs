use std::ffi::c_void;
use std::path::Path;
use std::ptr;

use leptonica_sys::{
    boxaDestroy, pixConnComp, pixConvertTo8, pixDestroy, pixGetDepth, pixGetHeight, pixGetWidth,
    pixReadMem, pixRemoveBorderConnComps, pixScale, pixWriteMem, pixaDestroy, pixaGetCount,
    Boxa, Pix as LepPix, Pixa, pixConvertTo1, lept_free,
};
use thiserror::Error;

const IFF_PNG_FORMAT: i32 = 3;

#[derive(Debug, Error)]
pub enum LeptonicaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{op} returned null")]
    NullResult { op: &'static str },
    #[error("pixWrite failed with code {code}")]
    PixWriteFailed { code: i32 },
}

#[derive(Debug)]
pub struct Pix {
    raw: *mut LepPix,
}

impl Pix {
    pub fn read(path: &Path) -> Result<Self, LeptonicaError> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, LeptonicaError> {
        let raw = unsafe { pixReadMem(data.as_ptr(), data.len()) };
        Self::from_raw(raw, "pixReadMem")
    }

    pub fn to_bytes(&self, format: i32) -> Result<Vec<u8>, LeptonicaError> {
        let mut data_ptr: *mut u8 = ptr::null_mut();
        let mut size: usize = 0;
        let rc = unsafe { pixWriteMem(&mut data_ptr, &mut size, self.raw, format) };

        if rc != 0 || data_ptr.is_null() {
            if !data_ptr.is_null() {
                unsafe { lept_free(data_ptr as *mut c_void) };
            }
            return Err(LeptonicaError::PixWriteFailed { code: rc });
        }

        let bytes = unsafe { std::slice::from_raw_parts(data_ptr as *const u8, size).to_vec() };
        unsafe { lept_free(data_ptr as *mut c_void) };
        Ok(bytes)
    }

    pub fn convert_to_8(&self) -> Result<Self, LeptonicaError> {
        let raw = unsafe { pixConvertTo8(self.raw, 0) };
        Self::from_raw(raw, "pixConvertTo8")
    }

    pub fn convert_to_1(&self, threshold: i32) -> Result<Self, LeptonicaError> {
        let raw = unsafe { pixConvertTo1(self.raw, threshold) };
        Self::from_raw(raw, "pixConvertTo1")
    }

    pub fn scale(&self, scale_x: f32, scale_y: f32) -> Result<Self, LeptonicaError> {
        let raw = unsafe { pixScale(self.raw, scale_x, scale_y) };
        Self::from_raw(raw, "pixScale")
    }

    pub fn remove_border_conn_comps(&self, connectivity: i32) -> Result<Self, LeptonicaError> {
        let raw = unsafe { pixRemoveBorderConnComps(self.raw, connectivity) };
        Self::from_raw(raw, "pixRemoveBorderConnComps")
    }

    pub fn conn_comp_count(&self, connectivity: i32) -> Result<i32, LeptonicaError> {
        let mut boxa: *mut Boxa;
        let mut pixa: *mut Pixa = ptr::null_mut();

        boxa = unsafe { pixConnComp(self.raw, &mut pixa, connectivity) };
        if boxa.is_null() {
            cleanup_conn_comp_outputs(&mut boxa, &mut pixa);
            return Err(LeptonicaError::NullResult { op: "pixConnComp" });
        }

        let count = if pixa.is_null() {
            0
        } else {
            unsafe { pixaGetCount(pixa) }
        };
        cleanup_conn_comp_outputs(&mut boxa, &mut pixa);
        Ok(count)
    }

    pub fn write_png(&self, path: &Path) -> Result<(), LeptonicaError> {
        let bytes = self.to_bytes(IFF_PNG_FORMAT)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn width(&self) -> i32 {
        unsafe { pixGetWidth(self.raw) }
    }

    pub fn height(&self) -> i32 {
        unsafe { pixGetHeight(self.raw) }
    }

    pub fn depth(&self) -> i32 {
        unsafe { pixGetDepth(self.raw) }
    }

    fn from_raw(raw: *mut LepPix, op: &'static str) -> Result<Self, LeptonicaError> {
        if raw.is_null() {
            Err(LeptonicaError::NullResult { op })
        } else {
            Ok(Self { raw })
        }
    }
}

impl Drop for Pix {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { pixDestroy(&mut self.raw) };
        }
    }
}

fn cleanup_conn_comp_outputs(boxa: &mut *mut Boxa, pixa: &mut *mut Pixa) {
    if !(*boxa).is_null() {
        unsafe { boxaDestroy(boxa) };
    }

    if !(*pixa).is_null() {
        unsafe { pixaDestroy(pixa) };
    }
}
