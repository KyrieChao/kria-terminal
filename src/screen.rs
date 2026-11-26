use crate::window::{draw_glyph, is_wide, FontSet, BG_COLOR, FG_COLOR};

#[derive(Clone)]
pub struct Cell {
    pub ch: char,
    pub fg: u32,
    pub bg: u32,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: FG_COLOR,
            bg: BG_COLOR,
        }
    }
}

/// 用单色填满一块矩形，带越界裁剪
fn fill_rect(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    px: isize,
    py: isize,
    w: usize,
    h: usize,
    color: u32,
) {
    for gy in 0..h {
        let by = py + gy as isize;
        if by < 0 || by >= height as isize {
            continue;
        }
        let row = by as usize * width;
        for gx in 0..w {
            let bx = px + gx as isize;
            if bx < 0 || bx >= width as isize {
                continue;
            }
            buffer[row + bx as usize] = color;
        }
    }
}

pub struct Screen {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
    /// 单格像素宽高，由字体度量决定
    pub cell_w: usize,
    pub cell_h: usize,
    pub font_size: f32,
    /// 光标位置（列, 行）
    pub cx: usize,
    pub cy: usize,
}

impl Screen {
    pub fn new(cols: usize, rows: usize, fonts: &FontSet, font_size: f32) -> Self {
        // 等宽字体：用 'M' 的 advance 作为单元宽度，行高取字号的 1.2 倍
        let (metrics, _) = fonts.rasterize('M', font_size);
        let cell_w = metrics.advance_width.ceil() as usize;
        let cell_h = (font_size * 1.2).ceil() as usize;

        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            cell_w,
            cell_h,
            font_size,
            cx: 0,
            cy: 0,
        }
    }

    /// 测试用：不加载字体，单元尺寸固定为 1 像素
    #[cfg(test)]
    pub fn new_test(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            cell_w: 1,
            cell_h: 1,
            font_size: 1.0,
            cx: 0,
            cy: 0,
        }
    }

    pub fn put(&mut self, x: usize, y: usize, ch: char) {
        if x < self.cols && y < self.rows {
            self.cells[y * self.cols + x].ch = ch;
        }
    }

    /// 在光标处写字并前进，到行尾自动换行；宽字符占两格
    pub fn write_char(&mut self, ch: char) {
        let w = if is_wide(ch) { 2 } else { 1 };
        // 剩余空间放不下宽字符时先换行，避免把字劈成两半
        if self.cx + w > self.cols {
            self.cx = 0;
            self.line_feed();
        }
        self.put(self.cx, self.cy, ch);
        self.cx += w;
        if self.cx >= self.cols {
            self.cx = 0;
            self.line_feed();
        }
    }

    /// 换行：光标下移一行，如果到底就整体上滚
    pub fn line_feed(&mut self) {
        if self.cy + 1 < self.rows {
            self.cy += 1;
        } else {
            self.scroll_up();
        }
    }

    pub fn carriage_return(&mut self) {
        self.cx = 0;
    }

    pub fn backspace(&mut self) {
        self.cx = self.cx.saturating_sub(1);
    }

    pub fn home(&mut self) {
        self.cx = 0;
        self.cy = 0;
    }

    /// 光标定位（参数已经转成 0 基），越界则夹到边界
    pub fn move_to(&mut self, x: usize, y: usize) {
        self.cx = x.min(self.cols.saturating_sub(1));
        self.cy = y.min(self.rows.saturating_sub(1));
    }

    pub fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| *c = Cell::default());
        self.home();
    }

    /// 整体上滚一行：丢掉第一行，末尾补一行空白
    fn scroll_up(&mut self) {
        self.cells.drain(0..self.cols);
        self.cells.resize(self.cols * self.rows, Cell::default());
    }


    pub fn render(&self, buffer: &mut [u32], width: usize, height: usize, fonts: &FontSet) {
        // 第一遍：铺全部背景。
        // 必须先铺完再画字，否则中文字形跨到右边一格时，会被那一格的背景擦掉。
        for y in 0..self.rows {
            for x in 0..self.cols {
                let cell = &self.cells[y * self.cols + x];
                fill_rect(
                    buffer,
                    width,
                    height,
                    (x * self.cell_w) as isize,
                    (y * self.cell_h) as isize,
                    self.cell_w,
                    self.cell_h,
                    cell.bg,
                );
            }
        }

        // 第二遍：画字形
        for y in 0..self.rows {
            for x in 0..self.cols {
                let cell = &self.cells[y * self.cols + x];
                if cell.ch == ' ' {
                    continue;
                }

                let (metrics, bitmap) = fonts.rasterize(cell.ch, self.font_size);
                let px = (x * self.cell_w) as isize;
                let py = (y * self.cell_h) as isize;

                draw_glyph(
                    buffer,
                    width,
                    height,
                    &bitmap,
                    metrics.width,
                    metrics.height,
                    px + metrics.xmin as isize,
                    // 字底对齐格子底部（粗略：留 1/5 行高给下伸部分）
                    py + (self.cell_h - self.cell_h / 5) as isize
                        - metrics.ymin as isize
                        - metrics.height as isize,
                    cell.fg,
                );
            }
        }
    }
}
