use libremarkable::framebuffer::cgmath::Point2;
use libremarkable::framebuffer::common::*;
use libremarkable::framebuffer::{FramebufferIO, FramebufferRefresh};
use libremarkable::framebuffer::refresh::PartialRefreshMode;
use libremarkable::appctx;
use libremarkable::framebuffer::core::Framebuffer;

use vte::{Parser, Perform};

use std::collections::HashMap;

const RES_WIDTH: u32 = 1404;
const RES_HEIGHT: u32 = 1872;

type Glyph = (fontdue::Metrics, Vec<u8>);

struct Terminal<'a> {
    fb: &'a mut Framebuffer<'a>,

    row: u32,
    col: u32,

    char_width: u32,
    line_height: u32,
    left_padding: u32,
    top_padding: u32,
    n_rows: u32,
    n_cols: u32,

    line_metrics: fontdue::LineMetrics,
    glyphs: HashMap<char, Glyph>,
}

impl<'a> Terminal<'a> {
    pub fn new(fb: &'a mut Framebuffer<'a>) -> Self {
        //let font = include_bytes!("../fonts/SourceCodePro-Medium.ttf") as &[u8];
        let font = include_bytes!("../fonts/CourierPrime-Bold.ttf") as &[u8];
        let font = fontdue::Font::from_bytes(font, fontdue::FontSettings::default()).unwrap();

        let font_size = 32.;

        let line_metrics = font.horizontal_line_metrics(font_size).unwrap();

        let line_height = line_metrics.new_line_size as u32;
        let char_width = font.metrics('a', font_size).advance_width as u32;

        let n_cols = 60;
        let n_rows = 50;
        let left_padding = RES_WIDTH.saturating_sub(n_cols * char_width) / 2;
        let top_padding = RES_HEIGHT.saturating_sub(n_rows * line_height) / 2;

        let mut glyphs = HashMap::new();
        for b in 32u8..=127 {
            let glyph = font.rasterize(b as char, font_size);
            glyphs.insert(b as char, glyph);
        }

        Self {
            fb,
            row: 0,
            col: 0,
            char_width,
            line_height,
            left_padding,
            top_padding,
            n_cols,
            n_rows,
            glyphs,
            line_metrics,
        }
    }

    fn cursor(&self) -> (i32, i32) {
        let x = self.col * self.char_width + self.left_padding;
        let y = self.row * self.line_height + self.top_padding;
        (x as i32, y as i32)
    }

    fn new_line(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= self.n_rows {
            self.row = 0;
        }
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            let (x, y) = self.cursor();
            clear(&mut self.fb, x as usize, y as usize, self.char_width as usize, self.line_height as usize);
        }
    }
}

impl<'a> Perform for Terminal<'a> {
    fn print(&mut self, c: char) {
        // DELETE -> BACKSPACE
        if c as u8 == 127 {
            self.backspace();
            return
        }

        let glyph = self.glyphs.get(&c).unwrap();

        let (x, y) = self.cursor();
        let y = y + self.line_metrics.descent as i32;
        draw_glyph(&mut self.fb, x as usize, y as usize, self.char_width as usize, self.line_height as usize, &glyph);

        self.col += 1;
        if self.col >= self.n_cols {
            self.new_line();
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            8 => {
                // BACKSPACE
                self.backspace();
            },
            10..=13 => {
                // CARRIAGE RETURN / LINE FEED
                self.new_line();
            },
            _ => {
                println!("execute {}?", byte);
            },
        }
    }
}

fn draw_glyph(fb: &mut Framebuffer<'_>, x: usize, y: usize, _w: usize, h: usize, glyph: &Glyph) {
    const THRESHOLD: i32 = 128; 

    let (metrics, bitmap) = glyph;

    let left_pad = metrics.xmin;
    let top_pad = h as i32 - metrics.height as i32 - metrics.ymin;

    use rand::Rng;
    let mut rng = rand::thread_rng();

    use noise::{NoiseFn, Perlin};

    let perlin = Perlin::new();
    let noise_z = rng.gen_range(0., 1024.);

    for i in 0..metrics.height {
        for j in 0..metrics.width {
            let val = perlin.get([i as f64 / 10., j as f64 / 10., noise_z]);
            /*
            if val > 0. {
                fb.write_pixel(Point2{ x: (x + j) as i32 + left_pad, y: (y + i) as i32 + top_pad }, color::BLACK);
            }*/

            let noise = (val * 120.) as i32;
            if bitmap[i * metrics.width + j] as i32 + noise > THRESHOLD {
                fb.write_pixel(Point2{ x: (x + j) as i32 + left_pad, y: (y + i) as i32 + top_pad }, color::BLACK);
            }
        }
    }

    let rect = mxcfb_rect {
        left: (x as i32 + left_pad) as u32,
        top: (y as i32 + top_pad) as u32,
        width: metrics.width as u32,
        height: metrics.height as u32,
    };

    fb.partial_refresh(
        &rect,
        PartialRefreshMode::Async,
        waveform_mode::WAVEFORM_MODE_DU,
        display_temp::TEMP_USE_REMARKABLE_DRAW,
        dither_mode::EPDC_FLAG_EXP1,
        DRAWING_QUANT_BIT,
        false,
    );
}

fn clear(fb: &mut Framebuffer<'_>, x: usize, y: usize, w: usize, h: usize) {
    let rect = mxcfb_rect {
        left: x as u32,
        top: y as u32,
        width: w as u32,
        height: h as u32,
    };
    for i in y..(y+h) {
        for j in x..(x+w) {
            fb.write_pixel(Point2{ x: j as i32, y: i as i32 }, color::WHITE);
        }
    }
    fb.partial_refresh(
        &rect,
        PartialRefreshMode::Async,
        waveform_mode::WAVEFORM_MODE_GC16,
        display_temp::TEMP_USE_REMARKABLE_DRAW,
        dither_mode::EPDC_FLAG_EXP1,
        DRAWING_QUANT_BIT,
        false,
    );
}
    
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};

fn main() {
    let mut app: appctx::ApplicationContext<'_> =
        appctx::ApplicationContext::new(|_, _| {}, |_, _| {}, |_, _| {});
    app.clear(true);

    let mut term = Terminal::new(app.get_framebuffer_ref()); 
    let mut statemachine = Parser::new();

    let listener = TcpListener::bind("0.0.0.0:5000").unwrap();
    let mut buf = [0u8; 16];
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        loop {
            match stream.read(&mut buf) {
                Ok(n) => {
                    for &byte in buf.iter().take(n) {
                        if byte == 27 {
                            break;
                        }
                        statemachine.advance(&mut term, byte);
                    }
                },
                Err(_) => break,
            }
        }
    }

    /*
    for byte in std::io::stdin().bytes() {
        let byte = byte.unwrap();
        if byte == 27 {
            break;
        }
        statemachine.advance(&mut term, byte);
    }
    */

    // Print an exit message
    term.new_line();
    term.new_line();
    for c in "exiting...".chars() {
        term.print(c);
    }
}

