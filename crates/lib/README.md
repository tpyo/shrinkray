# shrinkray

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance Rust library for image processing powered by [libvips](https://www.libvips.org/).

## Features

- **Fast image processing** - Built on libvips for exceptional performance
- **Fluent API** - Intuitive builder pattern for chaining operations
- **Format conversion** - Support for JPEG, PNG, WebP, and AVIF
- **Resizing & cropping** - Multiple fit modes (crop, clip, max)
- **Filters & effects** - Vintage, sepia, monochrome, duotone, and more
- **Image adjustments** - Sharpen, blur, tint, rotation, and trimming
- **Type-safe** - Strong typing for colours, formats, and options

## Installation

```bash
cargo add shrinkray
```

### System Dependencies

- **libvips 8.17.3** (for older 8.16.x libvips versions use shrinkray 1.0.1)
- libaom (for AVIF support via libheif)
- libheif
- libjpeg
- libpng
- libwebp

For more information on installing the required dependencies, see the [Ubuntu](https://github.com/tpyo/shrinkray/blob/main/docker/Dockerfile.base-slim-bookworm) and [Alpine](https://github.com/tpyo/shrinkray/blob/main/docker/Dockerfile.base-alpine) base images.

## Usage

### Basic Example

```rust
use shrinkray::ImageProcessor;
use shrinkray::options::{Format, Fit};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read input image
    let image_bytes = std::fs::read("input.jpg")?;

    // Process the image
    let result = ImageProcessor::new(&image_bytes)
        .resize(800, 600)
        .quality(85)
        .format(Format::Webp)
        .fit(Fit::Crop)
        .process()?;

    // Save the result
    std::fs::write("output.webp", result.bytes())?;

    Ok(())
}
```

### Create Thumbnails

```rust
let thumbnail = ImageProcessor::new(&image_bytes)
    .thumbnail(200)  // 200x200 square thumbnail
    .process()?;
```

### Apply Filters

```rust
let filtered = ImageProcessor::new(&image_bytes)
    .vintage(80)
    .sharpen(40)
    .process()?;
```

### Duotone Effect

```rust
let duotone = ImageProcessor::new(&image_bytes)
    .duotone(
        0, 50, 100,      // Shadow colour (dark blue)
        255, 165, 0      // Highlight colour (orange)
    )
    .duotone_alpha(75)   // 75% opacity
    .process()?;
```

### Rotation and Trimming

```rust
let trimmed = ImageProcessor::new(&image_bytes)
    .rotate(90)
    .trim()  // Auto-trim transparent/whitespace borders
    .process()?;
```

### Custom Aspect Ratio

```rust
let cropped = ImageProcessor::new(&image_bytes)
    .aspect_ratio(16, 9)
    .width(1920)
    .fit(Fit::Crop)
    .process()?;
```

### Device Pixel Ratio (Retina Support)

```rust
let retina = ImageProcessor::new(&image_bytes)
    .width(400)
    .height(300)
    .device_pixel_ratio(2)  // Output will be 800x600
    .process()?;
```

## Available Options

### Resizing
- `width(i32)` - Set output width
- `height(i32)` - Set output height
- `resize(i32, i32)` - Set both width and height
- `thumbnail(i32)` - Create square thumbnail
- `fit(Fit)` - Resize behavior (Crop, Clip, Max)
- `device_pixel_ratio(i32)` - DPR multiplier
- `aspect_ratio(i32, i32)` - Target aspect ratio

### Format & Quality
- `format(Format)` - Output format (Jpeg, Png, Webp, Avif)
- `quality(i32)` - Quality 1-100 (default: 75)
- `lossless(bool)` - Enable lossless compression

### Adjustments
- `rotate(u16)` - Rotate 90, 180, or 270 degrees
- `sharpen(u8)` - Sharpening intensity 1-100
- `blur(u8)` - Blur amount 1-100
- `trim()` - Auto-trim borders
- `trim_colour(u8, u8, u8)` - Trim specific colour
- `background(u8, u8, u8)` - Background colour for padding/flattening

### Filters
- `grayscale()` - Convert to grayscale
- `monochrome(u8)` - Monochrome with intensity
- `sepia(u8)` - Sepia tone filter
- `vintage(u8)` - Vintage film effect
- `polaroid(u8)` - Polaroid effect
- `kodachrome(u8)` - Kodachrome film simulation
- `technicolor(u8)` - Technicolor effect
- `tint(u8, u8, u8)` - Colour tint overlay
- `duotone(u8, u8, u8, u8, u8, u8)` - Duotone with shadow and highlight colours
- `duotone_alpha(u8)` - Duotone opacity 1-100

### Safety Limits
- `max_megapixels(f64)` - Maximum input image size
- `max_output_resolution(u32)` - Maximum output dimension

## ProcessedImage API

The `process()` method returns a `ProcessedImage` with these methods:

- `bytes(&self) -> &[u8]` - Get image bytes as a slice
- `into_bytes(self) -> Vec<u8>` - Consume and get owned bytes
- `format(&self) -> Format` - Get the output format
- `mime_type(&self) -> &'static str` - Get MIME type string

## Full Server Implementation

This library powers the [shrinkray image server](https://github.com/tpyo/shrinkray), which provides:
- HTTP/S3/file backend routing
- HMAC signature verification
- Prometheus metrics
- OpenTelemetry tracing
- Kubernetes deployment configs

See the [main repository](https://github.com/tpyo/shrinkray) for the complete server implementation.

## License

MIT License - see [LICENSE](../../LICENSE) for details.
