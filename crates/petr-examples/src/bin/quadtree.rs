use petr_spatial::{BranchlessQuadtree, Point2};

fn main() {
    let tree = BranchlessQuadtree::new(6);
    let route = tree.route(Point2 { x: 42, y: 19 });
    for step in route {
        println!(
            "level={} quadrant={} selector={:#06x}",
            step.level,
            step.quadrant,
            step.selector.raw()
        );
    }
}
