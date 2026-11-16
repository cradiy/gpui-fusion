use gpui::{prelude::*, *};
use std::time::{Duration, Instant};

pub(super) struct LabelTransition {
    from: Vec<(SharedString, f32)>,
    target: SharedString,
    started: Instant,
}
impl LabelTransition {
    pub(super) fn new(from: SharedString, target: SharedString, now: Instant) -> Self {
        Self {
            from: vec![(from, 1.)],
            target,
            started: now,
        }
    }
    pub(super) fn retarget(&mut self, target: SharedString, now: Instant, duration: Duration) {
        self.from = self.weights(now, duration);
        self.target = target;
        self.started = now;
    }
    fn weights(&self, now: Instant, duration: Duration) -> Vec<(SharedString, f32)> {
        let t = if duration.is_zero() {
            1.
        } else {
            (now.duration_since(self.started).as_secs_f32() / duration.as_secs_f32()).min(1.)
        };
        let t = 1. - (1. - t).powi(3);
        let mut layers: Vec<_> = self
            .from
            .iter()
            .map(|(text, weight)| (text.clone(), weight * (1. - t)))
            .collect();
        if let Some((_, weight)) = layers.iter_mut().find(|(text, _)| *text == self.target) {
            *weight += t;
        } else {
            layers.push((self.target.clone(), t));
        }
        layers.retain(|(_, weight)| *weight > 0.01);
        let total: f32 = layers.iter().map(|(_, weight)| weight).sum();
        for (_, weight) in &mut layers {
            *weight /= total;
        }
        layers
    }
    pub(super) fn finished(&self, now: Instant, duration: Duration) -> bool {
        now.duration_since(self.started) >= duration
    }
    pub(super) fn render(
        &self,
        style: TextStyle,
        window: &mut Window,
        duration: Duration,
        label_scale: f32,
    ) -> AnyElement {
        let now = Instant::now();
        let height = style.line_height_in_pixels(window.rem_size());
        let font_size = style.font_size.to_pixels(window.rem_size());
        let lines: Vec<_> = self
            .weights(now, duration)
            .into_iter()
            .map(|(text, weight)| {
                let mut color = style.color;
                color.a *= weight;
                let run = TextRun {
                    len: text.len(),
                    font: style.font(),
                    color,
                    background_color: None,
                    underline: style.underline,
                    strikethrough: style.strikethrough,
                };
                (
                    window
                        .text_system()
                        .shape_line(text, font_size, &[run], None),
                    weight,
                )
            })
            .collect();
        let width = lines
            .iter()
            .fold(px(0.), |sum, (line, weight)| sum + line.width * *weight);
        window.request_animation_frame();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                for (line, _) in lines {
                    let scale = (bounds.size.width / line.width.max(px(1.))).min(1.) * label_scale;
                    let transform = GlyphRunTransform::new(
                        0..line.len(),
                        point(line.width * 0.5, height * 0.5),
                        Transformation::scale(size(scale, scale)),
                    );
                    let _ = line.paint_with_transforms(
                        bounds.origin,
                        height,
                        TextAlign::Center,
                        Some(bounds.size.width),
                        &[transform],
                        window,
                        cx,
                    );
                }
            },
        )
        .w(width)
        .h(height)
        .into_any_element()
    }
}

pub(super) fn scaled_label(
    text: SharedString,
    style: TextStyle,
    scale: f32,
    window: &mut Window,
) -> AnyElement {
    let height = style.line_height_in_pixels(window.rem_size());
    let font_size = style.font_size.to_pixels(window.rem_size());
    let run = TextRun {
        len: text.len(),
        font: style.font(),
        color: style.color,
        background_color: None,
        underline: style.underline,
        strikethrough: style.strikethrough,
    };
    let line = window
        .text_system()
        .shape_line(text, font_size, &[run], None);
    let width = line.width;
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let transform = GlyphRunTransform::new(
                0..line.len(),
                point(width * 0.5, height * 0.5),
                Transformation::scale(size(scale, scale)),
            );
            let _ = line.paint_with_transforms(
                bounds.origin,
                height,
                TextAlign::Center,
                Some(width),
                &[transform],
                window,
                cx,
            );
        },
    )
    .w(width)
    .h(height)
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::LabelTransition;
    use gpui::SharedString;
    use std::time::{Duration, Instant};

    #[test]
    fn interrupted_label_transition_preserves_visible_layers_and_releases_history() {
        let start = Instant::now();
        let duration = Duration::from_millis(180);
        let mut motion = LabelTransition::new("Connect".into(), "Disconnect".into(), start);
        let interrupted = start + duration / 3;
        let before = motion.weights(interrupted, duration);
        motion.retarget("Connect".into(), interrupted, duration);
        assert_eq!(motion.weights(interrupted, duration), before);
        let end = interrupted + duration;
        assert!(motion.finished(end, duration));
        assert_eq!(
            motion.weights(end, duration),
            vec![(SharedString::from("Connect"), 1.)]
        );
        assert_eq!(
            motion.weights(interrupted, Duration::ZERO),
            motion.weights(end, duration)
        );
    }
}
