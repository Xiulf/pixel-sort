# pixel-sort

A pixel sorter written in rust.

## Usage
```sh
$ pixel-sort [input] [output] [command] [options]
```

### Commands
```
linear                          : Sort the image linearly.

sine [amp] [period] [offset]    : Sort the image along a sine wave.
```

### Options
```
--seq                           : Sort a sequence of files.

--min <min>                     : The minimum threshold.

--max <max>                     : The maximum threshold.

--angle <angle>                 : The angle to sort at.

--vertical                      : Sort the image vertically.

--fn <name>                     : The sorting function to use.
                                  [red, green, blue, max, min, chroma, luma, hue, saturation, brightness]

--interval <interval>           : The interval function to use.
                                  [random, threshold]

--invert                        : Invert the image when sorting.

--reverse                       : Sort the image backwards.
```

## Installing
You can install pixel-sort either by downloading it from the releases or by building it from source (see instructions below).

## Building
Building pixel-sort requires [`cargo`](https://github.com/rust-lang/cargo).
```sh
git clone https://github.com/Xiulf/pixel-sort.git
cd pixel-sort

# building
cargo build --release

# installing
cargo install --path . --force
```
