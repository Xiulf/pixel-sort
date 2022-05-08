use super::*;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;

fn calc_steps(opts: &Opts) -> u64 {
    let mut steps = 3;

    if opts.vertical {
        steps += 2;
    }

    if opts.internal_scale.is_some() {
        steps += 2;
    }

    if opts.internal_scale.is_none() && opts.resize.is_some() {
        steps += 1;
    }

    steps
}

pub fn process_image(input: impl AsRef<Path>, output: impl AsRef<Path>, opts: Opts) {
    let spinner_style = ProgressStyle::default_spinner()
        .template("{spinner} {msg}: {elapsed} [{pos}/{len}]")
        .tick_chars(r"-\|/ ");

    let dots_style = ProgressStyle::default_spinner()
        .template("  {msg}{spinner}")
        .tick_strings(&["   ", ".  ", ".. ", "...", "   "]);

    let bar_style = ProgressStyle::default_bar()
        .template("  {msg} [{bar}] {pos:>5}/{len}")
        .progress_chars(r"=> ");

    let steps = calc_steps(&opts);
    let bars = MultiProgress::new();
    let input = input.as_ref().to_path_buf();
    let output = output.as_ref().to_path_buf();
    let pbo = bars.add(ProgressBar::new_spinner());
    let pb = bars.add(ProgressBar::new_spinner());

    pbo.set_style(spinner_style);
    pbo.set_length(steps);
    pbo.set_position(1);
    pbo.set_message(input.display().to_string());
    pbo.enable_steady_tick(100);

    pb.set_style(dots_style.clone());
    pb.enable_steady_tick(250);

    let thread = std::thread::spawn(move || {
        pb.set_message("Reading");

        let mut image = image::open(input).unwrap();
        let mut resize = opts.resize;
        let (iw, ih) = image.dimensions();

        if opts.vertical {
            pbo.inc(1);
            pb.set_message("Rotating");
            image = image.rotate90();
        }

        if let Some(scale) = opts.internal_scale {
            pbo.inc(1);
            resize = resize.or(Some(Scale::Pixels(iw, ih)));

            let (sw, sh) = scale.calc(iw, ih);

            if sw != iw || sh != ih {
                pb.set_message("Resizing");

                if opts.vertical {
                    image = image.resize_exact(sh, sw, image::imageops::FilterType::Triangle);
                } else {
                    image = image.resize_exact(sw, sh, image::imageops::FilterType::Triangle);
                }
            }
        } else if let Some(scale) = opts.resize {
            pbo.inc(1);
            resize = None;

            let (sw, sh) = scale.calc(iw, ih);

            if sw != iw || sh != ih {
                pb.set_message("Resizing");

                if opts.vertical {
                    image = image.resize_exact(sh, sw, image::imageops::FilterType::Triangle);
                } else {
                    image = image.resize_exact(sw, sh, image::imageops::FilterType::Triangle);
                }
            }
        }

        pbo.inc(1);
        pb.set_length(0);
        pb.set_style(bar_style);
        pb.set_message("Sorting");

        let mut res = sort_image(&pb, image, &opts);

        pb.set_style(dots_style);

        if let Some(scale) = resize {
            pbo.inc(1);

            let (nw, nh) = res.dimensions();
            let (sw, sh) = scale.calc(iw, ih);

            if sw != nw || sh != nh {
                pb.set_message("Resizing");

                if opts.vertical {
                    res = res.resize_exact(sh, sw, image::imageops::FilterType::Lanczos3);
                } else {
                    res = res.resize_exact(sw, sh, image::imageops::FilterType::Lanczos3);
                }
            }
        }

        if opts.vertical {
            pbo.inc(1);
            pb.set_message("Rotating");
            res = res.rotate270();
        }

        pbo.inc(1);
        pb.set_message("Saving");
        res.save(output).unwrap();

        pb.finish();
        pbo.finish();
    });

    bars.join_and_clear().unwrap();

    if let Err(panic) = thread.join() {
        std::panic::resume_unwind(panic);
    }
}

pub fn sort_image(pb: &ProgressBar, image: DynamicImage, opts: &Opts) -> DynamicImage {
    let mut rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    match opts.sort_type {
        SortType::Spiral => {
            pb.set_length(u64::from(height / 2));
            pb.tick();

            let rgba_c = rgba.clone();

            for i in 0..height / 2 {
                let top = ((i + 1)..width - i).map(|x| (x, i));
                let right = ((i + 1)..height - i).map(|y| (width - i - 1, y));
                let bottom = (i..width - i - 1).map(|x| (x, height - i - 1)).rev();
                let left = (i..height - i - 1).map(|y| (i, y)).rev();
                let idxs = top
                    .chain(right)
                    .chain(bottom)
                    .chain(left)
                    .collect::<Vec<_>>();

                let mut pixels = idxs
                    .iter()
                    .map(|(x, y)| rgba_c.get_pixel(*x, *y))
                    .collect::<Vec<_>>();

                sort_pixels(opts, &mut pixels[..], opts.sort_fn);

                for ((x, y), px) in idxs.into_iter().zip(pixels) {
                    rgba.put_pixel(x, y, *px);
                }

                pb.inc(1);
            }
        }
        SortType::Circle { cx, cy } => {
            let dist = |x: u32, y: u32| {
                ((x as f64 - cx as f64).powi(2) + (y as f64 - cy as f64).powi(2)).sqrt()
            };

            let n = dist(0, 0)
                .max(dist(0, height))
                .max(dist(width, 0))
                .max(dist(width, height))
                .ceil() as u32;

            pb.set_length(n as u64);
            pb.tick();

            let rgba_c = rgba.clone();
            let mut written = vec![vec![false; height as usize]; width as usize];

            for i in 0..n {
                let idxs = circle_points(&rgba_c, cx, cy, i, opts.angle);
                let mut pixels = idxs
                    .iter()
                    .map(|(x, y)| rgba_c.get_pixel(*x, *y))
                    .collect::<Vec<_>>();

                sort_pixels(opts, &mut pixels[..], opts.sort_fn);

                for ((x, y), px) in idxs.into_iter().zip(pixels) {
                    rgba.put_pixel(x, y, *px);
                    written[x as usize][y as usize] = true;
                }

                pb.inc(1);
            }

            let rgba_c = rgba.clone();
            let color_of_neighbours = |x: i32, y: i32| {
                let neighbours = [
                    (x - 1, y - 1),
                    (x, y - 1),
                    (x + 1, y - 1),
                    (x - 1, y),
                    (x + 1, y),
                    (x - 1, y + 1),
                    (x, y + 1),
                    (x + 1, y + 1),
                ]
                .iter()
                .filter(|(x, _)| (0..written.len() as i32).contains(x))
                .filter(|(x, y)| (0..written[*x as usize].len() as i32).contains(y))
                .filter(|(x, y)| written[*x as usize][*y as usize])
                .map(|(x, y)| rgba_c.get_pixel(*x as u32, *y as u32))
                .collect::<Vec<_>>();

                let mut avg = Rgba([0.0, 0.0, 0.0, 0.0]);
                let len = neighbours.len() as f64;

                for pixel in neighbours {
                    avg.0[0] += pixel.0[0] as f64;
                    avg.0[1] += pixel.0[1] as f64;
                    avg.0[2] += pixel.0[2] as f64;
                    avg.0[3] += pixel.0[3] as f64;
                }

                avg.0[0] /= len;
                avg.0[1] /= len;
                avg.0[2] /= len;
                avg.0[3] /= len;

                Rgba([
                    avg.0[0] as u8,
                    avg.0[1] as u8,
                    avg.0[2] as u8,
                    avg.0[3] as u8,
                ])
            };

            for (x, col) in written.iter().enumerate() {
                for (y, val) in col.iter().enumerate() {
                    if !*val {
                        let col = color_of_neighbours(x as i32, y as i32);

                        rgba.put_pixel(x as u32, y as u32, col);
                    }
                }
            }
        }
        SortType::Sine { amp, lam, offset } => {
            let (c_x, c_y, diag) = (
                (width as f64 * 0.5).floor(),
                (height as f64 * 0.5).floor(),
                (width as f64).hypot(height as f64).floor() as u32,
            );

            pb.set_length(u64::from(diag * 3));
            pb.tick();

            let ang = opts.angle.to_radians();
            let (sin, cos) = (ang.sin(), ang.cos());
            let rgba_c = rgba.clone();

            for y in 0..(diag * 3) {
                let idxs = (0..diag)
                    .map(|x| x as f64)
                    .map(|x| (x, y as f64 / 3.0 + (x / lam + offset).sin() * amp))
                    .map(|(x, y)| (x - diag as f64 / 2.0, y - diag as f64 / 2.0))
                    .map(|(x, y)| (x * cos - y * sin, y * cos + x * sin))
                    .map(|(x, y)| (x + c_x, y + c_y))
                    .filter_map(|(x, y)| {
                        if x >= 0.0 && x < width as f64 && y > -0.0 && y < height as f64 {
                            Some((x.floor() as u32, y.floor() as u32))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                let mut pixels = idxs
                    .iter()
                    .map(|(x, y)| rgba_c.get_pixel(*x, *y))
                    .collect::<Vec<_>>();

                sort_pixels(opts, &mut pixels[..], opts.sort_fn);

                for ((x, y), px) in idxs.into_iter().zip(pixels) {
                    rgba.put_pixel(x, y, *px);
                }

                pb.inc(1);
            }
        }
        SortType::Linear if opts.angle != 0.0 => {
            let tan = opts.angle.to_radians().tan();
            let extra_height = (tan * width as f64).floor() as i64;
            let range = if extra_height > 0 {
                -extra_height..i64::from(height)
            } else {
                0..(i64::from(height) - extra_height)
            };

            pb.set_length((range.end - range.start) as u64);
            pb.tick();

            let rgba_c = rgba.clone();

            for y in range {
                let idxs = (0..width)
                    .map(|xv| (xv, (xv as f64 * tan + y as f64) as u32))
                    .filter(|(_, y)| *y > 0 && *y < height)
                    .collect::<Vec<_>>();

                let mut pixels = idxs
                    .iter()
                    .map(|(x, y)| rgba_c.get_pixel(*x, *y))
                    .collect::<Vec<_>>();

                sort_pixels(opts, &mut pixels[..], opts.sort_fn);

                for ((x, y), px) in idxs.iter().zip(pixels.iter()) {
                    rgba.put_pixel(*x, *y, **px);
                }

                pb.inc(1);
            }
        }
        SortType::Linear => {
            pb.set_length(height as u64);
            pb.tick();

            for (y, row) in rgba
                .clone()
                .pixels()
                .collect::<Vec<_>>()
                .chunks_mut(width as usize)
                .enumerate()
            {
                sort_pixels(opts, &mut row[..], opts.sort_fn);

                for (x, px) in row.iter().enumerate() {
                    rgba.put_pixel(x as u32, y as u32, **px);
                }

                pb.inc(1);
            }
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

pub fn sort_pixels(opts: &Opts, pixels: &mut [&Rgba<u8>], sort_fn: impl Fn(&[u8]) -> u8) {
    let mut reverse = opts.reverse;
    let mut ctr = 0;
    let interval_fn = match &opts.interval {
        IntervalType::Random => interval_random,
        IntervalType::Threshold => interval_threshold,
    };

    let interval_fn_reverse = match &opts.interval {
        IntervalType::Random => interval_none,
        IntervalType::Threshold => interval_threshold_reverse,
    };

    while ctr < pixels.len() {
        let numel = interval_fn(opts, pixels, ctr).min(pixels.len() - ctr);

        pixels[ctr..ctr + numel].sort_unstable_by(|l, r| {
            if reverse {
                sort_fn(&r.0).cmp(&sort_fn(&l.0))
            } else {
                sort_fn(&l.0).cmp(&sort_fn(&r.0))
            }
        });

        ctr += numel;
        ctr += interval_fn_reverse(opts, pixels, ctr);

        if opts.split && ctr >= pixels.len() / 2 {
            reverse = !reverse;
        }
    }
}

fn circle_points(img: &RgbaImage, cx: u32, cy: u32, r: u32, angle: f64) -> Vec<(u32, u32)> {
    let xr = 0..img.width();
    let yr = 0..img.height();
    let mut circle = Vec::new();
    let mut point = |x: i32, y: i32| {
        if xr.contains(&(x as u32)) && yr.contains(&(y as u32)) {
            circle.push((x as u32, y as u32));
        }
    };

    let mut circle_points = |cx: i32, cy: i32, x: i32, y: i32| {
        if x == 0 {
            point(cx, cy + y);
            point(cx, cy - y);
            point(cx + y, cy);
            point(cx - y, cy);
        } else if x == y {
            point(cx + x, cy + y);
            point(cx - x, cy + y);
            point(cx + x, cy - y);
            point(cx - x, cy - y);
        } else if x < y {
            point(cx + x, cy + y);
            point(cx - x, cy + y);
            point(cx + x, cy - y);
            point(cx - x, cy - y);
            point(cx + y, cy + x);
            point(cx - y, cy + x);
            point(cx + y, cy - x);
            point(cx - y, cy - x);
        }
    };

    let mut x = 0;
    let mut y = r as i32;
    let mut p = (5 - r as i32 * 4) / 4;

    circle_points(cx as i32, cy as i32, x, y);

    while x < y {
        x += 1;
        p += if p < 0 {
            2 * x + 1
        } else {
            y -= 1;
            2 * (x - y) + 1
        };

        circle_points(cx as i32, cy as i32, x, y);
    }

    circle.sort_by(|a, b| {
        let a = ((a.1 as f64 - cy as f64)
            .atan2(a.0 as f64 - cx as f64)
            .to_degrees()
            - 270.0
            - angle)
            % 360.0;

        let b = ((b.1 as f64 - cy as f64)
            .atan2(b.0 as f64 - cx as f64)
            .to_degrees()
            - 270.0
            - angle)
            % 360.0;

        if a < b {
            std::cmp::Ordering::Less
        } else if a > b {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });

    circle.dedup();
    circle
}

fn interval_none(_: &Opts, _: &[&Rgba<u8>], _: usize) -> usize {
    0
}

fn interval_random(opts: &Opts, pixels: &[&Rgba<u8>], ctr: usize) -> usize {
    use rand::{Rng, SeedableRng};
    use std::sync::Mutex;

    lazy_static::lazy_static! {
        static ref RNG: Mutex<rand::rngs::StdRng> = Mutex::new(rand::rngs::StdRng::seed_from_u64(0));
    }

    if opts.split {
        1.max(usize::min(
            pixels.len() / 2 - ctr,
            RNG.lock().unwrap().gen_range(opts.min, opts.max),
        ))
    } else {
        usize::min(
            pixels.len() - ctr,
            RNG.lock().unwrap().gen_range(opts.min, opts.max),
        )
    }
}

fn interval_threshold(opts: &Opts, pixels: &[&Rgba<u8>], ctr: usize) -> usize {
    let count = pixels[ctr..]
        .iter()
        .take_while(|p| {
            let l = (opts.sort_fn)(&p.0) as usize;

            (l >= opts.min && l <= opts.max) != opts.invert && mask_fn(opts, *p)
        })
        .count();

    if opts.split {
        1.max(count.min(pixels.len() / 2))
    } else {
        count
    }
}

fn interval_threshold_reverse(opts: &Opts, pixels: &[&Rgba<u8>], ctr: usize) -> usize {
    let count = pixels[ctr..]
        .iter()
        .take_while(|p| {
            let l = (opts.sort_fn)(&p.0) as usize;

            (l < opts.min || l > opts.max) != opts.invert || !mask_fn(opts, *p)
        })
        .count();

    if opts.split {
        1.max(count.min(pixels.len() / 2))
    } else {
        count
    }
}

fn mask_fn(opts: &Opts, p: &Rgba<u8>) -> bool {
    !(opts.mask_alpha && p[3] == 0)
}
