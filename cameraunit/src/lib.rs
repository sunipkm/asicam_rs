use std::{fmt::Display, time::Duration};
use std::any::Any;
use imagedata::{ImageData, ImageMetaData};

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

pub trait CameraUnit {
    fn get_vendor(&self) -> &str
    {
        "Unknown"
    }

    fn get_handle(&self) -> Option<&dyn Any>
    {
        None
    }

    fn get_uuid(&self) -> Option<&str>
    {
        None
    }

    fn capture_image(&self) -> Result<ImageData, String>
    {
        Err("Not implemented".to_string())
    }

    fn cancel_capture(&self) -> Result<(), String>
    {
        Err("Not implemented".to_string())
    }

    fn is_capturing(&self) -> bool
    {
        false
    }

    fn get_last_image(&self) -> Option<ImageData>
    {
        None
    }

    fn camera_ready(&self) -> bool
    {
        false
    }

    fn camera_name(&self) -> &str
    {
        "Unknown"
    }

    fn set_exposure(&self, exposure: Duration) -> Result<Duration, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_exposure(&self) -> Duration
    {
        Duration::from_secs(0)
    }
    
    fn get_gain(&self) -> f32
    {
        0.0
    }

    fn get_gain_raw(&self) -> i64
    {
        0
    }

    fn set_gain(&self, gain: f32) -> Result<f32, String>
    {
        Err("Not implemented".to_string())
    }

    fn set_gain_raw(&self, gain: i64) -> Result<i64, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_offset(&self) -> i32
    {
        0
    }

    fn set_offset(&self, offset: i32) -> Result<i32, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_min_exposure(&self) -> Result<Duration, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_max_exposure(&self) -> Result<Duration, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_min_gain(&self) -> Result<i64, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_max_gain(&self) -> Result<i64, String>
    {
        Err("Not implemented".to_string())
    }

    fn set_shutter_open(&self, open: bool) -> Result<bool, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_shutter_open(&self) -> Result<bool, String>
    {
        Err("Not implemented".to_string())
    }

    fn set_temperature(&self, temperature: f32) -> Result<f32, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_temperature(&self) -> Option<f32>
    {
        None
    }

    fn get_cooler_power(&self) -> Option<f32>
    {
        None
    }

    fn set_cooler_power(&self, power: f32) -> Result<f32, String>
    {
        Err("Not implemented".to_string())
    }

    fn set_roi(&self, roi: ROI) -> Result<ROI, String>
    {
        Err("Not implemented".to_string())
    }

    fn get_bin_x(&self) -> i32
    {
        1
    }

    fn get_bin_y(&self) -> i32
    {
        1
    }

    fn get_roi(&self) -> &ROI
    {
        &ROI {
            x_min: 0,
            x_max: 0,
            y_min: 0,
            y_max: 0,
            bin_x: 1,
            bin_y: 1,
        }
    }

    fn get_status(&self) -> String
    {
        "Not implemented".to_string()
    }

    fn get_ccd_width(&self) -> u32
    {
        0
    }

    fn get_ccd_height(&self) -> u32
    {
        0
    }

    fn get_pixel_size(&self) -> Option<f32>
    {
        None
    }
}