use imagedata::ImageData;
use std::any::Any;
use std::{fmt::Display, time::Duration};

#[derive(Clone, Copy)]
pub struct ROI {
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
    pub bin_x: i32,
    pub bin_y: i32,
}

impl Display for ROI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(ROI: x_min = {}, x_max = {}, y_min: {}, y_max = {}, bin_x = {}, bin_y = {})",
            self.x_min, self.x_max, self.y_min, self.y_max, self.bin_x, self.bin_y
        )
    }
}

pub trait CameraInfo {
    fn camera_ready(&self) -> bool {
        false
    }

    fn camera_name(&self) -> &str {
        "Unknown"
    }

    fn get_uuid(&self) -> Option<String> {
        None
    }

    fn is_capturing(&self) -> bool {
        false
    }

    fn set_temperature(&self, _temperature: f32) -> Result<f32, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_temperature(&self) -> Option<f32> {
        None
    }

    fn get_cooler_power(&self) -> Option<f32> {
        None
    }

    fn set_cooler_power(&self, _power: f32) -> Result<f32, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_ccd_width(&self) -> u32 {
        0
    }

    fn get_ccd_height(&self) -> u32 {
        0
    }

    fn get_pixel_size(&self) -> Option<f32> {
        None
    }
}

pub trait CameraUnit : CameraInfo {
    fn get_vendor(&self) -> &str {
        "Unknown"
    }

    fn get_handle(&self) -> Option<&dyn Any> {
        None
    }

    fn capture_image(&self) -> Result<ImageData, Error> {
        Err(Error::GeneralError("Not implemented".to_string()))
    }

    fn cancel_capture(&self) -> Result<(), Error> {
        Err(Error::GeneralError("Not implemented".to_string()))
    }

    fn get_last_image(&self) -> Option<ImageData> {
        None
    }

    fn set_exposure(&mut self, _exposure: Duration) -> Result<Duration, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_exposure(&self) -> Duration {
        Duration::from_secs(0)
    }

    fn get_gain(&self) -> f32 {
        0.0
    }

    fn get_gain_raw(&self) -> i64 {
        0
    }

    fn set_gain(&mut self, _gain: f32) -> Result<f32, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn set_gain_raw(&mut self, _gain: i64) -> Result<i64, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_offset(&self) -> i32 {
        0
    }

    fn set_offset(&mut self, _offset: i32) -> Result<i32, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_min_exposure(&self) -> Result<Duration, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_max_exposure(&self) -> Result<Duration, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_min_gain(&self) -> Result<i64, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_max_gain(&self) -> Result<i64, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn set_shutter_open(&mut self, _open: bool) -> Result<bool, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_shutter_open(&self) -> Result<bool, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn set_roi(&mut self, _roi: &ROI) -> Result<&ROI, Error> {
        Err(Error::Message("Not implemented".to_string()))
    }

    fn get_bin_x(&self) -> i32 {
        1
    }

    fn get_bin_y(&self) -> i32 {
        1
    }

    fn get_roi(&self) -> &ROI {
        &ROI {
            x_min: 0,
            x_max: 0,
            y_min: 0,
            y_max: 0,
            bin_x: 1,
            bin_y: 1,
        }
    }

    fn get_status(&self) -> String {
        "Not implemented".to_string()
    }
}

#[derive(Debug)]
pub enum Error {
    Message(String),
    InvalidIndex(i32),
    InvalidId(i32),
    InvalidControlType(String),
    NoCamerasAvailable,
    CameraClosed,
    CameraRemoved,
    InvalidPath(String),
    InvalidFormat(String),
    InvalidSize(usize),
    InvalidImageType(String),
    TimedOut,
    InvalidSequence,
    BufferTooSmall(usize),
    ExposureInProgress,
    GeneralError(String),
    InvalidMode(String),
    ExposureFailed(String),
    InvalidValue(String),
    OutOfBounds(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Error::Message(msg) => msg.clone(),
            Error::InvalidIndex(idx) => format!("Invalid index: {}", idx),
            Error::InvalidId(id) => format!("Invalid id: {}", id),
            Error::InvalidControlType(t) => format!("Invalid control type: {}", t),
            Error::NoCamerasAvailable => "No cameras available".to_string(),
            Error::CameraClosed => "Camera closed".to_string(),
            Error::CameraRemoved => "Camera removed".to_string(),
            Error::InvalidPath(p) => format!("Invalid path: {}", p),
            Error::InvalidFormat(f) => format!("Invalid format: {}", f),
            Error::InvalidSize(s) => format!("Invalid size: {}", s),
            Error::InvalidImageType(t) => format!("Invalid image type: {}", t),
            Error::TimedOut => "Timed out".to_string(),
            Error::InvalidSequence => "Invalid sequence".to_string(),
            Error::BufferTooSmall(s) => format!("Buffer too small: {}", s),
            Error::ExposureInProgress => "Exposure in progress".to_string(),
            Error::GeneralError(msg) => msg.clone(),
            Error::InvalidMode(msg) => msg.clone(),
            Error::ExposureFailed(msg) => msg.clone(),
            Error::InvalidValue(msg) => msg.clone(),
            Error::OutOfBounds(msg) => msg.clone(),
        };
        write!(f, "{}", msg)
    }
}