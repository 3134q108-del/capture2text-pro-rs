use crate::leptonica::bounding_rect::{self, BoundingBox};
use crate::leptonica::{Box, LeptonicaError, Pix};

pub const OCR_SCALE_FACTOR_DEFAULT: f32 = 3.5;
pub const OCR_SCALE_FACTOR_MIN: f32 = 0.71;
pub const OCR_SCALE_FACTOR_MAX: f32 = 5.0;
pub const DARK_BG_THRESHOLD: f32 = 0.5;
pub const NEG_RECT_PROBE_SIZE: i32 = 40;
pub const USM_HALFWIDTH: i32 = 5;
pub const USM_FRACT: f32 = 2.5;
pub const OTSU_SX: i32 = 2000;
pub const OTSU_SY: i32 = 2000;
pub const OTSU_SMOOTH_X: i32 = 0;
pub const OTSU_SMOOTH_Y: i32 = 0;
pub const OTSU_SCOREFRACT: f32 = 0.0;
pub const REMOVE_NOISE_MIN_BLOB: i32 = 3;
pub const PIPELINE_CONNECTIVITY: i32 = 8;
pub const OCR_DPI: i32 = 300;

#[derive(Debug, Clone, Copy)]
pub struct ExtractParams {
    pub pt_x: i32,
    pub pt_y: i32,
    pub lookahead: i32,
    pub lookbehind: i32,
    pub search_radius: i32,
    pub vertical: bool,
    pub scale_factor: f32,
}

#[derive(Debug)]
pub struct ExtractResult {
    pub cropped_bin: Pix,
    pub bbox_unscaled: BoundingBox,
    pub bbox_scaled: BoundingBox,
}

pub fn extract_text_block(
    src: &Pix,
    params: ExtractParams,
) -> Result<Option<ExtractResult>, LeptonicaError> {
    let scale = params
        .scale_factor
        .clamp(OCR_SCALE_FACTOR_MIN, OCR_SCALE_FACTOR_MAX);

    let mut gray = make_gray(src)?;
    let binarize_for_neg = gray.otsu_adaptive_threshold(
        OTSU_SX,
        OTSU_SY,
        OTSU_SMOOTH_X,
        OTSU_SMOOTH_Y,
        OTSU_SCOREFRACT,
    )?;

    let neg_rect = make_neg_probe_rect(
        params.pt_x,
        params.pt_y,
        binarize_for_neg.width(),
        binarize_for_neg.height(),
    );
    let pixel_avg = binarize_for_neg.average_in_rect(neg_rect).unwrap_or(0.0);
    drop(binarize_for_neg);

    if pixel_avg > DARK_BG_THRESHOLD {
        gray.invert()?;
    }

    let mut bin = scale_unsharp_binarize(&gray, scale)?;

    if params.vertical {
        erase_connecting_border_pixels_below(&mut bin, (params.pt_y as f32 * scale) as i32)?;
    } else {
        erase_connecting_border_pixels_right(&mut bin, (params.pt_x as f32 * scale) as i32)?;
    }

    let no_border = bin.remove_border_conn_comps(PIPELINE_CONNECTIVITY)?;
    let denoise = no_border.select_by_size_gt_either(
        REMOVE_NOISE_MIN_BLOB,
        REMOVE_NOISE_MIN_BLOB,
        PIPELINE_CONNECTIVITY,
    )?;

    let bbox_scaled = bounding_rect::get_bounding_rect(
        &denoise,
        (params.pt_x as f32 * scale) as i32,
        (params.pt_y as f32 * scale) as i32,
        params.vertical,
        (params.lookahead as f32 * scale) as i32,
        (params.lookbehind as f32 * scale) as i32,
        (params.search_radius as f32 * scale) as i32,
    );
    drop(denoise);

    if bbox_scaled.w < 3 && bbox_scaled.h < 3 {
        return Ok(None);
    }

    let mut cropped_bin = bin.clip_rectangle(Box {
        x: bbox_scaled.x,
        y: bbox_scaled.y,
        w: bbox_scaled.w,
        h: bbox_scaled.h,
    })?;
    drop(bin);

    cropped_bin.set_resolution(OCR_DPI, OCR_DPI)?;

    let x0 = (bbox_scaled.x as f32 / scale).floor() as i32;
    let y0 = (bbox_scaled.y as f32 / scale).floor() as i32;
    let x1 = ((bbox_scaled.x + bbox_scaled.w) as f32 / scale).ceil() as i32;
    let y1 = ((bbox_scaled.y + bbox_scaled.h) as f32 / scale).ceil() as i32;

    let bbox_unscaled = BoundingBox {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    };

    Ok(Some(ExtractResult {
        cropped_bin,
        bbox_unscaled,
        bbox_scaled,
    }))
}

fn make_gray(src: &Pix) -> Result<Pix, LeptonicaError> {
    match src.depth() {
        32 => src.convert_rgb_to_gray_32bpp(),
        24 => {
            let p32 = src.convert_24_to_32()?;
            p32.convert_rgb_to_gray_32bpp()
        }
        _ => src.convert_to_8(),
    }
}

fn scale_unsharp_binarize(gray: &Pix, scale_factor: f32) -> Result<Pix, LeptonicaError> {
    let scaled = gray.scale_gray_li(scale_factor, scale_factor)?;
    let sharpened = scaled.unsharp_mask_gray(USM_HALFWIDTH, USM_FRACT)?;
    sharpened.otsu_adaptive_threshold(
        OTSU_SX,
        OTSU_SY,
        OTSU_SMOOTH_X,
        OTSU_SMOOTH_Y,
        OTSU_SCOREFRACT,
    )
}

fn make_neg_probe_rect(pt_x: i32, pt_y: i32, img_w: i32, img_h: i32) -> Box {
    let x = (pt_x - NEG_RECT_PROBE_SIZE / 2).max(0);
    let y = (pt_y - NEG_RECT_PROBE_SIZE / 2).max(0);
    let w = (img_w - x).min(NEG_RECT_PROBE_SIZE);
    let h = (img_h - y).min(NEG_RECT_PROBE_SIZE);
    Box { x, y, w, h }
}

fn erase_connecting_border_pixels_right(
    bin: &mut Pix,
    right_of_x: i32,
) -> Result<(), LeptonicaError> {
    let border_conn = bin.extract_border_conn_comps(PIPELINE_CONNECTIVITY)?;
    let h = border_conn.height();
    let w = border_conn.width();
    let start_x = right_of_x.max(0);

    for y in 0..h {
        for x in start_x..w {
            if bounding_rect::is_black(&border_conn, x, y) {
                bin.clear_in_rect(Box {
                    x,
                    y,
                    w: w - x,
                    h: 1,
                })?;
                break;
            }
        }
    }

    Ok(())
}

fn erase_connecting_border_pixels_below(
    bin: &mut Pix,
    below_y: i32,
) -> Result<(), LeptonicaError> {
    let border_conn = bin.extract_border_conn_comps(PIPELINE_CONNECTIVITY)?;
    let h = border_conn.height();
    let w = border_conn.width();
    let start_y = below_y.max(0);

    for x in 0..w {
        for y in start_y..h {
            if bounding_rect::is_black(&border_conn, x, y) {
                bin.clear_in_rect(Box {
                    x,
                    y,
                    w: 1,
                    h: h - y,
                })?;
                break;
            }
        }
    }

    Ok(())
}
