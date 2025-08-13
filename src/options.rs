use libvips::ops;
use ring::hmac;
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::PartialOrd;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::{Div, Mul};
use strum::Display;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub struct ImageOptions {
    #[serde(default, rename = "sig", skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    /// Background color
    #[serde(
        default,
        rename = "bg",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_colour"
    )]
    pub background: Option<Colour>,

    /// Aspect ratio
    #[serde(
        default,
        rename = "ar",
        deserialize_with = "deserialize_aspect_ratio",
        skip_serializing_if = "Option::is_none"
    )]
    pub aspect_ratio: Option<AspectRatio>,

    /// Lossless output
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lossless: Option<bool>,

    /// Quality
    #[serde(default, rename = "q", skip_serializing_if = "Option::is_none")]
    pub quality: Option<i32>,

    /// Device Pixel Ratio
    #[serde(default, rename = "dpr", skip_serializing_if = "Option::is_none")]
    pub device_pixel_ratio: Option<i32>,

    /// Rotation
    #[serde(
        default,
        rename = "rot",
        deserialize_with = "deserialize_rotation",
        skip_serializing_if = "Option::is_none"
    )]
    pub rotate: Option<Rotation>,

    /// Width
    #[serde(
        default,
        rename = "w",
        deserialize_with = "deserialize_dimension",
        skip_serializing_if = "Option::is_none"
    )]
    pub width: Option<i32>,

    /// Height
    #[serde(
        default,
        rename = "h",
        deserialize_with = "deserialize_dimension",
        skip_serializing_if = "Option::is_none"
    )]
    pub height: Option<i32>,

    /// Fit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<Fit>,

    /// Image format
    #[serde(default, rename = "fm", skip_serializing_if = "Option::is_none")]
    pub format: Option<ImageFormat>,

    /// Download
    #[serde(default, rename = "dl", skip_serializing_if = "Option::is_none")]
    pub download: Option<String>,

    /// Trim
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<Trim>,

    #[serde(
        default,
        rename = "trim-colour",
        deserialize_with = "deserialize_colour",
        skip_serializing_if = "Option::is_none"
    )]
    pub trim_colour: Option<Colour>,
    //pub heif_effort: i32,
    //pub heif_encoder: Encoder,

    // Sharpen
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharpen: Option<Percentage>,

    // Blur
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur: Option<Percentage>,

    // Filters
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kodachrome: Option<Percentage>,

    #[serde(
        default,
        deserialize_with = "deserialize_percentage",
        skip_serializing_if = "Option::is_none"
    )]
    pub technicolor: Option<Percentage>,

    #[serde(
        default,
        deserialize_with = "deserialize_percentage",
        skip_serializing_if = "Option::is_none"
    )]
    pub vintage: Option<Percentage>,

    #[serde(
        default,
        deserialize_with = "deserialize_percentage",
        skip_serializing_if = "Option::is_none"
    )]
    pub polaroid: Option<Percentage>,

    #[serde(
        default,
        deserialize_with = "deserialize_percentage",
        skip_serializing_if = "Option::is_none"
    )]
    pub sepia: Option<Percentage>,

    #[serde(
        default,
        deserialize_with = "deserialize_percentage",
        skip_serializing_if = "Option::is_none"
    )]
    pub monochrome: Option<Percentage>,
}

impl Default for ImageOptions {
    fn default() -> Self {
        ImageOptions {
            signature: None,
            background: None,
            quality: None,
            aspect_ratio: None,
            download: None,
            trim: None,
            trim_colour: None,
            sharpen: None,
            blur: None,
            kodachrome: None,
            technicolor: None,
            vintage: None,
            polaroid: None,
            sepia: None,
            monochrome: None,
            width: None,
            height: None,
            device_pixel_ratio: Some(1),
            rotate: None,
            format: None,
            //heif_effort: 6, // 0-6
            //heif_encoder: Encoder::Rav1E,
            lossless: None,
            fit: None,
        }
    }
}

impl ImageOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn any_set(&self) -> bool {
        self.signature.is_some()
            || self.background.is_some()
            || (self.quality.is_some() && self.quality.unwrap() != 75)
            || self.aspect_ratio.is_some()
            || self.download.is_some()
            || self.trim.is_some()
            || self.trim_colour.is_some()
            || self.sharpen.is_some()
            || self.blur.is_some()
            || self.kodachrome.is_some()
            || self.technicolor.is_some()
            || self.vintage.is_some()
            || self.polaroid.is_some()
            || self.sepia.is_some()
            || self.monochrome.is_some()
            || self.width.is_some()
            || self.height.is_some()
            || self.device_pixel_ratio.is_some()
            || self.rotate.is_some()
            || self.fit.is_some()
            || self.format.is_some()
            || self.lossless.is_some()
    }

    /// Calculate the resize scale based on the image dimensions and the specified width and height.
    pub fn get_resize_scale(&self, image_width: i32, image_height: i32) -> f64 {
        if let Some(width) = self.width {
            let scale_x = f64::from(width) / f64::from(image_width);
            if let Some(height) = self.height {
                scale_x.min(f64::from(height) / f64::from(image_height))
            } else {
                scale_x
            }
        } else if let Some(height) = self.height {
            f64::from(height) / f64::from(image_height)
        } else {
            1.0
        }
    }

    pub fn query_str(&self) -> String {
        let mut params: BTreeMap<String, String> = BTreeMap::new();

        // Insert each option if it has a value
        if let Some(bg) = &self.background {
            params.insert("background".into(), bg.into());
        }
        if let Some(q) = self.quality {
            params.insert("quality".into(), q.to_string());
        }
        if let Some(ar) = &self.aspect_ratio {
            params.insert("ar".into(), ar.to_string());
        }
        if let Some(download) = &self.download {
            params.insert("download".into(), download.to_string());
        }
        if let Some(trim) = &self.trim {
            params.insert("trim".into(), trim.to_string());
        }
        if let Some(trim_colour) = &self.trim_colour {
            params.insert("trim-colour".into(), trim_colour.into());
        }
        if let Some(sharpen) = &self.sharpen {
            params.insert("sharpen".into(), sharpen.0.to_string());
        }
        if let Some(blur) = &self.blur {
            params.insert("blur".into(), blur.0.to_string());
        }
        if let Some(kodachrome) = &self.kodachrome {
            params.insert("kodachrome".into(), kodachrome.0.to_string());
        }
        if let Some(technicolor) = &self.technicolor {
            params.insert("technicolor".into(), technicolor.0.to_string());
        }
        if let Some(vintage) = &self.vintage {
            params.insert("vintage".into(), vintage.0.to_string());
        }
        if let Some(polaroid) = &self.polaroid {
            params.insert("polaroid".into(), polaroid.0.to_string());
        }
        if let Some(sepia) = &self.sepia {
            params.insert("sepia".into(), sepia.0.to_string());
        }
        if let Some(monochrome) = &self.monochrome {
            params.insert("monochrome".into(), monochrome.0.to_string());
        }
        if let Some(width) = self.width {
            params.insert("width".into(), width.to_string());
        }
        if let Some(height) = self.height {
            params.insert("height".into(), height.to_string());
        }
        if let Some(dpr) = self.device_pixel_ratio {
            params.insert("dpr".into(), dpr.to_string());
        }
        if let Some(rot) = &self.rotate {
            params.insert("rot".into(), rot.0.to_string());
        }
        if let Some(fit) = &self.fit {
            params.insert("fit".into(), fit.to_string().to_lowercase());
        }
        if let Some(fmt) = &self.format {
            params.insert("format".into(), fmt.to_string());
        }
        if let Some(lossless) = self.lossless {
            params.insert("lossless".into(), lossless.to_string());
        }

        // Create the query string
        params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&")
    }

    pub fn sign(&self, secret: &str) -> std::string::String {
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        hex::encode(hmac::sign(&key, self.query_str().as_bytes()).as_ref())
    }

    pub fn verify_signature(&self, signing_secret: &str) -> bool {
        if let Some(ref sig_hex) = self.signature {
            if let Ok(sig_bytes) = hex::decode(sig_hex) {
                let key = hmac::Key::new(hmac::HMAC_SHA256, signing_secret.as_bytes());
                return ring::hmac::verify(&key, self.query_str().as_bytes(), &sig_bytes).is_ok();
            }
        }
        false
    }
}

#[derive(Debug, Serialize, Clone, Copy, Deserialize, PartialEq)]
pub struct Percentage(pub i32);

impl From<&Percentage> for f64 {
    fn from(val: &Percentage) -> Self {
        f64::from(val.0)
    }
}

fn deserialize_percentage<'de, D>(deserializer: D) -> Result<Option<Percentage>, D::Error>
where
    D: Deserializer<'de>,
{
    let result = String::deserialize(deserializer);
    match result {
        Ok(value) => {
            let percentage = value.parse::<i32>().map_err(|err| {
                serde::de::Error::custom(format!("failed to parse percenpercentagetage: {}", err))
            })?;
            if !(1..=100).contains(&percentage) {
                return Err(serde::de::Error::custom(
                    "percentage must be between 1 and 100",
                ));
            }
            Ok(Some(Percentage(percentage)))
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct Rotation(pub i32);

impl From<&Rotation> for f64 {
    fn from(val: &Rotation) -> Self {
        f64::from(val.0)
    }
}

fn deserialize_rotation<'de, D>(deserializer: D) -> Result<Option<Rotation>, D::Error>
where
    D: Deserializer<'de>,
{
    let result = i32::deserialize(deserializer);
    match result {
        Ok(value) => {
            if value == 90 || value == 180 || value == 270 {
                return Ok(Some(Rotation(value)));
            }
            Err(serde::de::Error::custom(
                "rotation must be one of 90, 180, or 270",
            ))
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for Colour {
    fn default() -> Self {
        Colour {
            r: 255,
            g: 255,
            b: 255,
        }
    }
}

impl From<&Colour> for Vec<f64> {
    fn from(val: &Colour) -> Self {
        vec![f64::from(val.r), f64::from(val.g), f64::from(val.b)]
    }
}

impl From<&Colour> for String {
    fn from(val: &Colour) -> Self {
        format!("{:02x}{:02x}{:02x}", val.r, val.g, val.b)
    }
}

fn deserialize_colour<'de, D>(deserializer: D) -> Result<Option<Colour>, D::Error>
where
    D: Deserializer<'de>,
{
    let result = String::deserialize(deserializer);
    match result {
        Ok(value) => {
            if value.is_empty() || value.len() != 6 {
                return Ok(None);
            }
            let r = u8::from_str_radix(&value[0..2], 16)
                .map_err(|err| serde::de::Error::custom(err.to_string()))?;
            let g = u8::from_str_radix(&value[2..4], 16)
                .map_err(|err| serde::de::Error::custom(err.to_string()))?;
            let b = u8::from_str_radix(&value[4..6], 16)
                .map_err(|err| serde::de::Error::custom(err.to_string()))?;

            let colour = Colour { r, g, b };
            Ok(Some(colour))
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Display, PartialEq, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[strum(serialize = "avif")]
    Avif,
    #[strum(serialize = "jpeg")]
    Jpeg,
    #[strum(serialize = "webp")]
    Webp,
    #[strum(serialize = "png")]
    Png,
}

impl ImageFormat {
    #[must_use]
    pub fn content_type(self) -> &'static str {
        match self {
            ImageFormat::Avif => "image/avif",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Png => "image/png",
        }
    }
}

#[derive(Debug, Display, PartialEq, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Trim {
    /// Automatically trim the image using either a colour or the alpha channel
    #[strum(serialize = "auto")]
    Auto,
    /// Trim the image using a colour
    #[strum(serialize = "colour")]
    Colour,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize)]
pub struct AspectRatio {
    pub ratio: f64,
    pub x: i32,
    pub y: i32,
}

impl AspectRatio {
    #[inline]
    fn new(x: i32, y: i32) -> Self {
        Self {
            ratio: f64::from(x) / f64::from(y),
            x,
            y,
        }
    }
}

impl Display for AspectRatio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.x, self.y)
    }
}

impl Div<AspectRatio> for i32 {
    type Output = i32;

    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    fn div(self, aspect_ratio: AspectRatio) -> Self::Output {
        (f64::from(self) / aspect_ratio.ratio).round() as i32
    }
}

impl Mul<AspectRatio> for i32 {
    type Output = i32;

    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    fn mul(self, aspect_ratio: AspectRatio) -> Self::Output {
        (f64::from(self) * aspect_ratio.ratio).round() as i32
    }
}

impl From<&mut ImageOptions> for ops::HeifsaveBufferOptions {
    fn from(options: &mut ImageOptions) -> ops::HeifsaveBufferOptions {
        let mut opts = ops::HeifsaveBufferOptions {
            q: options.quality.unwrap_or(75),
            lossless: options.lossless.unwrap_or(false),
            compression: ops::ForeignHeifCompression::Hevc,
            effort: 4,
            ..Default::default()
        };
        if let Some(ImageFormat::Avif) = options.format {
            opts.compression = ops::ForeignHeifCompression::Av1;
            opts.bitdepth = 8;
        }
        opts
    }
}

impl From<&mut ImageOptions> for ops::WebpsaveBufferOptions {
    fn from(options: &mut ImageOptions) -> ops::WebpsaveBufferOptions {
        ops::WebpsaveBufferOptions {
            q: options.quality.unwrap_or(80),
            lossless: options.lossless.unwrap_or(false),
            ..Default::default()
        }
    }
}

impl From<&mut ImageOptions> for ops::JpegsaveBufferOptions {
    fn from(options: &mut ImageOptions) -> ops::JpegsaveBufferOptions {
        ops::JpegsaveBufferOptions {
            q: options.quality.unwrap_or(80),
            optimize_coding: false,
            // Setting interlace to true slows down the encoding process significantly
            interlace: false,
            ..Default::default()
        }
    }
}

impl From<&mut ImageOptions> for ops::PngsaveBufferOptions {
    fn from(options: &mut ImageOptions) -> ops::PngsaveBufferOptions {
        ops::PngsaveBufferOptions {
            q: options.quality.unwrap_or(80),
            compression: 6,
            interlace: true,
            ..Default::default()
        }
    }
}

#[derive(Display, PartialEq, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Fits within bounds without cropping or distortion, maintaining aspect ratio.
    Clip,

    /// Fills dimensions, cropping excess without distortion.
    Crop,

    /// Fits within bounds without cropping or distortion but won't upscale smaller images.
    Max,
}

fn deserialize_aspect_ratio<'de, D>(deserializer: D) -> Result<Option<AspectRatio>, D::Error>
where
    D: Deserializer<'de>,
{
    let result = String::deserialize(deserializer);
    match result {
        Ok(value) if value.is_empty() => Ok(None),
        Ok(value) => {
            let parts: Vec<&str> = value.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(numerator), Ok(denominator)) =
                    (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                {
                    return Ok(Some(AspectRatio::new(numerator, denominator)));
                }
            }
            Err(serde::de::Error::custom("invalid aspect ratio"))
        }
        Err(err) => Err(err),
    }
}

fn deserialize_dimension<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let result = i32::deserialize(deserializer);
    match result {
        Ok(value) if value > 0 => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(err) => Err(err),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn calculate_crop_dimensions(
    image_options: &ImageOptions,
    image_width: i32,
    image_height: i32,
    aspect_ratio: Option<AspectRatio>,
) -> (i32, i32) {
    // Use the given `ar` or default to the image's aspect ratio
    let ar = aspect_ratio.unwrap_or(AspectRatio::new(image_width, image_height));
    match (image_options.width, image_options.height) {
        // If no constraints are provided, use the original dimensions
        (None, None) => (image_width, image_height),

        // If only width is provided, calculate the height based on aspect ratio
        (Some(width), None) => (width, width / ar),

        // If only height is provided, calculate the width based on aspect ratio
        (None, Some(height)) => ((height * ar), height),

        // If both width and height are specified, use them directly
        (Some(width), Some(height)) => (width, height),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn calculate_clip_dimensions(
    image_options: &ImageOptions,
    image_width: i32,
    image_height: i32,
    aspect_ratio: Option<AspectRatio>,
) -> (i32, i32) {
    let ar = aspect_ratio.unwrap_or(AspectRatio::new(image_width, image_height));
    match (image_options.width, image_options.height) {
        // If no constraints are provided, use the original dimensions
        (None, None) => (image_width, image_height),

        // If only width is provided, calculate the height based on aspect ratio
        (Some(width), None) => (width, width / ar),

        // If only height is provided, calculate the width based on aspect ratio
        (None, Some(height)) => (height * ar, height),

        // If both width and height are provided, fit within the bounding box
        (Some(width), Some(height)) => {
            let target_aspect_ratio = AspectRatio::new(width, height);
            if target_aspect_ratio > ar {
                (height * ar, height)
            } else {
                (width, width / ar)
            }
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn calculate_max_dimensions(
    image_options: &ImageOptions,
    image_width: i32,
    image_height: i32,
    aspect_ratio: Option<AspectRatio>,
) -> (i32, i32) {
    let ar = aspect_ratio.unwrap_or(AspectRatio::new(image_width, image_height));
    match (image_options.width, image_options.height) {
        // If no constraints are provided, use the original dimensions
        (None, None) => (image_width, image_height),

        // Constrained by width only, but do not upscale
        (Some(width), None) => {
            let new_width = width.min(image_width);
            let new_height = (new_width / ar).min(image_height);
            (new_width, new_height)
        }

        // Constrained by height only, but do not upscale
        (None, Some(height)) => {
            let new_height = height.min(image_height);
            let new_width = (new_height * ar).min(image_width);
            (new_width, new_height)
        }

        // Constrained by both width and height, but do not upscale
        (Some(width), Some(height)) => {
            let target_aspect_ratio = AspectRatio::new(width, height);

            if target_aspect_ratio > ar {
                // Fit by height
                let new_height = height.min(image_height);
                let new_width = (new_height * ar).min(image_width);
                (new_width, new_height)
            } else {
                // Fit by width
                let new_width = width.min(image_width);
                let new_height = (new_width / ar).min(image_height);
                (new_width, new_height)
            }
        }
    }
}

pub fn calculate_dimensions(image_options: &mut ImageOptions, image_width: i32, image_height: i32) {
    let dpr = image_options.device_pixel_ratio.unwrap_or(1);

    let aspect_ratio = match image_options.aspect_ratio.clone() {
        Some(ar) => Some(ar),
        None => Some(AspectRatio::new(image_width * dpr, image_height * dpr)),
    };

    // Determine the new dimensions based on the `fit` parameter
    let (width, height) = match image_options.fit {
        Some(Fit::Crop) => {
            calculate_crop_dimensions(image_options, image_width, image_height, aspect_ratio)
        }
        Some(Fit::Max) => {
            calculate_max_dimensions(image_options, image_width, image_height, aspect_ratio)
        }
        Some(Fit::Clip) | None => {
            calculate_clip_dimensions(image_options, image_width, image_height, aspect_ratio)
        }
    };

    // Apply the Device Pixel Ratio (DPR) scaling
    image_options.width = Some(width * dpr);
    image_options.height = Some(height * dpr);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::Query, http::Uri};
    use rstest::rstest;

    #[rstest]
    // No resizing
    #[case::no_resizing("", (600, 400), (600, 400))]
    #[case::no_resizing("?", (600, 400), (600, 400))]
    // Clip: Width only
    #[case::clip_width_only_invalid("?w=0&fit=clip", (600, 400), (600, 400))]
    #[case::clip_width_only("?w=150&fit=clip", (600, 400), (150, 100))]
    #[case::clip_width_only("?w=300&fit=clip", (600, 400), (300, 200))]
    // Clip: Height only
    #[case::clip_height_only_invalid("?h=0&fit=clip", (600, 400), (600, 400))]
    #[case::clip_height_only("?h=100&fit=clip", (600, 400), (150, 100))]
    #[case::clip_height_only("?h=200&fit=clip", (600, 400), (300, 200))]
    // Clip: Width and Height
    #[case::clip_width_and_height_invalid("?w=0&h=0&fit=clip", (600, 400), (600, 400))]
    #[case::clip_width_and_height("?w=300&h=200&fit=clip", (600, 400), (300, 200))]
    #[case::clip_width_and_height("?w=100&h=100&fit=clip", (600, 400), (100, 67))]
    // Crop: Width only
    #[case::crop_width_only_invalid("?w=0&fit=crop", (600, 400), (600, 400))]
    #[case::crop_width_only("?w=150&fit=crop", (600, 400), (150, 100))]
    #[case::crop_width_only("?w=300&fit=crop", (600, 400), (300, 200))]
    // Crop: Height only
    #[case::crop_height_only_invalid("?h=0&fit=crop", (600, 400), (600, 400))]
    #[case::crop_height_only("?h=100&fit=crop", (600, 400), (150, 100))]
    #[case::crop_height_only("?h=200&fit=crop", (600, 400), (300, 200))]
    // Crop: Width and Height
    #[case::crop_width_and_height_invalid("?w=0&h=0&fit=crop", (600, 400), (600, 400))]
    #[case::crop_width_and_height("?w=300&h=200&fit=crop", (600, 400), (300, 200))]
    #[case::crop_width_and_height("?w=100&h=100&fit=crop", (600, 400), (100, 100))]
    // Max: Width only
    #[case::max_width_only_invalid("?w=0&fit=max", (600, 400), (600, 400))]
    #[case::max_width_only("?w=150&fit=max", (600, 400), (150, 100))]
    #[case::max_width_only("?w=300&fit=max", (600, 400), (300, 200))]
    // Max: Height only
    #[case::max_height_only_invalid("?h=0&fit=max", (600, 400), (600, 400))]
    #[case::max_height_only("?h=100&fit=max", (600, 400), (150, 100))]
    #[case::max_height_only("?h=200&fit=max", (600, 400), (300, 200))]
    // Max: Width and Height
    #[case::max_width_and_height_invalid("?w=0&h=0&fit=max", (600, 400), (600, 400))]
    #[case::max_width_and_height("?w=300&h=200&fit=max", (600, 400), (300, 200))]
    #[case::max_width_and_height("?w=100&h=100&fit=max", (600, 400), (100, 67))]
    fn test_calculate_dimensions(
        #[case] query: &str,
        #[case] image_dimensions: (i32, i32),
        #[case] expected: (i32, i32),
    ) {
        let url = String::from("https://google.com/image.jpg") + query;
        let uri: Uri = url.parse().expect("failed to parse url");
        let mut image_options: Query<ImageOptions> =
            Query::try_from_uri(&uri).expect("failed to parse query");
        let (width, height) = image_dimensions;
        calculate_dimensions(&mut image_options, width, height);
        assert_eq!(
            (image_options.width.unwrap(), image_options.height.unwrap()),
            expected
        );
    }

    fn get_image_options() -> ImageOptions {
        ImageOptions {
            width: Some(300),
            height: Some(200),
            quality: Some(80),
            aspect_ratio: Some(AspectRatio::new(16, 9)),
            device_pixel_ratio: Some(2),
            fit: Some(Fit::Crop),
            background: Some(Colour { r: 255, g: 0, b: 0 }),
            format: Some(ImageFormat::Jpeg),
            download: Some("image.jpg".to_string()),
            trim: Some(Trim::Auto),
            trim_colour: Some(Colour { r: 0, g: 255, b: 0 }),
            sharpen: Some(Percentage(50)),
            kodachrome: Some(Percentage(50)),
            ..Default::default()
        }
    }

    #[test]
    fn test_query_str_generation() {
        let query_str = get_image_options().query_str();
        assert_eq!(
            query_str,
            "ar=16:9&background=ff0000&download=image.jpg&dpr=2&fit=crop&format=jpeg&height=200&kodachrome=50&quality=80&sharpen=50&trim=auto&trim-colour=00ff00&width=300"
        );
    }

    #[test]
    fn test_signing() {
        let secret = "super_secret_key";
        let signature = get_image_options().sign(secret);

        assert_eq!(
            signature,
            "210868675de768f0320ad506c85580bf686ab2feec6a35542e93c378e078e28a"
        );
    }
}
