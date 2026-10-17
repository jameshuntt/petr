//! Spatial routing examples: arithmetic quadrant routing and Morton codes.

use petr_core::SelectorWord;

/// 2D point in normalized integer space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point2 {
    /// X coordinate.
    pub x: u32,
    /// Y coordinate.
    pub y: u32,
}

/// Morton/Z-order helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Morton2(pub u64);

impl Morton2 {
    /// Encode a point into a simple Morton code.
    pub fn encode(p: Point2) -> Self {
        let mut out = 0u64;
        for i in 0..32 {
            out |= (((p.x as u64 >> i) & 1) << (2 * i)) | (((p.y as u64 >> i) & 1) << (2 * i + 1));
        }
        Self(out)
    }
}

/// A quadtree route step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadStep {
    /// Level/depth.
    pub level: u8,
    /// Quadrant 0..=3.
    pub quadrant: u8,
    /// Selector word representing spatial behavior.
    pub selector: SelectorWord,
}

/// Branchless-style quadtree router.
#[derive(Debug, Clone, Copy)]
pub struct BranchlessQuadtree {
    depth: u8,
}

impl BranchlessQuadtree {
    /// Create a router with fixed depth.
    pub const fn new(depth: u8) -> Self {
        Self { depth }
    }

    /// Route a point by arithmetic quadrant extraction.
    pub fn route(self, point: Point2) -> Vec<QuadStep> {
        let morton = Morton2::encode(point).0;
        let mut steps = Vec::with_capacity(self.depth as usize);
        for level in 0..self.depth {
            let shift = (self.depth - level - 1) * 2;
            let quadrant = ((morton >> shift) & 0b11) as u8;
            let selector = SelectorWord::new(0x0A, quadrant & 0x0F, level);
            steps.push(QuadStep {
                level,
                quadrant,
                selector,
            });
        }
        steps
    }
}
