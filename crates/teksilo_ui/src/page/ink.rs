use crate::{
    icons::Icon,
    session_vm::SessionVm,
    widgets::{Rebuild, caption, kbd},
};
use ::common::types::EntityId;
use noton_format::{Sample, Stroke};
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};
use teksilo::core::binding::BindingLevel;
use teksilo::tokens::VAlignment;
use teksilo::widgets::color_picker::ColorSwatch;
use teksilo::{prelude::*, widgets::*};

/// Whole-stroke erasure tests segments, including sparse pen traces, not just samples.
fn eraser_hits(eraser: &[Sample], ink: &[Sample], radius: f64) -> bool {
    fn cross(a: &Sample, b: &Sample, p: &Sample) -> f64 {
        (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
    }
    fn distance(p: &Sample, a: &Sample, b: &Sample) -> f64 {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length = dx * dx + dy * dy;
        let t = if length > 0. {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0., 1.)
        } else {
            0.
        };
        (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
    }
    eraser.iter().enumerate().any(|(i, a)| {
        let b = eraser.get(i + 1).unwrap_or(a);
        ink.iter().enumerate().any(|(j, c)| {
            let d = ink.get(j + 1).unwrap_or(c);
            let proper_crossing =
                cross(a, b, c) * cross(a, b, d) < 0. && cross(c, d, a) * cross(c, d, b) < 0.;
            proper_crossing
                || distance(a, c, d)
                    .min(distance(b, c, d))
                    .min(distance(c, a, b))
                    .min(distance(d, a, b))
                    <= radius
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InkMode {
    Draw,
    Erase,
    Select,
    Pan,
}
const MODES: [InkMode; 4] = [InkMode::Draw, InkMode::Erase, InkMode::Select, InkMode::Pan];
/// Brush widths in sample space: fine, medium, thick.
pub const WIDTHS: [f64; 3] = [1.5, 2.5, 4.5];
/// Ink colours, RGBA: ink, vermilion, blue, green, ochre.
pub const COLORS: [u32; 5] = [0x2B2724FF, 0xB03D1AFF, 0x2456A6FF, 0x2F6B3AFF, 0xA86A00FF];

fn rgba(value: u32) -> Color {
    Color::from_rgba(
        (value >> 24) as f32 / 255.,
        ((value >> 16) & 0xff) as f32 / 255.,
        ((value >> 8) & 0xff) as f32 / 255.,
        (value & 0xff) as f32 / 255.,
    )
}

/// The drawing tools, shared by every sketch.
#[derive(Debug, Clone)]
pub struct InkTools {
    /// The tool: 0 draw, 1 erase, 2 select, 3 pan.
    pub mode: Signal<usize>,
    /// Index into [`WIDTHS`].
    pub width: Signal<usize>,
    /// Index into [`COLORS`].
    pub color: Signal<usize>,
}
impl Default for InkTools {
    fn default() -> Self {
        Self {
            mode: Signal::new(0),
            width: Signal::new(1),
            color: Signal::new(0),
        }
    }
}

#[derive(Debug)]
struct InkState {
    samples: Vec<Sample>,
    pointer: Option<teksilo::core::pointer::PointerId>,
    strokes: Vec<(EntityId, Stroke)>,
    selected: BTreeSet<EntityId>,
    pan_from: Option<Point>,
}
#[derive(Debug)]
pub struct InkSurface {
    vm: SessionVm,
    block: EntityId,
    state: Rc<RefCell<InkState>>,
    revision: Signal<u64>,
    size: Rc<std::cell::Cell<(f32, f32)>>,
}
impl InkSurface {
    pub fn new(vm: SessionVm, block: EntityId) -> anyhow::Result<Self> {
        let b = vm.0.service.block(block)?;
        let mut strokes = vec![];
        for id in b.strokes {
            let s = vm.0.service.ink_stroke(id)?;
            if !s.trashed {
                strokes.push((
                    id,
                    Stroke {
                        uid: s.uid,
                        trashed: false,
                        version: 1,
                        width: s.brush_width,
                        rgba: s.brush_rgba as u32,
                        pressure: s.use_pressure,
                        samples: serde_json::from_str(&s.samples_json)?,
                    },
                ));
            }
        }
        Ok(Self {
            vm,
            block,
            state: Rc::new(RefCell::new(InkState {
                samples: vec![],
                pointer: None,
                strokes,
                selected: BTreeSet::new(),
                pan_from: None,
            })),
            revision: Signal::new(0),
            size: Rc::new(std::cell::Cell::new((560., 320.))),
        })
    }
}

fn erase(
    vm: &SessionVm,
    block: EntityId,
    s: &mut InkState,
    ids: Vec<EntityId>,
) -> anyhow::Result<()> {
    if !ids.is_empty() {
        vm.0.service.erase_strokes(block, &ids, vm.page_stack()?)?;
        s.strokes.retain(|(id, _)| !ids.contains(id));
        s.selected.retain(|id| !ids.contains(id));
        vm.changed();
    }
    Ok(())
}

impl Widget for InkSurface {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        self.revision.bind_to(
            ctx.self_id(),
            ctx.binding_registry(),
            BindingLevel::RepaintOnly,
        );
        let state = self.state.clone();
        let vm = self.vm.clone();
        let id = self.block;
        let revision = self.revision.clone();
        let size = self.size.clone();
        let tools = self.vm.0.ink.clone();
        if self.vm.0.drawing.get() == Some(id) {
            let me = ctx.self_id();
            ctx.run_after_mount(move |ec| ec.request_focus(me));
        }
        let sample = move |position: Point, pressure: Option<f32>| {
            let (w, h) = size.get();
            // A stroke is saved only if every pressure is within 0–1, so a
            // device reporting more (or NaN) must not cost the whole stroke.
            let pressure = pressure.filter(|p| p.is_finite()).unwrap_or(1.);
            Sample {
                x: position.x as f64 * 560. / w as f64,
                y: position.y as f64 * 320. / h as f64,
                pressure: pressure.clamp(0., 1.) as f64,
            }
        };
        ctx.apply_self_handlers(
            teksilo::core::widget_builder::HandlerSet::new()
                .focusable(true)
                .on_pointer_event(move |event, ctx| {
                    if vm.0.drawing.get() != Some(id) {
                        return EventResponse::Ignored;
                    }
                    let mode = MODES[tools.mode.get().min(3)];
                    let mut s = state.borrow_mut();
                    match event {
                        WidgetEvent::PointerDown {
                            position,
                            button: PointerButton::Primary,
                            pointer,
                            ..
                        } if s.pointer.is_none() => {
                            vm.0.edit.claim_structure(false);
                            vm.0.active_block.set(Some(id));
                            vm.0.active_text.set(None);
                            s.samples.clear();
                            s.pointer = Some(pointer.id);
                            ctx.capture_pointer();
                            match mode {
                                InkMode::Pan => s.pan_from = Some(*position),
                                InkMode::Select => {
                                    let p = sample(*position, None);
                                    let hit = s
                                        .strokes
                                        .iter()
                                        .rev()
                                        .find(|(_, st)| eraser_hits(&[p], &st.samples, 8.))
                                        .map(|(id, _)| *id);
                                    match hit {
                                        Some(h) if !s.selected.remove(&h) => {
                                            s.selected.insert(h);
                                        }
                                        None => s.selected.clear(),
                                        _ => {}
                                    }
                                }
                                _ => s.samples.push(sample(*position, pointer.axes.pressure)),
                            }
                        }
                        WidgetEvent::PointerMove {
                            position, pointer, ..
                        } if s.pointer == Some(pointer.id) => match mode {
                            InkMode::Pan => {
                                if let (Some(from), Some(c)) =
                                    (s.pan_from, vm.0.camera.borrow().as_ref())
                                {
                                    c.pan_x.set(c.pan_x.get() + position.x - from.x);
                                    c.pan_y.set(c.pan_y.get() + position.y - from.y);
                                }
                            }
                            InkMode::Select => {}
                            _ => s.samples.push(sample(*position, pointer.axes.pressure)),
                        },
                        WidgetEvent::PointerUp {
                            position, pointer, ..
                        } if s.pointer == Some(pointer.id) => {
                            s.pointer = None;
                            s.pan_from = None;
                            ctx.release_pointer();
                            if matches!(mode, InkMode::Pan | InkMode::Select) {
                                revision.set(revision.get() + 1);
                                return EventResponse::Handled;
                            }
                            s.samples.push(sample(*position, pointer.axes.pressure));
                            let samples = std::mem::take(&mut s.samples);
                            let result = (|| -> anyhow::Result<()> {
                                if mode == InkMode::Erase {
                                    let ids: Vec<_> = s
                                        .strokes
                                        .iter()
                                        .filter(|(_, stroke)| {
                                            eraser_hits(&samples, &stroke.samples, 12.)
                                        })
                                        .map(|(id, _)| *id)
                                        .collect();
                                    erase(&vm, id, &mut s, ids)?;
                                } else if !samples.is_empty() {
                                    let stroke = Stroke {
                                        uid: uuid::Uuid::new_v4(),
                                        trashed: false,
                                        version: 1,
                                        width: WIDTHS[tools.width.get().min(2)],
                                        rgba: COLORS[tools.color.get().min(4)],
                                        pressure: true,
                                        samples,
                                    };
                                    let sid = vm.0.service.add_stroke(
                                        id,
                                        stroke.clone(),
                                        vm.page_stack()?,
                                    )?;
                                    s.strokes.push((sid, stroke));
                                    vm.changed();
                                }
                                Ok(())
                            })();
                            vm.report(result);
                        }
                        WidgetEvent::PointerCancel { pointer, .. }
                            if s.pointer == Some(pointer.id) =>
                        {
                            s.pointer = None;
                            s.pan_from = None;
                            s.samples.clear();
                        }
                        _ => return EventResponse::Ignored,
                    }
                    revision.set(revision.get() + 1);
                    EventResponse::Handled
                })
                .on_key({
                    let s = self.state.clone();
                    let r = self.revision.clone();
                    let vm = self.vm.clone();
                    let tools = self.vm.0.ink.clone();
                    move |event, ctx| {
                        let WidgetEvent::KeyDown { key, modifiers, .. } = event else {
                            return EventResponse::Ignored;
                        };
                        if vm.0.drawing.get() != Some(id) || modifiers.command() {
                            return EventResponse::Ignored;
                        }
                        match key {
                            Key::Escape => {
                                let stroke_in_progress =
                                    s.borrow().pointer.is_some() || !s.borrow().samples.is_empty();
                                if stroke_in_progress {
                                    s.borrow_mut().samples.clear();
                                    s.borrow_mut().pointer = None;
                                    ctx.release_pointer();
                                } else {
                                    vm.leave_drawing();
                                }
                            }
                            Key::D => tools.mode.set(0),
                            Key::E => tools.mode.set(1),
                            Key::V => tools.mode.set(2),
                            Key::Character('[') => {
                                tools.width.set(tools.width.get().saturating_sub(1))
                            }
                            Key::Character(']') => tools.width.set((tools.width.get() + 1).min(2)),
                            Key::Delete | Key::Backspace => {
                                let ids: Vec<_> = s.borrow().selected.iter().copied().collect();
                                let result = erase(&vm, id, &mut s.borrow_mut(), ids);
                                vm.report(result);
                            }
                            _ => return EventResponse::Ignored,
                        }
                        r.set(r.get() + 1);
                        EventResponse::Handled
                    }
                }),
        );
        vec![]
    }
    fn layout_response(&self, p: SizeProposal, _: &LayoutContext) -> LayoutResponse {
        let w = p.width.unwrap_or(560.).max(80.);
        let h = p.height.unwrap_or(w * 320. / 560.).max(40.);
        self.size.set((w, h));
        Size::new(w, h).into()
    }
    fn paint(&self, bounds: Rect, canvas: &mut Canvas, ctx: &PaintContext) {
        canvas.save();
        canvas.set_clip(bounds);
        // A sheet of paper in both themes, so saved ink colours keep their meaning.
        canvas.fill_rect(bounds, Color::from_hex("#FFFEFB"));
        let state = self.state.borrow();
        let convert = |p: &Sample| {
            Point::new(
                bounds.x + p.x as f32 * bounds.width / 560.,
                bounds.y + p.y as f32 * bounds.height / 320.,
            )
        };
        let paint = |canvas: &mut Canvas, samples: &[Sample], width: f64, color: Color| {
            if samples.len() == 1 {
                let p = convert(&samples[0]);
                canvas.fill_circle(p, (width as f32 / 2.).max(0.5), color);
            }
            for pair in samples.windows(2) {
                canvas.stroke_path(
                    &Path::line(convert(&pair[0]), convert(&pair[1])),
                    color,
                    (width * (0.25 + 0.75 * pair[1].pressure)) as f32,
                );
            }
        };
        let highlight = ctx.theme.colors.selection_bg_active;
        for (id, s) in &state.strokes {
            if state.selected.contains(id) {
                paint(canvas, &s.samples, s.width + 6., highlight);
            }
        }
        for (_, s) in &state.strokes {
            paint(canvas, &s.samples, s.width, rgba(s.rgba));
        }
        let tools = &self.vm.0.ink;
        paint(
            canvas,
            &state.samples,
            WIDTHS[tools.width.get().min(2)],
            rgba(COLORS[tools.color.get().min(4)]),
        );
        canvas.clear_clip();
        canvas.restore();
    }
    fn accessibility(&self, b: &mut AccessNodeBuilder) {
        b.set_role(accesskit::Role::Image);
        b.set_name(
            self.vm
                .0
                .service
                .block(self.block)
                .map(|v| {
                    if v.description.is_empty() {
                        v.label
                    } else {
                        v.description
                    }
                })
                .unwrap_or_default(),
        );
    }
}

/// The reminder shown on a sketch in drawing mode.
pub fn escape_hint() -> impl Widget + 'static {
    HStack::new()
        .spacing(6.)
        .alignment(VAlignment::Center)
        .child(kbd("Esc"))
        .child(caption(tr!(esc_hint())))
        .access_role(accesskit::Role::Status)
}

/// A dot glyph for a brush width.
fn width_icon(radius: f32) -> IconWidget {
    IconWidget::from_svg(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="{radius}" fill="currentColor"/></svg>"#
    ))
    .icon_size(16.)
}

/// The drawing toolbar, shown instead of the formatting toolbar while a sketch
/// is in drawing mode.
pub fn drawing_bar(vm: &SessionVm) -> impl Widget + 'static {
    let tools = &vm.0.ink;
    // Icon segments keep the bar inside a 600 px window; each label becomes
    // the segment's tooltip and accessible name.
    let mode = SegmentedControl::indexed(tools.mode.clone())
        .label(tr!(drawing_tools()))
        .display(SegmentDisplay::Icon)
        .segment(Segment::new(tr!(draw())).icon(|| Icon::Pen.widget()))
        .segment(Segment::new(tr!(erase())).icon(|| Icon::Eraser.widget()))
        .segment(Segment::new(tr!(select())).icon(|| Icon::Pointer.widget()))
        .segment(Segment::new(tr!(pan())).icon(|| Icon::Hand.widget()));
    let width = SegmentedControl::indexed(tools.width.clone())
        .label(tr!(brush_width()))
        .display(SegmentDisplay::Icon)
        .segment(Segment::new(tr!(width_fine())).icon(|| width_icon(2.)))
        .segment(Segment::new(tr!(width_medium())).icon(|| width_icon(3.5)))
        .segment(Segment::new(tr!(width_thick())).icon(|| width_icon(5.5)));
    let color = tools.color.clone();
    let swatches = Rebuild::new(move || {
        let names = [
            tr!(ink_black()),
            tr!(ink_vermilion()),
            tr!(ink_blue()),
            tr!(ink_green()),
            tr!(ink_ochre()),
        ];
        let mut row = HStack::new().spacing(4.).alignment(VAlignment::Center);
        for (i, name) in names.into_iter().enumerate() {
            let c = color.clone();
            row = row.child(
                ColorSwatch::new(rgba(COLORS[i]))
                    .size(20.)
                    .corner_radius(10.)
                    .selected(color.get() == i)
                    .label(name.clone())
                    .tooltip(name)
                    .on_activate_fn(move |_| c.set(i)),
            );
        }
        Box::new(
            row.access_role(accesskit::Role::RadioGroup)
                .access_label(tr!(ink_color())),
        )
    })
    .on(&tools.color);
    let done = vm.clone();
    Padding::new(6., 12., 6., 16.).child(
        HStack::new()
            .spacing(12.)
            .alignment(VAlignment::Center)
            .child(mode)
            .child(width)
            .child(swatches)
            .child(
                crate::widgets::icon_button(Icon::Undo, tr!(undo_stroke()))
                    .on_activate_fn(|ctx| ctx.send_intent(Intent::new("page.undo"))),
            )
            .child(Spacer::new())
            .child(
                Button::new(tr!(done_drawing()))
                    .variant(ButtonVariant::Filled)
                    .icon(Icon::Check.widget(), IconLocation::Leading)
                    .on_activate_fn(move |_| done.leave_drawing()),
            )
            .access_role(accesskit::Role::Toolbar)
            .access_label(tr!(drawing_tools())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64) -> Sample {
        Sample { x, y, pressure: 1. }
    }
    #[test]
    fn eraser_hits_sparse_crossing_and_tap_but_not_distant_strokes() {
        let ink = [p(0., 50.), p(100., 50.)];
        assert!(eraser_hits(&[p(50., 0.), p(50., 100.)], &ink, 12.));
        assert!(eraser_hits(&[p(50., 55.)], &ink, 12.));
        assert!(!eraser_hits(&[p(50., 100.)], &ink, 12.));
        assert!(!eraser_hits(&[], &ink, 12.));
        assert!(eraser_hits(&[p(4., 4.)], &[p(4., 4.)], 12.));
    }
}
