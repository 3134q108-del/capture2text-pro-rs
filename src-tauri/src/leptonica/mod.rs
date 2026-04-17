pub mod bounding_rect;

use std::ffi::c_void;
use std::path::Path;
use std::ptr;

use leptonica_sys::{
    boxCreate, boxDestroy, boxaDestroy, lept_free, pixAverageInRect, pixClearInRect,
    pixClipRectangle, pixConnComp, pixConvert24To32, pixConvertRGBToGray, pixConvertTo1,
    pixConvertTo8, pixDestroy, pixExtractBorderConnComps, pixGetDepth, pixGetHeight, pixGetPixel,
    pixGetWidth, pixInvert, pixOtsuAdaptiveThreshold, pixReadMem, pixRemoveBorderConnComps,
    pixScale, pixScaleGrayLI, pixSelectBySize, pixSetResolution, pixUnsharpMaskingGray,
    pixWriteMem, pixaDestroy, pixaGetCount, Box as LepBox, Boxa, L_SELECT_IF_EITHER,
    L_SELECT_IF_GT, Pix as LepPix, Pixa,
};
use thiserror::Error;

const IFF_PNG_FORMAT: i32 = 3;

#[derive(Debug, Error)]
pub enum LeptonicaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{op} returned null")]
    NullResult { op: &'static str },
    #[error("{op} requires depth {expected}bpp, got {actual}bpp")]
    DepthMismatch {
        op: &'static str,
        expected: i32,
        actual: i32,
    },
    #[error("{op} failed with code {code}")]
    ApiFailed { op: &'static str, code: i32 },
    #[error("pixWrite failed with code {code}")]
    PixWriteFailed { code: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Box {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

struct OwnedBox(*mut LepBox);

impl OwnedBox {
    fn new(rect: Box) -> Result<Self, LeptonicaError> {
        let ptr = unsafe { boxCreate(rect.x, rect.y, rect.w, rect.h) };
        if ptr.is_null() {
            Err(LeptonicaError::NullResult { op: "boxCreate" })
        } else {
            Ok(Self(ptr))
        }
    }

    fn as_ptr(&self) -> *mut LepBox {
        self.0
    }
}

impl Drop for OwnedBox {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { boxDestroy(&mut self.0) };
        }
    }
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

    pub fn convert_rgb_to_gray_32bpp(&self) -> Result<Self, LeptonicaError> {
        self.ensure_depth("pixConvertRGBToGray", 32)?;
        let raw = unsafe { pixConvertRGBToGray(self.raw, 0.0, 0.0, 0.0) };
        Self::from_raw(raw, "pixConvertRGBToGray")
    }

    pub fn convert_24_to_32(&self) -> Result<Self, LeptonicaError> {
        self.ensure_depth("pixConvert24To32", 24)?;
        let raw = unsafe { pixConvert24To32(self.raw) };
        Self::from_raw(raw, "pixConvert24To32")
    }

    pub fn convert_to_1(&self, threshold: i32) -> Result<Self, LeptonicaError> {
        let raw = unsafe { pixConvertTo1(self.raw, threshold) };
        Self::from_raw(raw, "pixConvertTo1")
    }

    pub fn scale(&self, scale_x: f32, scale_y: f32) -> Result<Self, LeptonicaError> {
        let raw = unsafe { pixScale(self.raw, scale_x, scale_y) };
        Self::from_raw(raw, "pixScale")
    }

    pub fn scale_gray_li(&self, scale_x: f32, scale_y: f32) -> Result<Self, LeptonicaError> {
        self.ensure_depth("pixScaleGrayLI", 8)?;
        let raw = unsafe { pixScaleGrayLI(self.raw, scale_x, scale_y) };
        Self::from_raw(raw, "pixScaleGrayLI")
    }

    pub fn unsharp_mask_gray(&self, halfwidth: i32, fract: f32) -> Result<Self, LeptonicaError> {
        self.ensure_depth("pixUnsharpMaskingGray", 8)?;
        let raw = unsafe { pixUnsharpMaskingGray(self.raw, halfwidth, fract) };
        Self::from_raw(raw, "pixUnsharpMaskingGray")
    }

    pub fn otsu_adaptive_threshold(
        &self,
        sx: i32,
        sy: i32,
        smooth_x: i32,
        smooth_y: i32,
        scorefract: f32,
    ) -> Result<Self, LeptonicaError> {
        self.ensure_depth("pixOtsuAdaptiveThreshold", 8)?;
        let mut thresh_ptr: *mut LepPix = ptr::null_mut();
        let mut bin_ptr: *mut LepPix = ptr::null_mut();

        let status = unsafe {
            pixOtsuAdaptiveThreshold(
                self.raw,
                sx,
                sy,
                smooth_x,
                smooth_y,
                scorefract,
                &mut thresh_ptr,
                &mut bin_ptr,
            )
        };

        if status != 0 {
            if !thresh_ptr.is_null() {
                unsafe { pixDestroy(&mut thresh_ptr) };
            }
            if !bin_ptr.is_null() {
                unsafe { pixDestroy(&mut bin_ptr) };
            }
            return Err(LeptonicaError::ApiFailed {
                op: "pixOtsuAdaptiveThreshold",
                code: status,
            });
        }

        if !thresh_ptr.is_null() {
            unsafe { pixDestroy(&mut thresh_ptr) };
        }

        if bin_ptr.is_null() {
            return Err(LeptonicaError::NullResult {
                op: "pixOtsuAdaptiveThreshold",
            });
        }

        Ok(Self { raw: bin_ptr })
    }

    pub fn extract_border_conn_comps(&self, connectivity: i32) -> Result<Self, LeptonicaError> {
        Self::ensure_connectivity("pixExtractBorderConnComps", connectivity)?;
        let raw = unsafe { pixExtractBorderConnComps(self.raw, connectivity) };
        Self::from_raw(raw, "pixExtractBorderConnComps")
    }

    pub fn clear_in_rect(&mut self, rect: Box) -> Result<(), LeptonicaError> {
        let owned_box = OwnedBox::new(rect)?;
        let status = unsafe { pixClearInRect(self.raw, owned_box.as_ptr()) };
        if status != 0 {
            return Err(LeptonicaError::ApiFailed {
                op: "pixClearInRect",
                code: status,
            });
        }

        Ok(())
    }

    pub fn average_in_rect(&self, rect: Box) -> Result<f32, LeptonicaError> {
        let owned_box = OwnedBox::new(rect)?;
        let mut average: f32 = 0.0;
        let status = unsafe {
            pixAverageInRect(
                self.raw,
                ptr::null_mut(),
                owned_box.as_ptr(),
                0,
                255,
                1,
                &mut average,
            )
        };
        if status != 0 {
            return Err(LeptonicaError::ApiFailed {
                op: "pixAverageInRect",
                code: status,
            });
        }

        Ok(average)
    }

    pub fn invert(&mut self) -> Result<(), LeptonicaError> {
        let raw = unsafe { pixInvert(self.raw, self.raw) };
        if raw.is_null() {
            return Err(LeptonicaError::NullResult { op: "pixInvert" });
        }

        self.raw = raw;
        Ok(())
    }

    pub fn select_by_size_gt_either(
        &self,
        min_w: i32,
        min_h: i32,
        connectivity: i32,
    ) -> Result<Self, LeptonicaError> {
        Self::ensure_connectivity("pixSelectBySize", connectivity)?;
        let raw = unsafe {
            pixSelectBySize(
                self.raw,
                min_w,
                min_h,
                connectivity,
                L_SELECT_IF_EITHER,
                L_SELECT_IF_GT,
                ptr::null_mut(),
            )
        };
        Self::from_raw(raw, "pixSelectBySize")
    }

    pub fn clip_rectangle(&self, rect: Box) -> Result<Self, LeptonicaError> {
        let owned_box = OwnedBox::new(rect)?;
        let raw = unsafe { pixClipRectangle(self.raw, owned_box.as_ptr(), ptr::null_mut()) };
        Self::from_raw(raw, "pixClipRectangle")
    }

    pub fn set_resolution(&mut self, xres: i32, yres: i32) -> Result<(), LeptonicaError> {
        let status = unsafe { pixSetResolution(self.raw, xres, yres) };
        if status != 0 {
            return Err(LeptonicaError::ApiFailed {
                op: "pixSetResolution",
                code: status,
            });
        }

        Ok(())
    }

    pub fn remove_border_conn_comps(&self, connectivity: i32) -> Result<Self, LeptonicaError> {
        Self::ensure_connectivity("pixRemoveBorderConnComps", connectivity)?;
        let raw = unsafe { pixRemoveBorderConnComps(self.raw, connectivity) };
        Self::from_raw(raw, "pixRemoveBorderConnComps")
    }

    pub fn conn_comp_count(&self, connectivity: i32) -> Result<i32, LeptonicaError> {
        Self::ensure_connectivity("pixConnComp", connectivity)?;
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

    pub fn get_pixel(&self, x: i32, y: i32) -> Result<u32, LeptonicaError> {
        let mut pixel_value: u32 = 0;
        let status = unsafe { pixGetPixel(self.raw, x, y, &mut pixel_value) };
        if status != 0 {
            return Err(LeptonicaError::ApiFailed {
                op: "pixGetPixel",
                code: status,
            });
        }

        Ok(pixel_value)
    }

    fn from_raw(raw: *mut LepPix, op: &'static str) -> Result<Self, LeptonicaError> {
        if raw.is_null() {
            Err(LeptonicaError::NullResult { op })
        } else {
            Ok(Self { raw })
        }
    }

    fn ensure_depth(&self, op: &'static str, expected: i32) -> Result<(), LeptonicaError> {
        let actual = self.depth();
        if actual != expected {
            return Err(LeptonicaError::DepthMismatch {
                op,
                expected,
                actual,
            });
        }

        Ok(())
    }

    fn ensure_connectivity(op: &'static str, connectivity: i32) -> Result<(), LeptonicaError> {
        if connectivity == 4 || connectivity == 8 {
            Ok(())
        } else {
            Err(LeptonicaError::ApiFailed { op, code: -1 })
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
