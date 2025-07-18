use std::cell::LazyCell;

use typst_library::diag::SourceResult;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Locator;
use typst_library::layout::{BoxElem, Frame, FrameKind, Size};
use typst_library::visualize::Stroke;
use typst_utils::Numeric;

use crate::flow::unbreakable_pod;
use crate::shapes::{clip_rect, fill_and_stroke};

/// Lay out a box as part of inline layout.
#[typst_macros::time(name = "box", span = elem.span())]
pub fn layout_box(
    elem: &Packed<BoxElem>,
    engine: &mut Engine,
    locator: Locator,
    styles: StyleChain,
    region: Size,
) -> SourceResult<Frame> {
    // Fetch sizing properties.
    let width = elem.width.get(styles);
    let height = elem.height.get(styles);
    let inset = elem.inset.resolve(styles).unwrap_or_default();

    // Build the pod region.
    let pod = unbreakable_pod(&width, &height.into(), &inset, styles, region);

    // Layout the body.
    let mut frame = match elem.body.get_ref(styles) {
        // If we have no body, just create an empty frame. If necessary,
        // its size will be adjusted below.
        None => Frame::hard(Size::zero()),

        // If we have a child, layout it into the body. Boxes are boundaries
        // for gradient relativeness, so we set the `FrameKind` to `Hard`.
        Some(body) => crate::layout_frame(engine, body, locator, styles, pod)?
            .with_kind(FrameKind::Hard),
    };

    // Enforce a correct frame size on the expanded axes. Do this before
    // applying the inset, since the pod shrunk.
    frame.set_size(pod.expand.select(pod.size, frame.size()));

    // Apply the inset.
    if !inset.is_zero() {
        crate::pad::grow(&mut frame, &inset);
    }

    // Prepare fill and stroke.
    let fill = elem.fill.get_cloned(styles);
    let stroke = elem
        .stroke
        .resolve(styles)
        .unwrap_or_default()
        .map(|s| s.map(Stroke::unwrap_or_default));

    // Only fetch these if necessary (for clipping or filling/stroking).
    let outset = LazyCell::new(|| elem.outset.resolve(styles).unwrap_or_default());
    let radius = LazyCell::new(|| elem.radius.resolve(styles).unwrap_or_default());

    // Clip the contents, if requested.
    if elem.clip.get(styles) {
        frame.clip(clip_rect(frame.size(), &radius, &stroke, &outset));
    }

    // Add fill and/or stroke.
    if fill.is_some() || stroke.iter().any(Option::is_some) {
        fill_and_stroke(&mut frame, fill, &stroke, &outset, &radius, elem.span());
    }

    // Assign label to the frame.
    if let Some(label) = elem.label() {
        frame.label(label);
    }

    // Apply baseline shift. Do this after setting the size and applying the
    // inset, so that a relative shift is resolved relative to the final
    // height.
    let shift = elem.baseline.resolve(styles).relative_to(frame.height());
    if !shift.is_zero() {
        frame.set_baseline(frame.baseline() - shift);
    }

    Ok(frame)
}
