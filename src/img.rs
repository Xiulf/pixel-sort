use super::*;
use image::{DynamicImage, GenericImageView, Rgba};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;

fn calc_steps(opts: &Opts) -> usize {
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
        .template("{spinner} {msg}: {elapsed} {prefix}")
        .tick_chars(r"⣷⣯⣟⡿⢿⣻⣽⣾");

    let bar_style = ProgressStyle::default_bar()
        .template("{msg} {bar} {pos:>5}/{len}")
        .progress_chars("##-");

    let steps = calc_steps(&opts);
    let mut step = 1;

    let bars = MultiProgress::new();
    let input = input.as_ref().to_path_buf();
    let output = output.as_ref().to_path_buf();
    let pbo = bars.add(ProgressBar::new_spinner());

    pbo.set_style(spinner_style.clone());
    pbo.set_message(input.display().to_string());
    pbo.set_prefix(format!("[{}/{}]", step, steps));
    pbo.enable_steady_tick(100);

    let pb = bars.add(ProgressBar::new_spinner());

    pb.set_style(spinner_style.clone());
    pb.set_message("Reading");
    pb.enable_steady_tick(100);

    let thread = std::thread::spawn(move || {
        let mut image = image::open(input).unwrap();
        let mut resize = opts.resize;

        if opts.vertical {
            step += 1;
            pb.reset_elapsed();
            pb.set_message("Rotating");
            pbo.set_prefix(format!("[{}/{}]", step, steps));
            image = image.rotate90();
        }

        if let Some(scale) = opts.internal_scale {
            step += 1;
            pb.reset_elapsed();
            pb.set_message("Resizing");
            pbo.set_prefix(format!("[{}/{}]", step, steps));

            let (w, h) = image.dimensions();
            resize = resize.or(Some(Scale::Pixels(w, h)));
            let (w, h) = scale.calc(w, h);

            if opts.vertical {
                image = image.resize(h, w, image::imageops::FilterType::CatmullRom);
            } else {
                image = image.resize(w, h, image::imageops::FilterType::CatmullRom);
            }
        }

        step += 1;
        pb.reset_elapsed();
        pb.set_length(0);
        pb.set_style(bar_style);
        pb.set_message("Sorting");
        pbo.set_prefix(format!("[{}/{}]", step, steps));
        pb.disable_steady_tick();

        let mut res = sort_image(&pb, image, &opts);

        pb.set_style(spinner_style);
        pb.enable_steady_tick(100);

        if let Some(scale) = resize {
            step += 1;
            pb.reset_elapsed();
            pb.set_message("Resizing");
            pbo.set_prefix(format!("[{}/{}]", step, steps));

            let (w, h) = res.dimensions();
            let (w, h) = scale.calc(w, h);

            if opts.vertical {
                res = res.resize(h, w, image::imageops::FilterType::CatmullRom);
            } else {
                res = res.resize(w, h, image::imageops::FilterType::CatmullRom);
            }
        }

        if opts.vertical {
            step += 1;
            pb.reset_elapsed();
            pb.set_message("Rotating");
            pbo.set_prefix(format!("[{}/{}]", step, steps));
            res.rotate270();
        }

        step += 1;
        pb.reset_elapsed();
        pb.set_message("Saving");
        pbo.set_prefix(format!("[{}/{}]", step, steps));
        res.save(output).unwrap();

        pb.finish();
        pbo.finish();
    });

    bars.join_and_clear().unwrap();
    thread.join().unwrap();
}

pub fn sort_image(pb: &ProgressBar, image: DynamicImage, opts: &Opts) -> DynamicImage {
    let mut rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    match opts.sort_type {
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
