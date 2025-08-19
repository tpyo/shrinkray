use crate::config::Config;
use crate::options::{self, Percentage};
use libvips::ops;
use libvips::{Result as VipsResult, VipsImage};
use std::mem::discriminant;
use tracing::error;

pub struct Image {
    pub bytes: Vec<u8>,
    pub content_type: options::ImageFormat,
}

#[tracing::instrument(skip_all)]
pub fn flatten(image: &VipsImage, colour: &options::Colour) -> VipsResult<VipsImage> {
    let opts = ops::FlattenOptions {
        background: colour.into(),
        ..Default::default()
    };

    ops::flatten_with_opts(image, &opts)
}

#[tracing::instrument(skip_all)]
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

#[tracing::instrument(skip_all)]
fn trim(image: &VipsImage, options: &options::ImageOptions) -> VipsResult<VipsImage> {
    match find_trim(image, options) {
        Ok((left, top, width, height)) => ops::extract_area(image, left, top, width, height),
        Err(err) => {
            // If the image is not trimmed, return the original image
            error!("unable to trim image: {}", err);
            Ok(image.clone())
        }
    }
}

fn percent_to_value(p: i32, min: f64, max: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        return min;
    }
    let percent = f64::from(p).clamp(0.0, 100.0) / 100.0;
    min + (max - min) * percent
}

#[tracing::instrument(skip_all)]
fn sharpen(image: &VipsImage, options: &mut options::ImageOptions) -> VipsResult<VipsImage> {
    let percentage = options.sharpen.unwrap_or(Percentage(1));
    // min: 0.000001, max: 10, default: 0.5
    let sigma = percent_to_value(percentage.0, 0.000_001, 10.0);
    let opts = ops::SharpenOptions {
        sigma,
        ..Default::default()
    };
    ops::sharpen_with_opts(image, &opts)
}

#[tracing::instrument(skip_all)]
fn blur(image: &VipsImage, options: &mut options::ImageOptions) -> VipsResult<VipsImage> {
    let percentage = options.blur.unwrap_or(Percentage(1));
    // min: 0, max: 1000, default: 1.5
    let sigma = percent_to_value(percentage.0, 0.0, 50.0);
    let opts = ops::GaussblurOptions {
        min_ampl: 0.001, // min: 0.001, max: 1, default: 0.2
        precision: ops::Precision::Approximate,
    };
    ops::gaussblur_with_opts(image, sigma, &opts)
}

fn colourspace_is_srgb(image: &VipsImage) -> VipsResult<bool> {
    let interp = image.get_interpretation()?;
    let srgb = ops::Interpretation::Srgb;
    Ok(discriminant(&interp) == discriminant(&srgb))
}

#[tracing::instrument(skip_all)]
fn colourspace(image: &VipsImage) -> VipsResult<VipsImage> {
    if colourspace_is_srgb(image)? {
        Ok(image.clone())
    } else {
        ops::colourspace(image, ops::Interpretation::Srgb)
    }
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

#[tracing::instrument(skip_all)]
fn load(bytes: &[u8], random_access: bool) -> VipsResult<VipsImage> {
    // If rotation is needed, load the image with random access
    if random_access {
        VipsImage::new_from_buffer(bytes, "[access=VIPS_ACCESS_RANDOM]")
    } else {
        VipsImage::new_from_buffer(bytes, "[access=VIPS_ACCESS_SEQUENTIAL]")
    }
}

#[tracing::instrument(skip_all, parent = span)]
pub fn process_image(
    bytes: &[u8],
    options: &mut options::ImageOptions,
    config: &Config,
    span: tracing::Span,
) -> VipsResult<Image> {
    let rotation = options.rotate.is_some() || needs_rotation(bytes);
    let random_access = rotation || options.trim.is_some();

    let mut image = load(bytes, random_access)?;

    // Rotation
    if rotation {
        image = rotate(&image, options)?;
    }

    // // Trim whitespace
    if options.trim.is_some() {
        image = trim(&image, options)?;
    }

    // Flatten alpha image
    if let Some(background) = &options.background {
        image = flatten(&image, background)?;
    }

    // Resize
    if options.width.is_some() || options.height.is_some() {
        let image_width = image.get_width();
        let image_height = image.get_height();

        // Calculate crop dimensions
        options::calculate_dimensions(options, image_width, image_height);

        image = resize(&image, options, image_width, image_height)?;
    }

    // Sharpen
    if options.sharpen.is_some() {
        image = sharpen(&image, options)?;
    }

    // Blur
    if options.blur.is_some() {
        image = blur(&image, options)?;
    }

    // Filters
    if options.kodachrome.is_some() {
        image = apply_style(&image, KODACHROME, options.kodachrome)?;
    }
    if options.technicolor.is_some() {
        image = apply_style(&image, TECHNICOLOR, options.technicolor)?;
    }
    if options.polaroid.is_some() {
        image = apply_style(&image, POLAROID, options.polaroid)?;
    }
    if options.vintage.is_some() {
        image = apply_style(&image, VINTAGE, options.vintage)?;
    }
    if options.sepia.is_some() {
        image = apply_style(&image, SEPIA, options.sepia)?;
    }
    if options.monochrome.is_some() {
        image = apply_style(&image, MONOCHROME, options.monochrome)?;
    }

    // sRGB conversion
    if !colourspace_is_srgb(&image)? {
        image = colourspace(&image)?;
    }

    // Output the image
    output(&image, options, config)
}

#[tracing::instrument(skip_all)]
fn output(
    image: &VipsImage,
    options: &mut options::ImageOptions,
    _config: &Config,
) -> VipsResult<Image> {
    let format = options.format.unwrap_or(options::ImageFormat::Jpeg);

    tracing::Span::current().record("shrinkray.image.format", format.to_string());

    match format {
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
    }
}

#[tracing::instrument(skip_all)]
fn rotate(image: &VipsImage, options: &options::ImageOptions) -> VipsResult<VipsImage> {
    let mut image = ops::autorot(image)?;
    if let Some(angle) = &options.rotate {
        image = ops::rotate(&image, angle.into())?;
        tracing::Span::current().record("shrinkray.rotate", i64::from(angle.0));
    }
    Ok(image)
}

#[allow(clippy::cast_possible_truncation)]
#[tracing::instrument(skip_all, fields(shrinkray.width = image_width, shrinkray.height = image_height))]
fn resize(
    image: &VipsImage,
    options: &options::ImageOptions,
    image_width: i32,
    image_height: i32,
) -> VipsResult<VipsImage> {
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
    ops::thumbnail_image_with_opts(image, options.width.unwrap_or(0), &thumbnail_options)
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

#[tracing::instrument(skip_all)]
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

    ops::flatten_with_opts(&overlay, &opts)
}
