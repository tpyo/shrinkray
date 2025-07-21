use crate::config::Config;
use crate::options::{self, Percentage};
use libvips::ops;
use libvips::{Result as VipsResult, VipsImage};

use opentelemetry::Context as TraceContext;
use opentelemetry::KeyValue;
use opentelemetry::global::ObjectSafeSpan;
use opentelemetry::global::tracer;
use opentelemetry::trace::Span;
use opentelemetry::trace::Tracer;

use std::mem::discriminant;
use tracing::error;

pub struct Image {
    pub bytes: Vec<u8>,
    pub content_type: options::ImageFormat,
}

pub fn flatten(
    image: &VipsImage,
    colour: &options::Colour,
    cx: &TraceContext,
) -> VipsResult<VipsImage> {
    let mut span = tracer("shrinkray").start_with_context("flatten", cx);
    let opts = ops::FlattenOptions {
        background: colour.into(),
        ..Default::default()
    };

    let result = ops::flatten_with_opts(image, &opts);
    <dyn ObjectSafeSpan>::end(&mut span);
    result
}

fn find_trim(
    image: &VipsImage,
    options: &options::ImageOptions,
) -> VipsResult<(i32, i32, i32, i32)> {
    let mut opts = ops::FindTrimOptions {
        threshold: 40.0,
        background: vec![255.0, 255.0, 255.0],
        line_art: false,
    };

    if let Some(colour) = &options.trim_colour {
        opts.background = colour.into();
    }

    // Check if the image has alpha
    if opts.background.len() == 4 {
        // Flatten the image before trimming to avoid issues with alpha channels
        let flatten_opts = ops::FlattenOptions {
            // Use magenta as the background colour
            background: vec![255.0, 0.0, 255.0],
            ..Default::default()
        };
        let copy = ops::flatten_with_opts(image, &flatten_opts)?;

        // Fetch the new background colour from the top left corner
        opts.background = ops::getpoint(&copy, 0, 0)?;

        ops::find_trim_with_opts(&copy, &opts)
    } else {
        ops::find_trim_with_opts(image, &opts)
    }
}

fn trim(
    image: &VipsImage,
    options: &options::ImageOptions,
    cx: &TraceContext,
) -> VipsResult<VipsImage> {
    let mut span = tracer("shrinkray").start_with_context("trim", cx);
    let result = match find_trim(image, options) {
        Ok((left, top, width, height)) => ops::extract_area(image, left, top, width, height),
        Err(err) => {
            // If the image is not trimmed, return the original image
            error!("unable to trim image: {}", err);
            Ok(image.clone())
        }
    };
    <dyn ObjectSafeSpan>::end(&mut span);
    result
}

fn percent_to_value(p: i32, min: f64, max: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        return min;
    }
    let percent = f64::from(p).clamp(0.0, 100.0) / 100.0;
    min + (max - min) * percent
}

fn sharpen(
    image: &VipsImage,
    options: &mut options::ImageOptions,
    cx: &TraceContext,
) -> VipsResult<VipsImage> {
    let mut span = tracer("shrinkray").start_with_context("sharpen", cx);
    let percentage = options.sharpen.unwrap_or(Percentage(1));
    // min: 0.000001, max: 10, default: 0.5
    let sigma = percent_to_value(percentage.0, 0.000_001, 10.0);
    let opts = ops::SharpenOptions {
        sigma,
        ..Default::default()
    };
    let result = ops::sharpen_with_opts(image, &opts);
    <dyn ObjectSafeSpan>::end(&mut span);
    result
}

fn blur(
    image: &VipsImage,
    options: &mut options::ImageOptions,
    cx: &TraceContext,
) -> VipsResult<VipsImage> {
    let mut span = tracer("shrinkray").start_with_context("blur", cx);
    let percentage = options.blur.unwrap_or(Percentage(1));
    // min: 0, max: 1000, default: 1.5
    let sigma = percent_to_value(percentage.0, 0.0, 50.0);
    let opts = ops::GaussblurOptions {
        min_ampl: 0.001, // min: 0.001, max: 1, default: 0.2
        precision: ops::Precision::Approximate,
    };
    let result = ops::gaussblur_with_opts(image, sigma, &opts);
    <dyn ObjectSafeSpan>::end(&mut span);
    result
}

fn colourspace_is_srgb(image: &VipsImage) -> VipsResult<bool> {
    let interp = image.get_interpretation()?;
    let srgb = ops::Interpretation::Srgb;
    Ok(discriminant(&interp) == discriminant(&srgb))
}

fn colourspace(image: &VipsImage, cx: &TraceContext) -> VipsResult<VipsImage> {
    let mut span = tracer("shrinkray").start_with_context("colourspace", cx);
    let result = if colourspace_is_srgb(image)? {
        Ok(image.clone())
    } else {
        ops::colourspace(image, ops::Interpretation::Srgb)
    };
    <dyn ObjectSafeSpan>::end(&mut span);
    result
}

/// Check if the image needs rotation based on EXIF data
fn needs_rotation(buffer: &[u8]) -> bool {
    match rexif::parse_buffer_quiet(buffer).0 {
        Ok(data) => data.entries.into_iter().any(|e| {
            e.tag == rexif::ExifTag::Orientation
                && e.value.to_i64(0).is_some()
                && e.value.to_i64(0).unwrap() != 0
                && e.value.to_i64(0).unwrap() != 1
        }),
        Err(_) => false,
    }
}

fn load(bytes: &[u8], rotate: bool, cx: &TraceContext) -> VipsResult<VipsImage> {
    let mut span = tracer("shrinkray").start_with_context("load", cx);

    // If rotation is needed, load the image with random access
    let result = if rotate {
        VipsImage::new_from_buffer(bytes, "[access=VIPS_ACCESS_RANDOM]")
    } else {
        VipsImage::new_from_buffer(bytes, "[access=VIPS_ACCESS_SEQUENTIAL]")
    };
    <dyn ObjectSafeSpan>::end(&mut span);
    result
}

pub fn process_image(
    bytes: &[u8],
    options: &mut options::ImageOptions,
    config: &Config,
    cx: &TraceContext,
) -> VipsResult<Image> {
    let tracer = tracer("shrinkray");

    let rotation = options.rotate.is_some() || needs_rotation(bytes);

    let mut image = load(bytes, rotation, cx)?;

    // Rotation
    if rotation {
        image = rotate(&image, options, cx)?;
    }

    // // Trim whitespace
    if options.trim.is_some() {
        image = trim(&image, options, cx)?;
    }

    // Flatten alpha image
    if let Some(background) = &options.background {
        image = flatten(&image, background, cx)?;
    }

    // Resize
    if options.width.is_some() || options.height.is_some() {
        let image_width = image.get_width();
        let image_height = image.get_height();

        // Calculate crop dimensions
        options::calculate_dimensions(options, image_width, image_height);

        image = resize(&image, options, image_width, image_height, cx)?;
    }

    // Sharpen
    if options.sharpen.is_some() {
        image = sharpen(&image, options, cx)?;
    }

    // Blur
    if options.blur.is_some() {
        image = blur(&image, options, cx)?;
    }

    // Filters
    if options.kodachrome.is_some() {
        let mut span = tracer.start_with_context("kodachrome", cx);
        image = apply_style(&image, KODACHROME, options.kodachrome)?;
        <dyn ObjectSafeSpan>::end(&mut span);
    }
    if options.technicolor.is_some() {
        let mut span = tracer.start_with_context("technicolor", cx);
        image = apply_style(&image, TECHNICOLOR, options.technicolor)?;
        <dyn ObjectSafeSpan>::end(&mut span);
    }
    if options.polaroid.is_some() {
        let mut span = tracer.start_with_context("polaroid", cx);
        image = apply_style(&image, POLAROID, options.polaroid)?;
        <dyn ObjectSafeSpan>::end(&mut span);
    }
    if options.vintage.is_some() {
        let mut span = tracer.start_with_context("vintage", cx);
        image = apply_style(&image, VINTAGE, options.vintage)?;
        <dyn ObjectSafeSpan>::end(&mut span);
    }
    if options.sepia.is_some() {
        let mut span = tracer.start_with_context("sepia", cx);
        image = apply_style(&image, SEPIA, options.sepia)?;
        <dyn ObjectSafeSpan>::end(&mut span);
    }
    if options.monochrome.is_some() {
        let mut span = tracer.start_with_context("monochrome", cx);
        image = apply_style(&image, MONOCHROME, options.monochrome)?;
        <dyn ObjectSafeSpan>::end(&mut span);
    }

    // sRGB conversion
    if !colourspace_is_srgb(&image)? {
        image = colourspace(&image, cx)?;
    }

    // Output the image
    output(&image, options, config, cx)
}

fn output(
    image: &VipsImage,
    options: &mut options::ImageOptions,
    _config: &Config,
    cx: &TraceContext,
) -> VipsResult<Image> {
    let mut span = tracer("shrinkray").start_with_context("output", cx);

    let format = options.format.unwrap_or(options::ImageFormat::Jpeg);

    span.set_attributes([KeyValue::new("shrinkray.image.format", format.to_string())]);

    let result = match format {
        options::ImageFormat::Jpeg => Ok(Image {
            bytes: ops::jpegsave_buffer_with_opts(image, &options.into())?,
            content_type: options::ImageFormat::Jpeg,
        }),
        options::ImageFormat::Webp => Ok(Image {
            bytes: ops::webpsave_buffer_with_opts(image, &options.into())?,
            content_type: options::ImageFormat::Webp,
        }),
        options::ImageFormat::Avif => Ok(Image {
            bytes: ops::heifsave_buffer_with_opts(image, &options.into())?,
            content_type: options::ImageFormat::Avif,
        }),
        options::ImageFormat::Png => Ok(Image {
            bytes: ops::pngsave_buffer_with_opts(image, &options.into())?,
            content_type: options::ImageFormat::Png,
        }),
    };
    <dyn ObjectSafeSpan>::end(&mut span);
    result
}

fn rotate(
    image: &VipsImage,
    options: &options::ImageOptions,
    cx: &TraceContext,
) -> VipsResult<VipsImage> {
    let mut span = tracer("shrinkray").start_with_context("rotate", cx);

    let mut image = ops::autorot(image)?;
    if let Some(angle) = &options.rotate {
        span.set_attributes([KeyValue::new("shrinkray.image.rotate", i64::from(angle.0))]);
        image = ops::rotate(&image, angle.into())?;
    }
    <dyn ObjectSafeSpan>::end(&mut span);
    Ok(image)
}

#[allow(clippy::cast_possible_truncation)]
fn resize(
    image: &VipsImage,
    options: &options::ImageOptions,
    image_width: i32,
    image_height: i32,
    cx: &TraceContext,
) -> VipsResult<VipsImage> {
    let mut span = tracer("shrinkray").start_with_context("resize", cx);
    span.set_attributes([
        KeyValue::new("shrinkray.image.width", i64::from(image_width)),
        KeyValue::new("shrinkray.image.height", i64::from(image_height)),
        KeyValue::new("shrinkray.resize.width", options.width.map_or(0, i64::from)),
        KeyValue::new(
            "shrinkray.resize.height",
            options.height.map_or(0, i64::from),
        ),
    ]);
    let scale = options.get_resize_scale(image_width, image_height);
    let mut thumbnail_options = ops::ThumbnailImageOptions {
        import_profile: "sRGB".to_string(),
        export_profile: "sRGB".to_string(),
        crop: ops::Interesting::Centre,
        linear: false,
        size: ops::Size::Both,
        ..Default::default()
    };
    if options.height.is_some() {
        thumbnail_options.height = options.height.unwrap_or(0);
    } else {
        thumbnail_options.height = (f64::from(image_height) * scale) as i32;
    }
    let result =
        ops::thumbnail_image_with_opts(image, options.width.unwrap_or(0), &thumbnail_options);
    <dyn ObjectSafeSpan>::end(&mut span);
    result
}

const KODACHROME: [f64; 9] = [
    1.12855, -0.39673, -0.03992, -0.16404, 1.08352, -0.05498, -0.16786, -0.56034, 1.60148,
];
const POLAROID: [f64; 9] = [
    1.438, -0.062, -0.062, -0.122, 1.378, -0.122, -0.016, -0.016, 1.483,
];
const VINTAGE: [f64; 9] = [
    0.62793, 0.32021, -0.03965, 0.02578, 0.64411, 0.03259, 0.0466, -0.08512, 0.52416,
];
const TECHNICOLOR: [f64; 9] = [
    1.91252, -0.85453, -0.09155, -0.30878, 1.76589, -0.10601, -0.2311, -0.75018, 1.84759,
];
const MONOCHROME: [f64; 9] = [
    0.299, 0.587, 0.114, 0.299, 0.587, 0.114, 0.299, 0.587, 0.114,
];
const SEPIA: [f64; 9] = [
    0.393, 0.769, 0.189, 0.349, 0.686, 0.168, 0.272, 0.534, 0.131,
];

fn apply_style(
    image: &VipsImage,
    array: [f64; 9],
    opacity: Option<options::Percentage>,
) -> VipsResult<VipsImage> {
    let matrix = VipsImage::image_new_matrix_from_array(3, 3, &array)?;
    let mut overlay = ops::recomb(image, &matrix)?;

    // Convert to float band format to apply the opacity
    overlay = ops::cast(&overlay, ops::BandFormat::Float)?;

    overlay = if overlay.image_hasalpha() {
        overlay
    } else {
        ops::bandjoin_const(&overlay, &mut [255.0])?
    };

    let opacity = opacity.unwrap_or(options::Percentage(100));

    let multiply = [1.0, 1.0, 1.0, f64::from(opacity.0) / 100.0];
    let addition = [0.0, 0.0, 0.0, 0.0];
    let mut multiply = multiply.to_vec();
    let mut addition = addition.to_vec();
    overlay = ops::linear(&overlay, &mut multiply, &mut addition)?;

    if opacity == options::Percentage(100) {
        // Return the overlay image without blending
        return ops::cast(&overlay, ops::BandFormat::Uchar);
    }

    overlay = ops::composite_2(image, &overlay, ops::BlendMode::Over)?;

    overlay = ops::cast(&overlay, ops::BandFormat::Uchar)?;

    let colour = &options::Colour {
        r: 255,
        g: 255,
        b: 255,
    };

    let opts = ops::FlattenOptions {
        background: colour.into(),
        ..Default::default()
    };
    overlay = ops::flatten_with_opts(&overlay, &opts)?;

    Ok(overlay)
}
