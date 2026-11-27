pub mod decode;
pub mod resize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn centre(&self) -> f64 {
        f64::from(self.left + self.right) / 2.0
    }

    pub fn joined(&self, other: &Rect) -> Rect {
        Rect {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

pub struct Mask {
    pub pixels: Vec<bool>,
    pub width: usize,
}

impl Mask {
    pub fn height(&self) -> usize {
        self.pixels.len() / self.width
    }

    pub fn crop(&self, rect: &Rect) -> Mask {
        let (left, right) = (rect.left as usize, rect.right as usize);
        let pixels = (rect.top as usize..rect.bottom as usize)
            .flat_map(|y| self.pixels[y * self.width + left..y * self.width + right].iter())
            .copied()
            .collect();
        Mask {
            pixels,
            width: right - left,
        }
    }
}
