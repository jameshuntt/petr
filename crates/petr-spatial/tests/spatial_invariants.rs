use petr_spatial::{BranchlessQuadtree, Morton2, Point2};

#[test]
fn morton_encoding_interleaves_low_bits_exactly() {
    assert_eq!(Morton2::encode(Point2 { x: 0b0, y: 0b0 }).0 & 0b11_1111, 0b00_00_00);
    assert_eq!(Morton2::encode(Point2 { x: 0b1, y: 0b0 }).0 & 0b11, 0b01);
    assert_eq!(Morton2::encode(Point2 { x: 0b0, y: 0b1 }).0 & 0b11, 0b10);
    assert_eq!(Morton2::encode(Point2 { x: 0b1, y: 0b1 }).0 & 0b11, 0b11);

    // x=101, y=010 => quadrants from high-to-low depth bits: 1, 2, 1.
    assert_eq!(Morton2::encode(Point2 { x: 0b101, y: 0b010 }).0 & 0b11_11_11, 0b01_10_01);
}

#[test]
fn quadtree_route_extracts_quadrants_from_morton_code_high_to_low() {
    let route = BranchlessQuadtree::new(3).route(Point2 { x: 0b101, y: 0b010 });
    let quadrants: Vec<u8> = route.iter().map(|s| s.quadrant).collect();
    assert_eq!(quadrants, vec![1, 2, 1]);

    for (level, step) in route.iter().enumerate() {
        assert_eq!(step.level, level as u8);
        assert_eq!(step.selector.trait_id(), 0x0A);
        assert_eq!(step.selector.impl_id(), step.quadrant);
        assert_eq!(step.selector.function_id(), step.level);
    }
}

#[test]
fn quadtree_route_depth_controls_number_of_steps() {
    for depth in 0..12u8 {
        let steps = BranchlessQuadtree::new(depth).route(Point2 { x: 123, y: 456 });
        assert_eq!(steps.len(), depth as usize);
    }
}
