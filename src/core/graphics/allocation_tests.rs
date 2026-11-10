use super::{Region, Screen, VRAM_HEIGHT, VRAM_WIDTH};
use std::rc::Rc;

#[test]
fn reserves_framebuffers_and_allocates_first_fit() {
    let mut screen = Screen::new();
    assert_eq!((VRAM_WIDTH, VRAM_HEIGHT), (960, 480));

    let right = screen.allocate(320, 480).unwrap();
    assert_eq!(
        right.region,
        Region {
            x: 640,
            y: 0,
            width: 320,
            height: 480
        }
    );
    let bottom = screen.allocate(320, 240).unwrap();
    assert_eq!(
        bottom.region,
        Region {
            x: 0,
            y: 240,
            width: 320,
            height: 240
        }
    );
    assert!(screen.allocate(320, 480).is_err());
    let last = screen.allocate(320, 240).unwrap();
    assert_eq!(
        last.region,
        Region {
            x: 320,
            y: 240,
            width: 320,
            height: 240
        }
    );
    assert!(screen.allocate(1, 1).is_err());
}

#[test]
fn reallocation_clears_only_the_reclaimed_region() {
    let mut screen = Screen::new();
    let allocation = screen.allocate(10, 3).unwrap();
    let region = allocation.region;
    for y in region.y..region.y + region.height {
        for x in region.x..region.x + region.width {
            screen.vram[y * VRAM_WIDTH + x] = 0x7fff;
        }
    }
    let neighbor = region.y * VRAM_WIDTH + region.x + region.width;
    screen.vram[neighbor] = 0x1234;
    drop(allocation);

    let replacement = screen.allocate(10, 3).unwrap();
    assert_eq!(replacement.region, region);
    for y in region.y..region.y + region.height {
        for x in region.x..region.x + region.width {
            assert_eq!(screen.vram[y * VRAM_WIDTH + x], 0);
        }
    }
    assert_eq!(screen.vram[neighbor], 0x1234);
}

#[test]
fn fragmented_space_coalesces_after_dropping_neighbors() {
    let mut screen = Screen::new();
    let top_right = screen.allocate(320, 240).unwrap();
    let bottom_left = screen.allocate(320, 240).unwrap();
    let bottom_middle = screen.allocate(320, 240).unwrap();
    let bottom_right = screen.allocate(320, 240).unwrap();
    assert_eq!((top_right.region.x, top_right.region.y), (640, 0));
    assert_eq!((bottom_left.region.x, bottom_left.region.y), (0, 240));
    assert_eq!((bottom_middle.region.x, bottom_middle.region.y), (320, 240));
    assert_eq!((bottom_right.region.x, bottom_right.region.y), (640, 240));

    drop(top_right);
    drop(bottom_left);
    assert!(screen.allocate(640, 240).is_err());
    drop(bottom_middle);
    let combined = screen.allocate(640, 240).unwrap();
    assert_eq!(
        combined.region,
        Region {
            x: 0,
            y: 240,
            width: 640,
            height: 240
        }
    );
}

#[test]
fn invalid_dimensions_do_not_reserve_space() {
    let mut screen = Screen::new();
    for (width, height) in [
        (0, 1),
        (1, 0),
        (-1, 1),
        (1, -1),
        (i64::MIN, 1),
        (1, i64::MIN),
        (i64::MAX, 1),
        (1, i64::MAX),
        (i64::MAX, i64::MAX),
        (VRAM_WIDTH as i64 + 1, 1),
        (1, VRAM_HEIGHT as i64 + 1),
    ] {
        assert!(
            screen.allocate(width, height).is_err(),
            "accepted {width}x{height}"
        );
    }
    let valid = screen.allocate(1, 1).unwrap();
    assert_eq!((valid.region.x, valid.region.y), (640, 0));
}

#[test]
fn allocation_clones_keep_the_region_reserved() {
    let mut screen = Screen::new();
    let allocation = screen.allocate(320, 480).unwrap();
    let clone = Rc::clone(&allocation);
    let bottom = screen.allocate(640, 240).unwrap();
    drop(allocation);
    assert!(screen.allocate(320, 480).is_err());
    drop(clone);
    let replacement = screen.allocate(320, 480).unwrap();
    assert_eq!(
        replacement.region,
        Region {
            x: 640,
            y: 0,
            width: 320,
            height: 480
        }
    );
    assert_eq!((bottom.region.x, bottom.region.y), (0, 240));
}
