use super::*;

#[test]
fn test_bounds_center() {
    let bounds = Bounds {
        x: 100,
        y: 200,
        width: 50,
        height: 30,
    };
    assert_eq!(bounds.center(), (125, 215));
}
