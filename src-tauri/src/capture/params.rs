use crate::capture::HotkeyKind;

pub const W_START_OFFSET: i32 = 25;
pub const W_LENGTH: i32 = 750;
pub const W_LINE_WIDTH: i32 = 70;
pub const W_LOOKAHEAD: i32 = 14;
pub const W_LOOKBEHIND: i32 = 1;
pub const W_SEARCH_RADIUS: i32 = 30;

pub const E_LENGTH: i32 = 1500;
pub const E_LINE_WIDTH: i32 = 70;
pub const E_LOOKAHEAD: i32 = 14;
pub const E_LOOKBEHIND: i32 = 14;
pub const E_SEARCH_RADIUS: i32 = 30;

pub const CROP_TOP_OFFSET: i32 = 35;

#[derive(Debug, Clone, Copy)]
pub struct ModeProfile {
    pub crop_left_offset: i32,
    pub crop_top_offset: i32,
    pub crop_w: i32,
    pub crop_h: i32,
    pub pt_in_crop_x: i32,
    pub pt_in_crop_y: i32,
    pub lookahead: i32,
    pub lookbehind: i32,
    pub search_radius: i32,
}

pub fn profile_for(kind: HotkeyKind) -> Option<ModeProfile> {
    match kind {
        HotkeyKind::Q => None,
        HotkeyKind::W => Some(ModeProfile {
            crop_left_offset: W_START_OFFSET,
            crop_top_offset: CROP_TOP_OFFSET,
            crop_w: W_START_OFFSET + W_LENGTH,
            crop_h: W_LINE_WIDTH,
            pt_in_crop_x: W_START_OFFSET,
            pt_in_crop_y: CROP_TOP_OFFSET,
            lookahead: W_LOOKAHEAD,
            lookbehind: W_LOOKBEHIND,
            search_radius: W_SEARCH_RADIUS,
        }),
        HotkeyKind::E => Some(ModeProfile {
            crop_left_offset: E_LENGTH / 2,
            crop_top_offset: CROP_TOP_OFFSET,
            crop_w: E_LENGTH,
            crop_h: E_LINE_WIDTH,
            pt_in_crop_x: E_LENGTH / 2,
            pt_in_crop_y: CROP_TOP_OFFSET,
            lookahead: E_LOOKAHEAD,
            lookbehind: E_LOOKBEHIND,
            search_radius: E_SEARCH_RADIUS,
        }),
    }
}
