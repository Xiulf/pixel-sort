use super::*;
use image::{DynamicImage, Rgba};
use indicatif::ProgressBar;

pub fn sort_image(pb: &ProgressBar, mut image: DynamicImage, opts: &Opts) -> DynamicImage {
    if opts.vertical {
        image = image.rotate90();
    }

    let mut rgba = image.to_rgba();
    let (width, height) = rgba.dimensions();

    match opts.sort_type {
        SortType::Sine { amp, lam, offset } => {
            let (c_x, c_y, diag) = (
                (width as f64 * 0.5).floor(),
                (height as f64 * 0.5).floor(),
                (width as f64).hypot(height as f64).floor() as u32,
            );

            pb.set_length(u64::from(height + diag) * 3);
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

                for ((x, y), px) in idxs.iter().zip(pixels.iter()) {
                    rgba.put_pixel(*x, *y, **px);
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

            pb.set_length(range.end as u64);
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

    pb.finish_with_message("Done");

    let res = DynamicImage::ImageRgba8(rgba);

    if opts.vertical {
        res.rotate270()
    } else {
        res
    }
}

pub fn sort_pixels(opts: &Opts, pixels: &mut [&Rgba<u8>], sort_fn: impl Fn(&[u8]) -> u8) {
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
        let numel = interval_fn(opts, pixels, ctr);

        pixels[ctr..ctr + numel].sort_unstable_by(|l, r| {
            if opts.reverse {
                sort_fn(&r.0).cmp(&sort_fn(&l.0))
            } else {
                sort_fn(&l.0).cmp(&sort_fn(&r.0))
            }
        });

        ctr += numel;
        ctr += interval_fn_reverse(opts, pixels, ctr);
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

    usize::min(
        pixels.len() - ctr,
        RNG.lock().unwrap().gen_range(opts.min, opts.max),
    )
}

fn interval_threshold(opts: &Opts, pixels: &[&Rgba<u8>], ctr: usize) -> usize {
    pixels[ctr..]
        .iter()
        .take_while(|p| {
            let l = pixel_luma(&p.0) as usize;

            (l >= opts.min && l <= opts.max) != opts.invert && mask_fn(opts, *p)
        })
        .count()
}

fn interval_threshold_reverse(opts: &Opts, pixels: &[&Rgba<u8>], ctr: usize) -> usize {
    pixels[ctr..]
        .iter()
        .take_while(|p| {
            let l = pixel_luma(&p.0) as usize;

            (l < opts.min || l > opts.max) != opts.invert || !mask_fn(opts, *p)
        })
        .count()
}

fn mask_fn(opts: &Opts, p: &Rgba<u8>) -> bool {
    !(opts.mask_alpha && p[3] == 0)
}
