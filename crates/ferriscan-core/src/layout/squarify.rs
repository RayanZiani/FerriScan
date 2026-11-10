//! Squarified treemap layout (Bruls, Huizing, van Wijk 2000).
//!
//! The algorithm receives weights already sorted in descending order and emits
//! rectangles that fill a fixed area. Keeping it pure and deterministic means the
//! geometry can be golden-tested without a renderer.

/// Axis-aligned rectangle in layout space.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// Area of the rectangle.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.w.max(0.0) * self.h.max(0.0)
    }

    /// Whether the rectangle is large enough to be drawn and hit-tested.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.w >= 1.0 && self.h >= 1.0
    }
}

/// Lays out `weights` (descending, positive) inside `area`.
///
/// Zero or negative weights are treated as zero and receive no rectangle. The
/// returned vector always has the same length as `weights`.
#[must_use]
pub fn squarify(weights: &[f64], area: Rect) -> Vec<Rect> {
    let mut out = vec![Rect::default(); weights.len()];
    let positive: Vec<(usize, f64)> = weights
        .iter()
        .enumerate()
        .filter(|(_, w)| **w > 0.0)
        .map(|(i, w)| (i, *w))
        .collect();
    if positive.is_empty() {
        return out;
    }

    let total: f64 = positive.iter().map(|(_, w)| w).sum();
    if total <= 0.0 || !total.is_finite() {
        return out;
    }

    let scale = f64::from(area.area()) / total;
    let scaled: Vec<(usize, f64)> = positive.iter().map(|(i, w)| (*i, w * scale)).collect();

    let mut free = area;
    let mut start = 0usize;
    while start < scaled.len() {
        let remaining_area: f64 = scaled[start..].iter().map(|(_, a)| a).sum();
        if remaining_area <= f64::EPSILON {
            break;
        }
        let short_side = f64::from(free.w.min(free.h));
        if short_side <= 0.0 {
            break;
        }

        // Grow the row while the worst aspect ratio improves.
        let mut end = start + 1;
        let first = scaled[start].1;
        let mut row_sum = first;
        let mut worst = worst_ratio(short_side, first, row_sum);
        while end < scaled.len() {
            let next_sum = row_sum + scaled[end].1;
            let next_worst = worst_ratio(short_side, first, next_sum);
            if next_worst > worst {
                break;
            }
            row_sum = next_sum;
            worst = next_worst;
            end += 1;
        }

        free = place_row(&scaled[start..end], free, row_sum, &mut out);
        start = end;
    }

    out
}

fn worst_ratio(side: f64, first: f64, total: f64) -> f64 {
    if side <= 0.0 || first <= 0.0 || total <= 0.0 {
        return f64::INFINITY;
    }
    let side_sq = side * side;
    let total_sq = total * total;
    (side_sq * first / total_sq).max(total_sq / (side_sq * first))
}

fn place_row(row: &[(usize, f64)], free: Rect, row_sum: f64, out: &mut [Rect]) -> Rect {
    if row.is_empty() || row_sum <= 0.0 {
        return free;
    }
    // The row lies along the short side of the free area; its thickness is the row's area
    // divided by that side, and each item takes a share of the side proportional to its weight.
    let tall_columns = free.w >= free.h;
    let side = f64::from(if tall_columns { free.h } else { free.w });
    let extent = row_sum / side;
    let mut cursor = 0.0f64;

    for (idx, weight) in row {
        let span = weight / extent;
        let (x, y, w, h) = if tall_columns {
            (f64::from(free.x), f64::from(free.y) + cursor, extent, span)
        } else {
            (f64::from(free.x) + cursor, f64::from(free.y), span, extent)
        };
        // Clamp to the free area: floating-point accumulation can produce a
        // sub-pixel overflow that would leak outside the canvas.
        let max_x = f64::from(free.x + free.w);
        let max_y = f64::from(free.y + free.h);
        let x = x.min(max_x);
        let y = y.min(max_y);
        let w = w.min(max_x - x).max(0.0);
        let h = h.min(max_y - y).max(0.0);
        out[*idx] = Rect {
            x: x as f32,
            y: y as f32,
            w: w as f32,
            h: h as f32,
        };
        cursor += span;
    }

    let extent32 = extent as f32;
    if tall_columns {
        Rect {
            x: free.x + extent32,
            y: free.y,
            w: (free.w - extent32).max(0.0),
            h: free.h,
        }
    } else {
        Rect {
            x: free.x,
            y: free.y + extent32,
            w: free.w,
            h: (free.h - extent32).max(0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty() {
        assert!(
            squarify(
                &[],
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 100.0
                }
            )
            .is_empty()
        );
    }

    #[test]
    fn single_weight_fills_the_area() {
        let rects = squarify(
            &[10.0],
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        assert_eq!(rects.len(), 1);
        assert!((rects[0].area() - 5000.0).abs() < 1.0);
    }

    #[test]
    fn rectangles_stay_inside_the_area() {
        let weights = [50.0, 30.0, 12.0, 5.0, 2.0, 1.0];
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 300.0,
            h: 200.0,
        };
        for r in squarify(&weights, area) {
            assert!(r.x >= -0.01 && r.y >= -0.01);
            assert!(r.x + r.w <= area.w + 0.01);
            assert!(r.y + r.h <= area.h + 0.01);
        }
    }

    #[test]
    fn rectangles_tile_the_area_in_proportion_to_their_weights() {
        let weights = [50.0, 30.0, 12.0, 5.0, 2.0, 1.0];
        let area = Rect {
            x: 10.0,
            y: 20.0,
            w: 300.0,
            h: 200.0,
        };
        let rects = squarify(&weights, area);
        let total: f64 = weights.iter().sum();
        let mut covered = 0.0f64;
        for (i, r) in rects.iter().enumerate() {
            let expected = f64::from(area.area()) * weights[i] / total;
            assert!(
                (f64::from(r.area()) - expected).abs() < expected * 0.01 + 0.5,
                "tile {i}: {} vs {expected}",
                r.area()
            );
            covered += f64::from(r.area());
            for other in &rects[..i] {
                let overlap_w = (r.x + r.w).min(other.x + other.w) - r.x.max(other.x);
                let overlap_h = (r.y + r.h).min(other.y + other.h) - r.y.max(other.y);
                assert!(overlap_w <= 0.01 || overlap_h <= 0.01, "tiles {i} overlap");
            }
        }
        assert!((covered - f64::from(area.area())).abs() < 5.0);
    }

    #[test]
    fn zero_weights_get_empty_rects() {
        let rects = squarify(
            &[10.0, 0.0, 5.0],
            Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
        );
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[1], Rect::default());
        assert!(rects[0].area() > 0.0);
    }

    #[test]
    fn layout_is_deterministic() {
        let weights = [40.0, 25.0, 20.0, 10.0, 5.0];
        let area = Rect {
            x: 5.0,
            y: 7.0,
            w: 320.0,
            h: 180.0,
        };
        assert_eq!(squarify(&weights, area), squarify(&weights, area));
    }
}
