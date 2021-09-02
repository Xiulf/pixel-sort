use clap::{App, Arg, SubCommand};
use pixel_sort::*;

fn main() {
    let matches = App::new("pixel-sort")
        .arg(Arg::with_name("sequence").long("seq"))
        .arg(Arg::with_name("input").takes_value(true).required(true))
        .arg(Arg::with_name("output").takes_value(true).required(true))
        .arg(
            Arg::with_name("min")
                .long("min")
                .takes_value(true)
                .default_value("0"),
        )
        .arg(
            Arg::with_name("max")
                .long("max")
                .takes_value(true)
                .default_value("255"),
        )
        .arg(Arg::with_name("angle").long("angle").takes_value(true))
        .arg(Arg::with_name("vertical").long("vertical"))
        .arg(
            Arg::with_name("fn")
                .long("fn")
                .takes_value(true)
                .possible_values(&[
                    "red",
                    "green",
                    "blue",
                    "max",
                    "min",
                    "chroma",
                    "hue",
                    "luma",
                    "saturation",
                    "brightness",
                ]),
        )
        .arg(
            Arg::with_name("interval")
                .long("interval")
                .short("i")
                .takes_value(true)
                .possible_values(&["random", "threshold"]),
        )
        .arg(Arg::with_name("invert").long("invert"))
        .arg(Arg::with_name("reverse").long("reverse"))
        .subcommand(SubCommand::with_name("linear"))
        .subcommand(
            SubCommand::with_name("sine")
                .arg(Arg::with_name("amp").takes_value(true).required(true))
                .arg(Arg::with_name("lam").takes_value(true).required(true))
                .arg(Arg::with_name("offset").takes_value(true).required(true)),
        )
        .get_matches();

    let opts = Opts {
        sort_type: if let Some(_) = matches.subcommand_matches("linear") {
            SortType::Linear
        } else if let Some(matches) = matches.subcommand_matches("sine") {
            SortType::Sine {
                amp: matches.value_of("amp").unwrap().parse().unwrap(),
                lam: matches.value_of("lam").unwrap().parse().unwrap(),
                offset: matches.value_of("offset").unwrap().parse().unwrap(),
            }
        } else {
            SortType::Linear
        },
        sort_fn: match matches.value_of("fn") {
            Some("red") => pixel_red,
            Some("green") => pixel_green,
            Some("blue") => pixel_blue,
            Some("max") => pixel_max,
            Some("min") => pixel_min,
            Some("chroma") => pixel_chroma,
            Some("hue") => pixel_hue,
            Some("saturation") => pixel_saturation,
            Some("brightness") => pixel_brightness,
            Some("luma") => pixel_luma,
            None => pixel_max,
            _ => panic!("invalid sort function"),
        },
        interval: match matches.value_of("interval") {
            Some("random") => IntervalType::Random,
            Some("threshold") => IntervalType::Threshold,
            None => IntervalType::Random,
            _ => panic!("invalid interval type"),
        },
        mask_alpha: false,
        invert: matches.occurrences_of("invert") >= 1,
        reverse: matches.occurrences_of("reverse") >= 1,
        min: matches.value_of("min").unwrap().parse().unwrap(),
        max: matches.value_of("max").unwrap().parse().unwrap(),
        angle: matches
            .value_of("angle")
            .and_then(|a| a.parse().ok())
            .unwrap_or(0.0),
        vertical: matches.occurrences_of("vertical") >= 1,
    };

    let input = matches.value_of("input").unwrap();
    let output = matches.value_of("output").unwrap();

    if matches.occurrences_of("sequence") >= 1 {
        let output = find_parts(output);
        let pb = indicatif::ProgressBar::new(0).with_style(
            indicatif::ProgressStyle::default_bar()
                .template("{prefix} [{bar:40.cyan/blue}] {pos::>5}/{len}")
                .progress_chars("=> "),
        );

        for (i, input) in FileSeq::new(input).enumerate() {
            pb.set_prefix(&format!("Sorting {}", input));
            pb.set_position(0);

            let image = image::open(&input).unwrap();
            let sorted = pixel_sort::img::sort_image(&pb, image, &opts);
            let output = format!("{}{:0>width$}{}", output.0, i, output.1, width = output.2);

            sorted.save(output).unwrap();
        }
    } else {
        let pb = indicatif::ProgressBar::new(0).with_style(
            indicatif::ProgressStyle::default_bar()
                .template("{prefix} [{bar:40.cyan/blue}] {pos::>5}/{len}")
                .progress_chars("=> "),
        );

        pb.set_prefix(&format!("Sorting {}", input));

        let image = image::open(input).expect(input);
        let sorted = pixel_sort::img::sort_image(&pb, image, &opts);

        sorted.save(output).unwrap();
    }
}

fn find_parts(filename: &str) -> (&str, &str, usize, usize) {
    let re = regex::Regex::new(r"\[\*+(/\d+)?\]").unwrap();
    let seq = re.find(filename).unwrap();
    let captures = re.captures(filename).unwrap();
    let prefix = &filename[..seq.start()];
    let start = captures.get(1).map(|s| s.as_str()).unwrap_or("");
    let num = seq.end() - seq.start() - 2 - start.len();
    let suffix = &filename[seq.end()..];
    let start = if start.is_empty() {
        1
    } else {
        start[1..].parse().unwrap()
    };

    (prefix, suffix, num, start)
}

struct FileSeq<'a> {
    filename: (&'a str, &'a str, usize),
    idx: usize,
}

impl<'a> FileSeq<'a> {
    fn new(filename: &'a str) -> Self {
        let (a, b, c, idx) = find_parts(filename);

        FileSeq {
            filename: (a, b, c),
            idx,
        }
    }
}

impl<'a> Iterator for FileSeq<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let filename = format!(
            "{}{:0>width$}{}",
            self.filename.0,
            self.idx,
            self.filename.1,
            width = self.filename.2
        );

        let path = std::path::PathBuf::from(&filename);

        if path.exists() {
            self.idx += 1;

            Some(filename)
        } else {
            None
        }
    }
}
