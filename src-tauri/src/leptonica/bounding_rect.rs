use super::Pix;

pub const SCALE: f32 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundingBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy)]
enum D8 {
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
}

#[derive(Debug, Clone, Copy)]
struct DirDist {
    dir: D8,
    dist: i32,
}

#[derive(Debug, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

pub fn get_bounding_rect(
    pix: &Pix,
    start_x: i32,
    start_y: i32,
    vertical: bool,
    lookahead: i32,
    lookbehind: i32,
    max_search_dist: i32,
) -> BoundingBox {
    let nearest_pt = find_nearest_black_pixel(pix, start_x, start_y, max_search_dist);
    let mut rect = BoundingBox {
        x: nearest_pt.x,
        y: nearest_pt.y,
        w: 0,
        h: 0,
    };
    let mut rect_last = rect;

    let mut list_d4: Vec<DirDist> = Vec::new();

    if vertical {
        if lookbehind >= 1 {
            for i in 1..=lookbehind {
                list_d4.push(DirDist {
                    dir: D8::Top,
                    dist: i,
                });
            }
        }

        list_d4.push(DirDist {
            dir: D8::Right,
            dist: 1,
        });
        list_d4.push(DirDist {
            dir: D8::Left,
            dist: 1,
        });

        if lookahead >= 1 {
            for i in 1..=lookahead {
                list_d4.push(DirDist {
                    dir: D8::Bottom,
                    dist: i,
                });
            }
        }
    } else {
        list_d4.push(DirDist {
            dir: D8::Top,
            dist: 1,
        });

        if lookbehind >= 1 {
            for i in 1..=lookbehind {
                list_d4.push(DirDist {
                    dir: D8::Left,
                    dist: i,
                });
            }
        }

        list_d4.push(DirDist {
            dir: D8::Bottom,
            dist: 1,
        });

        if lookahead >= 1 {
            for i in 1..=lookahead {
                list_d4.push(DirDist {
                    dir: D8::Right,
                    dist: i,
                });
            }
        }
    }

    let list_corners = vec![
        DirDist {
            dir: D8::TopRight,
            dist: 1,
        },
        DirDist {
            dir: D8::BottomRight,
            dist: 1,
        },
        DirDist {
            dir: D8::BottomLeft,
            dist: 1,
        },
        DirDist {
            dir: D8::TopLeft,
            dist: 1,
        },
    ];

    for _ in 0..10 {
        expand_rect(pix, &list_d4, &mut rect, true);
        expand_rect(pix, &list_corners, &mut rect, false);

        if rect.x == rect_last.x
            && rect.y == rect_last.y
            && rect.w == rect_last.w
            && rect.h == rect_last.h
        {
            break;
        }

        rect_last = rect;
    }

    rect
}

fn find_nearest_black_pixel(pix: &Pix, start_x: i32, start_y: i32, max_dist: i32) -> Point {
    let mut pt = Point {
        x: start_x,
        y: start_y,
    };

    for dist in 1..max_dist {
        pt.x += 1;
        if is_black(pix, pt.x, pt.y) {
            return pt;
        }

        for _ in 0..(dist * 2 - 1) {
            pt.y += 1;
            if is_black(pix, pt.x, pt.y) {
                return pt;
            }
        }

        for _ in 0..(dist * 2) {
            pt.x -= 1;
            if is_black(pix, pt.x, pt.y) {
                return pt;
            }
        }

        for _ in 0..(dist * 2) {
            pt.y -= 1;
            if is_black(pix, pt.x, pt.y) {
                return pt;
            }
        }

        for _ in 0..(dist * 2) {
            pt.x += 1;
            if is_black(pix, pt.x, pt.y) {
                return pt;
            }
        }
    }

    Point { x: -1, y: -1 }
}

#[inline]
pub fn is_black(pix: &Pix, x: i32, y: i32) -> bool {
    if in_range_x(pix, x) && in_range_y(pix, y) {
        match pix.get_pixel(x, y) {
            Ok(pixel_value) => pixel_value == 1,
            Err(_) => false,
        }
    } else {
        false
    }
}

fn in_range_x(pix: &Pix, x: i32) -> bool {
    x >= 0 && x < pix.width()
}

fn in_range_y(pix: &Pix, y: i32) -> bool {
    y >= 0 && y < pix.height()
}

fn line_contain_black_horiz(pix: &Pix, start_x: i32, start_y: i32, width: i32) -> bool {
    let mut x = start_x;
    let end_x = start_x + width;

    while x <= end_x && in_range_x(pix, x) {
        if is_black(pix, x, start_y) {
            return true;
        }
        x += 1;
    }

    false
}

fn line_contain_black_vert(pix: &Pix, start_x: i32, start_y: i32, height: i32) -> bool {
    let mut y = start_y;
    let end_y = start_y + height;

    while y <= end_y && in_range_y(pix, y) {
        if is_black(pix, start_x, y) {
            return true;
        }
        y += 1;
    }

    false
}

fn try_expand_rect(pix: &Pix, rect: &mut BoundingBox, dir: D8, dist: i32) -> bool {
    match dir {
        D8::Top => {
            if line_contain_black_horiz(pix, rect.x, rect.y - dist, rect.w) {
                rect.y -= dist;
                rect.h += dist;
                return true;
            }
        }
        D8::TopRight => {
            if is_black(pix, rect.x + rect.w + dist, rect.y - dist) {
                rect.y -= dist;
                rect.h += dist;
                rect.w += dist;
                return true;
            }
        }
        D8::Right => {
            if line_contain_black_vert(pix, rect.x + rect.w + dist, rect.y, rect.h) {
                rect.w += dist;
                return true;
            }
        }
        D8::BottomRight => {
            if is_black(pix, rect.x + rect.w + dist, rect.y + rect.h + dist) {
                rect.h += dist;
                rect.w += dist;
                return true;
            }
        }
        D8::Bottom => {
            if line_contain_black_horiz(pix, rect.x, rect.y + rect.h + dist, rect.w) {
                rect.h += dist;
                return true;
            }
        }
        D8::BottomLeft => {
            if is_black(pix, rect.x - dist, rect.y + rect.h + dist) {
                rect.x -= dist;
                rect.h += dist;
                rect.w += dist;
                return true;
            }
        }
        D8::Left => {
            if line_contain_black_vert(pix, rect.x - dist, rect.y, rect.h) {
                rect.x -= dist;
                rect.w += dist;
                return true;
            }
        }
        D8::TopLeft => {
            if is_black(pix, rect.x - dist, rect.y + dist) {
                rect.x -= dist;
                rect.y -= dist;
                rect.h += dist;
                rect.w += dist;
                return true;
            }
        }
    }

    false
}

fn expand_rect(pix: &Pix, dir_dist_list: &[DirDist], rect: &mut BoundingBox, keep_going: bool) {
    let mut i: usize = 0;

    loop {
        if i >= dir_dist_list.len() {
            return;
        }

        let dir_dist = dir_dist_list[i];
        let has_black = try_expand_rect(pix, rect, dir_dist.dir, dir_dist.dist);

        if !has_black {
            i += 1;
        } else if !keep_going {
            return;
        }
    }
}
