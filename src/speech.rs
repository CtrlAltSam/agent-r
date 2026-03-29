use std::collections::VecDeque;
use std::time::Duration;

use font8x8::{BASIC_FONTS, UnicodeFonts};
use image::{imageops, Rgba, RgbaImage};

//DISCLAIMER: Most of the rasterization is AI generated!!!

#[derive(Clone, Copy, Debug)]
pub struct BubbleLayout {
	pub canvas_width: u32,
	pub canvas_height: u32,
	pub image_x: u32,
	pub image_y: u32,
	pub bubble_x: u32,
	pub bubble_y: u32,
	pub bubble_width: u32,
	pub bubble_height: u32,
	pub tail_width: u32,
}

pub struct SpeechBubble {
	current_text: Option<String>,
	queued_messages: VecDeque<String>,
	visible_chars: usize,
	reveal_progress: f32,
	normal_chars_per_second: f32,
	fast_chars_per_second: f32,
	speed_up_active: bool,
	indicator_phase: f32,
}

impl SpeechBubble {
	pub fn new(initial_text: impl Into<String>) -> Self {
		let mut bubble = Self {
			current_text: None,
			queued_messages: VecDeque::new(),
			visible_chars: 0,
			reveal_progress: 0.0,
			normal_chars_per_second: 24.0,
			fast_chars_per_second: 96.0,
			speed_up_active: false,
			indicator_phase: 0.0,
		};
		bubble.push_message(initial_text);
		bubble
	}

	pub fn push_message(&mut self, message: impl Into<String>) {
		let message = message.into();
		if message.trim().is_empty() {
			return;
		}

		if self.current_text.is_none() {
			self.start_message(message);
		} else {
			self.queued_messages.push_back(message);
		}
	}

	pub fn clear_messages(&mut self) {
		self.current_text = None;
		self.queued_messages.clear();
		self.visible_chars = 0;
		self.reveal_progress = 0.0;
		self.speed_up_active = false;
		self.indicator_phase = 0.0;
	}

	pub fn is_visible(&self) -> bool {
		self.current_text.is_some()
	}

	pub fn awaiting_advance(&self) -> bool {
		self.is_visible() && self.is_finished()
	}

	pub fn advance_message(&mut self) -> bool {
		if !self.awaiting_advance() {
			return false;
		}

		if let Some(next) = self.queued_messages.pop_front() {
			self.start_message(next);
			return true;
		}

		self.clear_messages();
		true
	}

	pub fn is_finished(&self) -> bool {
		if self.current_text.is_none() {
			return false;
		}
		self.visible_chars >= self.total_chars()
	}

	pub fn set_speed_up(&mut self, enabled: bool) {
		self.speed_up_active = enabled;
	}

	pub fn boost_once(&mut self) {
		if !self.is_visible() {
			return;
		}
		self.speed_up_active = true;
		if !self.is_finished() {
			self.visible_chars = (self.visible_chars + 1).min(self.total_chars());
			self.reveal_progress = self.visible_chars as f32;
		}
	}

	pub fn update(&mut self, delta: Duration) -> bool {
		if !self.is_visible() {
			return false;
		}

		self.indicator_phase += delta.as_secs_f32();

		if self.is_finished() {
			return false;
		}

		let chars_per_second = if self.speed_up_active {
			self.fast_chars_per_second
		} else {
			self.normal_chars_per_second
		};

		self.reveal_progress += delta.as_secs_f32() * chars_per_second;
		let new_visible = self.reveal_progress.floor() as usize;
		let clamped = new_visible.min(self.total_chars());
		let changed = clamped != self.visible_chars;
		self.visible_chars = clamped;
		changed
	}

	pub fn layout(&self, image_width: u32, image_height: u32) -> BubbleLayout {
		let margin = ((image_width.min(image_height) as f32) * 0.04).round().max(8.0) as u32;
		let bubble_width = ((image_width as f32) * 0.92).round().max(140.0) as u32;
		let font_scale = font_scale_for_image(image_height);
		let char_width = 8 * font_scale;
		let line_height = 9 * font_scale;
		let padding = ((image_height as f32) * 0.05).round().clamp(8.0, 28.0) as u32;
		let text_area_width = bubble_width
			.saturating_sub(padding.saturating_mul(2))
			.max(char_width);
		let max_cols = (text_area_width / char_width).max(1) as usize;
		let required_lines = wrap_text(self.current_text(), max_cols).len().max(1) as u32;

		let min_bubble_height = ((image_height as f32) * 0.30).round().clamp(72.0, 220.0) as u32;
		let max_bubble_height = ((image_height as f32) * 0.72).round().clamp(120.0, 540.0) as u32;
		let required_bubble_height = required_lines
			.saturating_mul(line_height)
			.saturating_add(padding.saturating_mul(2))
			.saturating_add(4);
		let bubble_height = required_bubble_height
			.max(min_bubble_height)
			.min(max_bubble_height);
		let tail_width = ((bubble_width as f32) * 0.11).round().clamp(18.0, 40.0) as u32;
		let tail_height = ((bubble_height as f32) * 0.18).round().clamp(12.0, 36.0) as u32;

		let canvas_width = image_width.max(bubble_width);
		let bubble_x = (canvas_width.saturating_sub(bubble_width)) / 2;
		let bubble_y = margin;
		let image_x = (canvas_width.saturating_sub(image_width)) / 2;
		let image_y = bubble_y + bubble_height + tail_height + (margin / 2);
		let canvas_height = image_y + image_height;

		BubbleLayout {
			canvas_width,
			canvas_height,
			image_x,
			image_y,
			bubble_x,
			bubble_y,
			bubble_width,
			bubble_height,
			tail_width,
		}
	}

	pub fn canvas_size(&self, image_width: u32, image_height: u32) -> (u32, u32) {
		if !self.is_visible() {
			return (image_width, image_height);
		}
		let layout = self.layout(image_width, image_height);
		(layout.canvas_width, layout.canvas_height)
	}

	pub fn image_offset(&self, image_width: u32, image_height: u32) -> (u32, u32) {
		if !self.is_visible() {
			return (0, 0);
		}
		let layout = self.layout(image_width, image_height);
		(layout.image_x, layout.image_y)
	}

	pub fn hit_test(&self, x: f32, y: f32, image_width: u32, image_height: u32) -> bool {
		if !self.is_visible() {
			return false;
		}

		let layout = self.layout(image_width, image_height);
		let bx = layout.bubble_x as f32;
		let by = layout.bubble_y as f32;
		let bw = layout.bubble_width as f32;
		let bh = layout.bubble_height as f32;

		let in_rect = x >= bx && x <= bx + bw && y >= by && y <= by + bh;
		if in_rect {
			return true;
		}

		let center_x = (layout.bubble_x + (layout.bubble_width / 2)) as f32;
		let base_y = (layout.bubble_y + layout.bubble_height) as f32;
		let tip_x = center_x;
		let tip_y = (layout.image_y.saturating_sub(1)) as f32;
		point_in_triangle(
			x,
			y,
			(center_x - (layout.tail_width as f32 / 2.0), base_y),
			(center_x + (layout.tail_width as f32 / 2.0), base_y),
			(tip_x, tip_y),
		)
	}

	pub fn compose(&self, image: &RgbaImage) -> Option<RgbaImage> {
		if !self.is_visible() {
			return None;
		}

		let layout = self.layout(image.width(), image.height());
		let mut canvas = RgbaImage::from_pixel(
			layout.canvas_width,
			layout.canvas_height,
			Rgba([0, 0, 0, 0]),
		);

		imageops::overlay(
			&mut canvas,
			image,
			layout.image_x as i64,
			layout.image_y as i64,
		);

		draw_bubble(&mut canvas, layout);
		self.draw_text(&mut canvas, layout);

		Some(canvas)
	}

	fn draw_text(&self, target: &mut RgbaImage, layout: BubbleLayout) {
		let visible = take_chars(self.current_text(), self.visible_chars);
		if visible.is_empty() {
			return;
		}

		let image_height = layout.canvas_height.saturating_sub(layout.image_y);
		let font_scale = font_scale_for_image(image_height);
		let char_width = 8 * font_scale;
		let line_height = 9 * font_scale;
		let padding = ((image_height as f32) * 0.05).round().clamp(8.0, 28.0) as u32;

		let text_area_width = layout
			.bubble_width
			.saturating_sub(padding.saturating_mul(2))
			.max(8);
		let text_area_height = layout
			.bubble_height
			.saturating_sub(padding.saturating_mul(2))
			.max(line_height);

		let max_cols = (text_area_width / char_width).max(1) as usize;
		let max_lines = (text_area_height / line_height).max(1) as usize;
		let mut lines = wrap_text(&visible, max_cols);
		if lines.len() > max_lines {
			lines.truncate(max_lines);
			if let Some(last) = lines.last_mut() {
				if last.len() >= 3 {
					last.truncate(last.len().saturating_sub(3));
				}
				last.push_str("...");
			}
		}

		let start_x = layout.bubble_x + padding;
		let start_y = layout.bubble_y + padding;
		let text_color = Rgba([20, 20, 20, 255]);

		for (line_idx, line) in lines.iter().enumerate() {
			let y = start_y + line_idx as u32 * line_height;
			for (char_idx, ch) in line.chars().enumerate() {
				let x = start_x + char_idx as u32 * char_width;
				draw_char(target, x, y, ch, font_scale, text_color);
			}
		}

		if self.awaiting_advance() {
			let (indicator_x, indicator_y) = indicator_anchor(
				layout,
				&lines,
				start_x,
				start_y,
				char_width,
				line_height,
			);
			let float_offset = (self.indicator_phase * 6.0).sin() * 3.0;
			let indicator_size = (font_scale * 4).max(4) as i32;
			let iy = (indicator_y as f32 + float_offset).round() as i32;
			draw_inverted_triangle_indicator(
				target,
				indicator_x as i32,
				iy,
				indicator_size,
				Rgba([20, 20, 20, 255]),
			);
		}
	}

	fn total_chars(&self) -> usize {
		self.current_text().chars().count()
	}

	fn current_text(&self) -> &str {
		self.current_text.as_deref().unwrap_or("")
	}

	fn start_message(&mut self, text: String) {
		self.current_text = Some(text);
		self.visible_chars = 0;
		self.reveal_progress = 0.0;
		self.speed_up_active = false;
		self.indicator_phase = 0.0;
	}
}

fn indicator_anchor(
	layout: BubbleLayout,
	lines: &[String],
	start_x: u32,
	start_y: u32,
	char_width: u32,
	line_height: u32,
) -> (u32, u32) {
	if let Some(last_line) = lines.last() {
		let line_idx = (lines.len().saturating_sub(1)) as u32;
		let x = start_x
			.saturating_add((last_line.chars().count() as u32).saturating_mul(char_width))
			.saturating_add(char_width / 2);
		let y = start_y
			.saturating_add(line_idx.saturating_mul(line_height))
			.saturating_add(line_height / 2);

		let max_x = layout
			.bubble_x
			.saturating_add(layout.bubble_width)
			.saturating_sub(char_width);
		let clamped_x = x.min(max_x);
		return (clamped_x, y);
	}

	let x = layout
		.bubble_x
		.saturating_add(layout.bubble_width)
		.saturating_sub(char_width * 2);
	let y = layout
		.bubble_y
		.saturating_add(layout.bubble_height)
		.saturating_sub(line_height * 2);
	(x, y)
}

fn draw_bubble(target: &mut RgbaImage, layout: BubbleLayout) {
	let fill = Rgba([255, 255, 255, 230]);
	let border = Rgba([24, 24, 24, 255]);
	let radius = (layout.bubble_height / 7).clamp(10, 24) as i32;

	fill_rounded_rect(
		target,
		layout.bubble_x as i32,
		layout.bubble_y as i32,
		layout.bubble_width as i32,
		layout.bubble_height as i32,
		radius,
		fill,
	);
	stroke_rounded_rect(
		target,
		layout.bubble_x as i32,
		layout.bubble_y as i32,
		layout.bubble_width as i32,
		layout.bubble_height as i32,
		radius,
		border,
	);

	let center_x = (layout.bubble_x + (layout.bubble_width / 2)) as i32;
	let base_y = (layout.bubble_y + layout.bubble_height) as i32;
	let tip_x = center_x;
	let tip_y = layout.image_y.saturating_sub(1) as i32;
	let half_tail = (layout.tail_width / 2) as i32;

	fill_triangle(
		target,
		(center_x - half_tail, base_y),
		(center_x + half_tail, base_y),
		(tip_x, tip_y),
		fill,
	);
	stroke_triangle(
		target,
		(center_x - half_tail, base_y),
		(center_x + half_tail, base_y),
		(tip_x, tip_y),
		border,
	);
}

fn wrap_text(text: &str, max_cols: usize) -> Vec<String> {
	if max_cols == 0 {
		return vec![String::new()];
	}

	let mut lines = Vec::new();
	let mut line = String::new();

	for word in text.split_whitespace() {
		if line.is_empty() {
			if word.chars().count() <= max_cols {
				line.push_str(word);
			} else {
				split_long_word(word, max_cols, &mut lines, &mut line);
			}
			continue;
		}

		let proposed = line.chars().count() + 1 + word.chars().count();
		if proposed <= max_cols {
			line.push(' ');
			line.push_str(word);
		} else {
			lines.push(std::mem::take(&mut line));
			if word.chars().count() <= max_cols {
				line.push_str(word);
			} else {
				split_long_word(word, max_cols, &mut lines, &mut line);
			}
		}
	}

	if !line.is_empty() {
		lines.push(line);
	}

	if lines.is_empty() {
		vec![String::new()]
	} else {
		lines
	}
}

fn split_long_word(word: &str, max_cols: usize, lines: &mut Vec<String>, line: &mut String) {
	let chars: Vec<char> = word.chars().collect();
	let mut cursor = 0;
	while cursor < chars.len() {
		let end = (cursor + max_cols).min(chars.len());
		let chunk: String = chars[cursor..end].iter().collect();
		if line.is_empty() {
			line.push_str(&chunk);
		} else {
			lines.push(std::mem::take(line));
			line.push_str(&chunk);
		}
		cursor = end;
		if cursor < chars.len() {
			lines.push(std::mem::take(line));
		}
	}
}

fn font_scale_for_image(image_height: u32) -> u32 {
	if image_height >= 420 {
		3
	} else if image_height >= 200 {
		2
	} else {
		1
	}
}

fn take_chars(text: &str, max_chars: usize) -> String {
	text.chars().take(max_chars).collect()
}

fn draw_char(target: &mut RgbaImage, x: u32, y: u32, ch: char, scale: u32, color: Rgba<u8>) {
	let glyph = BASIC_FONTS.get(ch).or_else(|| BASIC_FONTS.get('?'));
	let Some(bitmap) = glyph else {
		return;
	};

	for (row, bits) in bitmap.iter().enumerate() {
		for col in 0..8 {
			if bits & (1 << col) != 0 {
				for sy in 0..scale {
					for sx in 0..scale {
						blend_pixel(
							target,
							x + (col as u32 * scale) + sx,
							y + (row as u32 * scale) + sy,
							color,
						);
					}
				}
			}
		}
	}
}

fn draw_inverted_triangle_indicator(
	target: &mut RgbaImage,
	center_x: i32,
	center_y: i32,
	size: i32,
	color: Rgba<u8>,
) {
	let half = size.max(2);
	let top_y = center_y - (half / 2);
	let tip_y = center_y + half;

	fill_triangle(
		target,
		(center_x - half, top_y),
		(center_x + half, top_y),
		(center_x, tip_y),
		color,
	);
}

fn fill_rounded_rect(
	target: &mut RgbaImage,
	x: i32,
	y: i32,
	width: i32,
	height: i32,
	radius: i32,
	color: Rgba<u8>,
) {
	let max_x = x + width;
	let max_y = y + height;
	for py in y..max_y {
		for px in x..max_x {
			if point_in_rounded_rect(px, py, x, y, width, height, radius) {
				blend_pixel(target, px as u32, py as u32, color);
			}
		}
	}
}

fn stroke_rounded_rect(
	target: &mut RgbaImage,
	x: i32,
	y: i32,
	width: i32,
	height: i32,
	radius: i32,
	color: Rgba<u8>,
) {
	let max_x = x + width;
	let max_y = y + height;
	for py in y..max_y {
		for px in x..max_x {
			let inside = point_in_rounded_rect(px, py, x, y, width, height, radius);
			if !inside {
				continue;
			}

			let neighbors = [
				point_in_rounded_rect(px + 1, py, x, y, width, height, radius),
				point_in_rounded_rect(px - 1, py, x, y, width, height, radius),
				point_in_rounded_rect(px, py + 1, x, y, width, height, radius),
				point_in_rounded_rect(px, py - 1, x, y, width, height, radius),
			];

			if neighbors.iter().any(|inside_neighbor| !inside_neighbor) {
				blend_pixel(target, px as u32, py as u32, color);
			}
		}
	}
}

fn point_in_rounded_rect(
	px: i32,
	py: i32,
	x: i32,
	y: i32,
	width: i32,
	height: i32,
	radius: i32,
) -> bool {
	let rx = radius;
	let ry = radius;
	let left = x;
	let right = x + width - 1;
	let top = y;
	let bottom = y + height - 1;

	if px >= left + rx && px <= right - rx {
		return py >= top && py <= bottom;
	}
	if py >= top + ry && py <= bottom - ry {
		return px >= left && px <= right;
	}

	let corners = [
		(left + rx, top + ry),
		(right - rx, top + ry),
		(left + rx, bottom - ry),
		(right - rx, bottom - ry),
	];

	for (cx, cy) in corners {
		let dx = px - cx;
		let dy = py - cy;
		if dx * dx + dy * dy <= radius * radius {
			return true;
		}
	}

	false
}

fn fill_triangle(
	target: &mut RgbaImage,
	a: (i32, i32),
	b: (i32, i32),
	c: (i32, i32),
	color: Rgba<u8>,
) {
	let min_x = a.0.min(b.0).min(c.0);
	let max_x = a.0.max(b.0).max(c.0);
	let min_y = a.1.min(b.1).min(c.1);
	let max_y = a.1.max(b.1).max(c.1);

	for py in min_y..=max_y {
		for px in min_x..=max_x {
			if point_in_triangle(
				px as f32,
				py as f32,
				(a.0 as f32, a.1 as f32),
				(b.0 as f32, b.1 as f32),
				(c.0 as f32, c.1 as f32),
			) {
				blend_pixel(target, px as u32, py as u32, color);
			}
		}
	}
}

fn stroke_triangle(
	target: &mut RgbaImage,
	a: (i32, i32),
	b: (i32, i32),
	c: (i32, i32),
	color: Rgba<u8>,
) {
	draw_line(target, a, b, color);
	draw_line(target, b, c, color);
	draw_line(target, c, a, color);
}

fn draw_line(target: &mut RgbaImage, start: (i32, i32), end: (i32, i32), color: Rgba<u8>) {
	let mut x0 = start.0;
	let mut y0 = start.1;
	let x1 = end.0;
	let y1 = end.1;

	let dx = (x1 - x0).abs();
	let sx = if x0 < x1 { 1 } else { -1 };
	let dy = -(y1 - y0).abs();
	let sy = if y0 < y1 { 1 } else { -1 };
	let mut err = dx + dy;

	loop {
		blend_pixel(target, x0 as u32, y0 as u32, color);
		if x0 == x1 && y0 == y1 {
			break;
		}
		let e2 = 2 * err;
		if e2 >= dy {
			err += dy;
			x0 += sx;
		}
		if e2 <= dx {
			err += dx;
			y0 += sy;
		}
	}
}

fn point_in_triangle(
	px: f32,
	py: f32,
	a: (f32, f32),
	b: (f32, f32),
	c: (f32, f32),
) -> bool {
	let area = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
		(p1.0 * (p2.1 - p3.1) + p2.0 * (p3.1 - p1.1) + p3.0 * (p1.1 - p2.1)).abs() / 2.0
	};

	let total = area(a, b, c);
	let a1 = area((px, py), b, c);
	let a2 = area(a, (px, py), c);
	let a3 = area(a, b, (px, py));
	(a1 + a2 + a3 - total).abs() <= 0.5
}

fn blend_pixel(target: &mut RgbaImage, x: u32, y: u32, src: Rgba<u8>) {
	if x >= target.width() || y >= target.height() {
		return;
	}

	let dst = target.get_pixel_mut(x, y);
	let src_a = src[3] as f32 / 255.0;
	let dst_a = dst[3] as f32 / 255.0;
	let out_a = src_a + dst_a * (1.0 - src_a);

	if out_a <= 0.0 {
		*dst = Rgba([0, 0, 0, 0]);
		return;
	}

	for channel in 0..3 {
		let src_c = src[channel] as f32 / 255.0;
		let dst_c = dst[channel] as f32 / 255.0;
		let out_c = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
		dst[channel] = (out_c * 255.0).round().clamp(0.0, 255.0) as u8;
	}

	dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}
