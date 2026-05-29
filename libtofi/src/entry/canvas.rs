//! tiny-skia drawing context with a Cairo-like transform and clip stack.

use tiny_skia::{
    BlendMode, Color, FillRule, Paint, Path, PathBuilder, PixmapMut, Rect, Stroke, Transform,
};

use crate::color::Color as TofiColor;

#[derive(Clone, Copy)]
struct CanvasState {
    tx: f64,
    ty: f64,
    clip: Option<Rect>,
}

/// Logical-pixel canvas backed by a premultiplied RGBA [`PixmapMut`].
pub struct Canvas<'a> {
    pixmap: &'a mut PixmapMut<'a>,
    scale: f64,
    stack: Vec<CanvasState>,
    tx: f64,
    ty: f64,
    clip: Option<Rect>,
}

impl<'a> Canvas<'a> {
    pub fn new(pixmap: &'a mut PixmapMut<'a>, scale: f64) -> Self {
        Self {
            pixmap,
            scale,
            stack: Vec::new(),
            tx: 0.0,
            ty: 0.0,
            clip: None,
        }
    }

    pub fn save(&mut self) {
        self.stack.push(CanvasState {
            tx: self.tx,
            ty: self.ty,
            clip: self.clip,
        });
    }

    pub fn restore(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.tx = state.tx;
            self.ty = state.ty;
            self.clip = state.clip;
        }
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.tx += dx;
        self.ty += dy;
    }

    pub fn matrix_x0(&self) -> f64 {
        self.tx
    }

    pub fn matrix_y0(&self) -> f64 {
        self.ty
    }

    pub fn set_clip_logical(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let rect = logical_rect_to_physical(self.tx + x, self.ty + y, w, h, self.scale);
        self.clip = Some(match self.clip {
            Some(existing) => existing.intersect(&rect).unwrap_or(empty_rect()),
            None => rect,
        });
    }

    pub fn paint_solid(&mut self, color: TofiColor) {
        let sk_color = tofi_to_sk_color(color);
        if let Some(clip) = self.clip {
            self.pixmap
                .fill_rect(clip, &solid_paint(sk_color), Transform::identity(), None);
        } else {
            self.pixmap.fill(sk_color);
        }
    }

    pub fn fill_rounded_rect(&mut self, width: f64, height: f64, radius: f64, color: TofiColor) {
        let path = rounded_rectangle_path(width, height, radius);
        self.fill_path(&path, &solid_paint(tofi_to_sk_color(color)));
    }

    pub fn stroke_rounded_rect_preserve(
        &mut self,
        width: f64,
        height: f64,
        radius: f64,
        line_width: f64,
        color: TofiColor,
    ) {
        let path = rounded_rectangle_path(width, height, radius);
        let mut paint = solid_paint(tofi_to_sk_color(color));
        paint.anti_alias = true;
        let stroke = Stroke {
            width: (line_width * self.scale) as f32,
            ..Stroke::default()
        };
        self.stroke_path(&path, &paint, &stroke);
    }

    pub fn fill_even_odd_clear(&mut self, width: f64, height: f64, radius: f64) {
        let mut pb = PathBuilder::new();
        if let Some(outer) = Rect::from_xywh(0.0, 0.0, width as f32 + 1.0, height as f32 + 1.0) {
            pb.push_rect(outer);
        }
        pb.push_path(&rounded_rectangle_path(width, height, radius));
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, 255);
            paint.blend_mode = BlendMode::Clear;
            self.fill_path_with_rule(&path, &paint, FillRule::EvenOdd);
        }
    }

    pub fn fill_pixels<F>(&mut self, x: i32, y: i32, w: u32, h: u32, mut f: F)
    where
        F: FnMut(u32, u32) -> [u8; 4],
    {
        use tiny_skia::PremultipliedColorU8;

        let scale = self.scale;
        let base_x = self.tx * scale;
        let base_y = self.ty * scale;
        let px = base_x + x as f64 * scale;
        let py = base_y + y as f64 * scale;
        let pw = w as f64 * scale;
        let ph = h as f64 * scale;
        let stride = self.pixmap.width() as usize;

        let start_x = px.floor().max(0.0) as u32;
        let start_y = py.floor().max(0.0) as u32;
        let end_x = (px + pw).ceil().min(self.pixmap.width() as f64) as u32;
        let end_y = (py + ph).ceil().min(self.pixmap.height() as f64) as u32;
        let pixels = self.pixmap.pixels_mut();

        for py_i in start_y..end_y {
            for px_i in start_x..end_x {
                if let Some(clip) = self.clip
                    && (px_i as f32 + 0.5 < clip.left()
                        || py_i as f32 + 0.5 < clip.top()
                        || px_i as f32 + 0.5 >= clip.right()
                        || py_i as f32 + 0.5 >= clip.bottom())
                {
                    continue;
                }

                let lx = ((px_i as f64 + 0.5 - px) / scale) as u32;
                let ly = ((py_i as f64 + 0.5 - py) / scale) as u32;
                if lx >= w || ly >= h {
                    continue;
                }

                let [r, g, b, a] = f(lx, ly);
                if a == 0 {
                    continue;
                }

                let idx = py_i as usize * stride + px_i as usize;
                let Some(px_color) = pixels.get_mut(idx) else {
                    continue;
                };

                if let Some(c) = PremultipliedColorU8::from_rgba(r, g, b, a) {
                    *px_color = c;
                }
            }
        }
    }

    fn fill_path(&mut self, path: &Path, paint: &Paint<'_>) {
        self.fill_path_with_rule(path, paint, FillRule::Winding);
    }

    fn fill_path_with_rule(&mut self, path: &Path, paint: &Paint<'_>, rule: FillRule) {
        let transform = self.current_transform(0.0, 0.0);
        self.pixmap.fill_path(path, paint, rule, transform, None);
    }

    fn stroke_path(&mut self, path: &Path, paint: &Paint<'_>, stroke: &Stroke) {
        let transform = self.current_transform(0.0, 0.0);
        self.pixmap
            .stroke_path(path, paint, stroke, transform, None);
    }

    fn current_transform(&self, dx: f64, dy: f64) -> Transform {
        let s = self.scale as f32;
        Transform::from_translate((self.tx + dx) as f32 * s, (self.ty + dy) as f32 * s)
            .post_scale(s, s)
    }
}

fn empty_rect() -> Rect {
    Rect::from_xywh(0.0, 0.0, 0.0, 0.0).unwrap()
}

fn logical_rect_to_physical(x: f64, y: f64, w: f64, h: f64, scale: f64) -> Rect {
    Rect::from_xywh(
        (x * scale) as f32,
        (y * scale) as f32,
        (w * scale) as f32,
        (h * scale) as f32,
    )
    .unwrap_or(empty_rect())
}

fn solid_paint(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    paint
}

fn tofi_to_sk_color(c: TofiColor) -> Color {
    Color::from_rgba8(
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8,
        (c.a * 255.0).round() as u8,
    )
}

/// Build a rounded rectangle path at the origin.
pub fn rounded_rectangle_path(width: f64, height: f64, r: f64) -> Path {
    if r <= 0.0 {
        return PathBuilder::from_rect(
            Rect::from_xywh(0.0, 0.0, width as f32, height as f32).unwrap_or(empty_rect()),
        );
    }

    let w = width as f32;
    let h = height as f32;
    let r = r.min(width / 2.0).min(height / 2.0) as f32;
    let k = r * 0.552_284_8;

    let mut pb = PathBuilder::new();
    pb.move_to(r, 0.0);
    pb.line_to(w - r, 0.0);
    pb.cubic_to(w - r + k, 0.0, w, r - k, w, r);
    pb.line_to(w, h - r);
    pb.cubic_to(w, h - r + k, w - r + k, h, w - r, h);
    pb.line_to(r, h);
    pb.cubic_to(r - k, h, 0.0, h - r + k, 0.0, h - r);
    pb.line_to(0.0, r);
    pb.cubic_to(0.0, r - k, r - k, 0.0, r, 0.0);
    pb.close();
    pb.finish().unwrap()
}
