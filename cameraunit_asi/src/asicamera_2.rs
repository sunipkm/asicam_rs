#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[allow(dead_code, unused_imports)]
mod asicamera2_bindings;
use asicamera2_bindings::*;

use std::{
    ffi::{c_long, c_uchar, CStr},
    fmt::Display,
    mem::MaybeUninit,
    sync::{Arc, Mutex},
    thread::sleep,
    time::{Duration, SystemTime},
};

use cameraunit::{CameraInfo, CameraUnit, Error, ROI};
use image::DynamicImage;
use imagedata::{ImageData, ImageMetaData};
use log::{info, warn};

pub struct CameraUnit_ASI {
    id: Arc<ASICamId>,
    capturing: Arc<Mutex<bool>>,
    props: Box<ASICameraProps>,
    // control_caps: Vec<ASIControlCaps>,
    gain_min: i64,
    gain_max: i64,
    exp_min: Duration,
    exp_max: Duration,
    exposure: Duration,
    is_dark_frame: bool,
    image_fmt: ASIImageFormat,
    roi: ROI,
}

#[derive(Clone)]
pub struct CameraInfo_ASI {
    id: Arc<ASICamId>,
    capturing: Arc<Mutex<bool>>,
    name: String,
    uuid: [u8; 8],
    height: u32,
    width: u32,
    psize: f64,
    is_cooler_cam: bool,
}

#[derive(Clone)]
pub struct ASICameraProps {
    name: String,
    id: i32,
    uuid: [u8; 8],
    max_height: i64,
    max_width: i64,
    is_color_cam: bool,
    bayer_pattern: Option<ASIBayerPattern>,
    supported_bins: Vec<i32>,
    supported_formats: Vec<ASIImageFormat>,
    pixel_size: f64,
    mechanical_shutter: bool,
    is_cooler_cam: bool,
    is_usb3_camera: bool,
    e_per_adu: f32,
    bit_depth: i32,
    is_trigger_camera: bool,
}

pub fn num_cameras() -> i32 {
    unsafe { ASIGetNumOfConnectedCameras() }
}

pub fn get_camera_ids() -> Option<Vec<i32>> {
    let num_cameras = num_cameras();
    if num_cameras > 0 {
        let mut ids: Vec<i32> = Vec::with_capacity(num_cameras as usize);
        for i in 0..num_cameras {
            let info = MaybeUninit::<ASI_CAMERA_INFO>::zeroed();
            unsafe {
                let mut info = info.assume_init();
                let res = ASIGetCameraProperty(&mut info, i);
                if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_INDEX as i32 {
                    continue;
                }
                ids.push(info.CameraID);
            }
        }
        if ids.len() == 0 {
            return None;
        }
        Some(ids)
    } else {
        None
    }
}

pub fn open_camera(id: i32) -> Result<(CameraUnit_ASI, CameraInfo_ASI), Error> {
    if let Some(cam_ids) = get_camera_ids() {
        if !cam_ids.contains(&id) {
            return Err(Error::InvalidId(id));
        }
        let info = MaybeUninit::<ASI_CAMERA_INFO>::zeroed();
        let mut info = unsafe { info.assume_init() };
        let res = unsafe { ASIGetCameraPropertyByID(id, &mut info) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }

        let mut prop = ASICameraProps {
            name: String::from_utf8_lossy(&unsafe {
                std::mem::transmute_copy::<[i8; 64], [u8; 64]>(&info.Name)
            })
            .to_string(),
            id: info.CameraID,
            uuid: [0; 8],
            max_height: info.MaxHeight,
            max_width: info.MaxWidth,
            is_color_cam: info.IsColorCam == ASI_BOOL_ASI_TRUE,
            bayer_pattern: if info.IsColorCam == ASI_BOOL_ASI_TRUE {
                ASIBayerPattern::from_u32(info.BayerPattern)
            } else {
                None
            },
            supported_bins: {
                let mut bins: Vec<i32> = Vec::new();
                for x in info.SupportedBins.iter() {
                    if *x != 0 {
                        bins.push(*x);
                    } else {
                        break;
                    }
                }
                bins
            },
            supported_formats: {
                let mut formats: Vec<ASIImageFormat> = Vec::new();
                for x in info.SupportedVideoFormat.iter() {
                    if *x != 0 {
                        formats.push(ASIImageFormat::from_u32(*x as u32).unwrap());
                    } else {
                        break;
                    }
                }
                formats
            },
            pixel_size: info.PixelSize,
            mechanical_shutter: info.MechanicalShutter == ASI_BOOL_ASI_TRUE,
            is_cooler_cam: info.IsCoolerCam == ASI_BOOL_ASI_TRUE,
            is_usb3_camera: info.IsUSB3Host == ASI_BOOL_ASI_TRUE,
            e_per_adu: info.ElecPerADU,
            bit_depth: info.BitDepth,
            is_trigger_camera: info.IsTriggerCam == ASI_BOOL_ASI_TRUE,
        };

        if prop.is_usb3_camera {
            let cid = MaybeUninit::<ASI_ID>::zeroed();
            let mut cid = unsafe { cid.assume_init() };
            let res = unsafe { ASIGetID(id, &mut cid) };
            if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                return Err(Error::InvalidId(id));
            } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                return Err(Error::CameraClosed);
            }
            prop.uuid = cid.id;
        }

        let res = unsafe { ASIInitCamera(prop.id) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(prop.id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }

        let ccaps = get_control_caps(prop.id)?;

        let (gain_min, gain_max) = get_gain_minmax(&ccaps);
        let (exp_min, exp_max) = get_exposure_minmax(&ccaps);

        let cobj = CameraUnit_ASI {
            id: Arc::new(ASICamId(prop.id)),
            capturing: Arc::new(Mutex::new(false)),
            props: Box::new(prop.clone()),
            // control_caps: ccaps,
            gain_min: gain_min,
            gain_max: gain_max,
            exp_min: exp_min,
            exp_max: exp_max,
            exposure: Duration::from_millis(100),
            is_dark_frame: false,
            image_fmt: {
                if prop.is_color_cam {
                    ASIImageFormat::Image_RGB24
                } else if prop
                    .supported_formats
                    .contains(&ASIImageFormat::Image_RAW16)
                {
                    ASIImageFormat::Image_RAW16
                } else {
                    ASIImageFormat::Image_RAW8
                }
            },
            roi: ROI {
                x_min: 0,
                x_max: prop.max_width as i32,
                y_min: 0,
                y_max: prop.max_height as i32,
                bin_x: 1,
                bin_y: 1,
            },
        };

        cobj.set_start_pos(0, 0)?;
        cobj.set_roi_format(&ASIRoiMode {
            width: cobj.roi.x_max - cobj.roi.x_min,
            height: cobj.roi.y_max - cobj.roi.y_min,
            bin: cobj.roi.bin_x,
            fmt: cobj.image_fmt,
        })?;

        let cinfo = CameraInfo_ASI {
            id: cobj.id.clone(),
            capturing: cobj.capturing.clone(),
            name: prop.name.clone(),
            uuid: prop.uuid,
            height: prop.max_height as u32,
            width: prop.max_width as u32,
            psize: prop.pixel_size,
            is_cooler_cam: prop.is_cooler_cam,
        };

        return Ok((cobj, cinfo));
    } else {
        return Err(Error::NoCamerasAvailable);
    }
}

pub fn open_camera_by_index(idx: usize) -> Result<(CameraUnit_ASI, CameraInfo_ASI), Error> {
    let ids = get_camera_ids();
    if let Some(ids) = ids {
        if idx >= ids.len() {
            return Err(Error::InvalidIndex(idx as i32));
        }
        return open_camera(ids[idx]);
    } else {
        return Err(Error::NoCamerasAvailable);
    }
}

impl CameraUnit_ASI {
    pub fn set_uuid(&mut self, uuid: &[u8; 8]) -> Result<(), Error> {
        if self.props.uuid == *uuid {
            Ok(())
        } else {
            let cid = ASI_ID { id: *uuid };
            let res = unsafe { ASISetID(self.id.0, cid) };
            if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                return Err(Error::InvalidId(self.id.0));
            } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                return Err(Error::CameraClosed);
            }
            self.props.uuid = *uuid;
            Ok(())
        }
    }

    pub fn get_sdk_version() -> String {
        let c_buf = unsafe { ASIGetSDKVersion() };
        let c_str: &CStr = unsafe { CStr::from_ptr(c_buf) };
        let str_slice: &str = c_str.to_str().unwrap();
        str_slice.to_owned()
    }

    pub fn get_serial(&self) -> Result<u64, Error> {
        let ser = MaybeUninit::<ASI_SN>::zeroed();
        let mut ser = unsafe { ser.assume_init() };
        let res = unsafe { ASIGetSerialNumber(self.id.0, &mut ser) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id.0));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_GENERAL_ERROR as i32 {
            return Err(Error::GeneralError(
                "Camera does not have serial number.".to_owned(),
            ));
        }
        let ser = u64::from_be_bytes(ser.id);
        Ok(ser)
    }

    pub fn get_image_fmt(&self) -> ASIImageFormat {
        self.image_fmt
    }

    pub fn set_image_fmt(&mut self, fmt: ASIImageFormat) -> Result<(), Error> {
        if self.image_fmt == fmt {
            return Ok(());
        }
        if !self.props.supported_formats.contains(&fmt) {
            return Err(Error::InvalidMode(format!(
                "Format {:?} not supported by camera",
                fmt
            )));
        }
        if self.is_capturing() {
            return Err(Error::ExposureInProgress);
        }
        let mut roi = self.get_roi_format()?;
        roi.fmt = fmt;
        self.set_roi_format(&roi)?;
        self.image_fmt = fmt;
        Ok(())
    }

    pub fn get_props(&self) -> &ASICameraProps {
        &self.props
    }

    fn get_roi_format(&self) -> Result<ASIRoiMode, Error> {
        let mut roi = ASIRoiMode {
            width: 0,
            height: 0,
            bin: 0,
            fmt: ASIImageFormat::Image_RAW8,
        };
        let mut fmt: i32 = 0;
        let res = unsafe {
            ASIGetROIFormat(
                self.id.0,
                &mut roi.width,
                &mut roi.height,
                &mut roi.bin,
                &mut fmt,
            )
        };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id.0));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        if let Some(fmt) = ASIImageFormat::from_u32(fmt as u32) {
            roi.fmt = fmt;
            return Ok(roi);
        } else {
            return Err(Error::InvalidMode(format!("Invalid image format: {}", fmt)));
        }
    }

    fn set_roi_format(&self, roi: &ASIRoiMode) -> Result<(), Error> {
        let res =
            unsafe { ASISetROIFormat(self.id.0, roi.width, roi.height, roi.bin, roi.fmt as i32) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id.0));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        Ok(())
    }

    fn set_start_pos(&self, x: i32, y: i32) -> Result<(), Error> {
        if x < 0 || y < 0 {
            return Err(Error::InvalidValue(format!(
                "Invalid start position: {}, {}",
                x, y
            )));
        }
        let res = unsafe { ASISetStartPos(self.id.0, x, y) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id.0));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        } else if res == ASI_ERROR_CODE_ASI_ERROR_OUTOF_BOUNDARY as i32 {
            return Err(Error::OutOfBounds(format!(
                "Could not set start position to {}, {}",
                x, y
            )));
        }
        Ok(())
    }

    fn get_start_pos(&self) -> Result<(i32, i32), Error> {
        let mut x: i32 = 0;
        let mut y: i32 = 0;
        let res = unsafe { ASIGetStartPos(self.id.0, &mut x, &mut y) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id.0));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        Ok((x, y))
    }

    fn get_exposure_status(&self) -> Result<ASIExposureStatus, Error> {
        let stat = MaybeUninit::<ASI_EXPOSURE_STATUS>::zeroed();
        let mut stat = unsafe { stat.assume_init() };
        let res = unsafe { ASIGetExpStatus(self.id.0, &mut stat) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id.0));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        ASIExposureStatus::from_u32(stat)
    }
}

impl CameraInfo for CameraInfo_ASI {
    fn camera_name(&self) -> &str {
        &self.name
    }

    fn get_uuid(&self) -> Option<String> {
        Some(String::from_utf8_lossy(&self.uuid).to_string())
    }

    fn get_ccd_height(&self) -> u32 {
        self.height
    }

    fn get_ccd_width(&self) -> u32 {
        self.width
    }

    fn get_pixel_size(&self) -> Option<f32> {
        Some(self.psize as f32)
    }

    fn camera_ready(&self) -> bool {
        true
    }

    fn is_capturing(&self) -> bool {
        let res = self.capturing.try_lock();
        match res {
            Ok(capturing) => *capturing,
            Err(_) => true,
        }
    }

    fn get_cooler_power(&self) -> Option<f32> {
        get_cooler_power(self.id.0)
    }

    fn get_temperature(&self) -> Option<f32> {
        get_temperature(self.id.0)
    }

    fn set_temperature(&self, temperature: f32) -> Result<f32, Error> {
        set_temperature(self.id.0, temperature, self.is_cooler_cam)
    }
}

impl CameraInfo for CameraUnit_ASI {
    fn camera_name(&self) -> &str {
        &self.props.name
    }

    fn get_uuid(&self) -> Option<String> {
        Some(String::from_utf8_lossy(&self.props.uuid).to_string())
    }

    fn get_ccd_height(&self) -> u32 {
        self.props.max_height as u32
    }

    fn get_ccd_width(&self) -> u32 {
        self.props.max_width as u32
    }

    fn get_pixel_size(&self) -> Option<f32> {
        Some(self.props.pixel_size as f32)
    }

    fn camera_ready(&self) -> bool {
        true
    }

    fn is_capturing(&self) -> bool {
        let res = self.capturing.try_lock();
        match res {
            Ok(capturing) => *capturing,
            Err(_) => true,
        }
    }

    fn get_cooler_power(&self) -> Option<f32> {
        get_cooler_power(self.id.0)
    }

    fn get_temperature(&self) -> Option<f32> {
        get_temperature(self.id.0)
    }

    fn set_temperature(&self, temperature: f32) -> Result<f32, Error> {
        set_temperature(self.id.0, temperature, self.props.is_cooler_cam)
    }
}

impl CameraUnit for CameraUnit_ASI {
    fn get_vendor(&self) -> &str {
        "ZWO"
    }

    fn get_handle(&self) -> Option<&dyn std::any::Any> {
        Some(&self.id.0)
    }

    fn cancel_capture(&self) -> Result<(), Error> {
        let mut capturing = self.capturing.lock().unwrap();
        if *capturing {
            return Ok(());
        }
        sys_cancel_capture(self.id.0)?;
        *capturing = false;
        Ok(())
    }

    fn get_min_exposure(&self) -> Result<Duration, Error> {
        Ok(self.exp_min)
    }

    fn get_max_exposure(&self) -> Result<Duration, Error> {
        Ok(self.exp_max)
    }

    fn get_min_gain(&self) -> Result<i64, Error> {
        Ok(self.gain_min)
    }

    fn get_max_gain(&self) -> Result<i64, Error> {
        Ok(self.gain_max)
    }

    fn capture_image(&self) -> Result<ImageData, Error> {
        let start_time: SystemTime;
        let roi: ASIRoiMode;
        {
            let mut capturing = self.capturing.lock().unwrap();
            let stat = self.get_exposure_status()?;
            if stat == ASIExposureStatus::Working {
                *capturing = true;
                return Err(Error::ExposureInProgress);
            } else if stat == ASIExposureStatus::Failed {
                return Err(Error::ExposureFailed("Unknown".to_owned()));
            }
            *capturing = false;
            roi = self.get_roi_format()?;
            *capturing = true;
            start_time = SystemTime::now();
            let res = unsafe {
                ASIStartExposure(
                    self.id.0,
                    if self.is_dark_frame {
                        ASI_BOOL_ASI_TRUE as i32
                    } else {
                        ASI_BOOL_ASI_TRUE as i32
                    },
                )
            };
            if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                *capturing = false;
                return Err(Error::InvalidId(self.id.0));
            } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                *capturing = false;
                return Err(Error::CameraClosed);
            } else if res == ASI_ERROR_CODE_ASI_ERROR_VIDEO_MODE_ACTIVE as i32 {
                *capturing = true;
                return Err(Error::GeneralError("Video mode active".to_owned()));
            }
        }
        let mut stat: ASIExposureStatus;
        if self.exposure < Duration::from_millis(16) {
            loop {
                stat = self.get_exposure_status()?;
                if stat != ASIExposureStatus::Working {
                    break;
                }
                sleep(Duration::from_millis(1));
            }
        } else if self.exposure < Duration::from_secs(1) {
            loop {
                stat = self.get_exposure_status()?;
                if stat != ASIExposureStatus::Working {
                    break;
                }
                sleep(Duration::from_millis(100));
            }
        } else {
            loop {
                stat = self.get_exposure_status()?;
                if stat != ASIExposureStatus::Working {
                    break;
                }
                sleep(Duration::from_secs(1));
            }
        }
        let mut capturing = self.capturing.lock().unwrap(); // we are not dropping this until we return, so no problem reading exposure or roi
        if stat == ASIExposureStatus::Failed {
            *capturing = false;
            return Err(Error::ExposureFailed("Unknown".to_owned()));
        } else if stat == ASIExposureStatus::Idle {
            *capturing = false;
            return Err(Error::ExposureFailed(
                "Successful exposure but no available data".to_owned(),
            ));
        } else if stat == ASIExposureStatus::Working {
            sys_cancel_capture(self.id.0)?;
            *capturing = false;
            return Err(Error::ExposureFailed("Exposure timed out".to_owned()));
        } else {
            let img = match roi.fmt {
                ASIImageFormat::Image_RAW8 => {
                    let mut data = vec![0u8; (roi.width * roi.height) as usize];
                    let res = unsafe {
                        ASIGetDataAfterExp(
                            self.id.0,
                            data.as_mut_ptr() as *mut c_uchar,
                            (roi.width * roi.height) as c_long,
                        )
                    };
                    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                        return Err(Error::InvalidId(self.id.0));
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                        return Err(Error::CameraClosed);
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_TIMEOUT as i32 {
                        return Err(Error::TimedOut);
                    }
                    *capturing = false; // whether the call succeeds or fails, we are not capturing anymore
                    let mut img =
                        DynamicImage::new_luma8(roi.width as u32, roi.height as u32).into_luma8();
                    img.copy_from_slice(&data);
                    DynamicImage::from(img)
                }
                ASIImageFormat::Image_RAW16 => {
                    let mut data = vec![0u16; (roi.width * roi.height) as usize];
                    let res = unsafe {
                        ASIGetDataAfterExp(
                            self.id.0,
                            data.as_mut_ptr() as *mut c_uchar,
                            (roi.width * roi.height * 2) as c_long,
                        )
                    };
                    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                        return Err(Error::InvalidId(self.id.0));
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                        return Err(Error::CameraClosed);
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_TIMEOUT as i32 {
                        return Err(Error::TimedOut);
                    }
                    *capturing = false; // whether the call succeeds or fails, we are not capturing anymore
                    let mut img =
                        DynamicImage::new_luma16(roi.width as u32, roi.height as u32).into_luma16();
                    img.copy_from_slice(&data);
                    DynamicImage::from(img)
                }
                ASIImageFormat::Image_RGB24 => {
                    let mut data = vec![0u8; (roi.width * roi.height * 3) as usize];
                    let res = unsafe {
                        ASIGetDataAfterExp(
                            self.id.0,
                            data.as_mut_ptr() as *mut c_uchar,
                            (roi.width * roi.height * 3) as c_long,
                        )
                    };
                    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                        return Err(Error::InvalidId(self.id.0));
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                        return Err(Error::CameraClosed);
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_TIMEOUT as i32 {
                        return Err(Error::TimedOut);
                    }
                    *capturing = false; // whether the call succeeds or fails, we are not capturing anymore
                    let mut img =
                        DynamicImage::new_rgb8(roi.width as u32, roi.height as u32).into_rgb8();
                    img.copy_from_slice(&data);

                    DynamicImage::from(img)
                }
            };
            let mut meta = ImageMetaData::full_builder(
                self.get_bin_x() as u32,
                self.get_bin_y() as u32,
                self.roi.y_min as u32,
                self.roi.x_min as u32,
                self.get_temperature().unwrap_or(-273.0),
                self.exposure,
                start_time,
                self.camera_name(),
                self.get_gain_raw(),
                self.get_offset() as i64,
                self.get_min_gain().unwrap_or(0) as i32,
                self.get_max_gain().unwrap_or(0) as i32,
            );
            meta.add_extended_attrib(
                "DARK_FRAME",
                &format!(
                    "{}",
                    if !self.get_shutter_open().unwrap_or(false) {
                        "True"
                    } else {
                        "False"
                    }
                ),
            );

            return Ok(ImageData::new(img, meta));
        }
    }

    fn get_bin_x(&self) -> i32 {
        self.roi.bin_x
    }

    fn get_bin_y(&self) -> i32 {
        self.roi.bin_y
    }

    fn get_exposure(&self) -> Duration {
        self.exposure
    }

    fn get_gain(&self) -> f32 {
        let res = get_control_value(self.id.0, ASIControlType::Gain);
        if let Ok((val, _)) = res {
            return (val as f32 - self.gain_min as f32)
                / (self.gain_max as f32 - self.gain_min as f32);
        }
        0.0
    }

    fn get_roi(&self) -> &ROI {
        return &self.roi;
    }

    fn get_gain_raw(&self) -> i64 {
        let res = get_control_value(self.id.0, ASIControlType::Gain);
        if let Ok((val, _)) = res {
            return val;
        }
        0
    }

    fn get_offset(&self) -> i32 {
        let res = get_control_value(self.id.0, ASIControlType::Offset);
        if let Ok((val, _)) = res {
            return val as i32;
        }
        0
    }

    fn get_shutter_open(&self) -> Result<bool, Error> {
        if !self.props.mechanical_shutter {
            return Err(Error::InvalidControlType(
                "Camera does not have mechanical shutter".to_owned(),
            ));
        }
        Ok(!self.is_dark_frame)
    }

    fn set_exposure(&mut self, exposure: Duration) -> Result<Duration, Error> {
        if exposure < self.exp_min {
            return Err(Error::InvalidValue(format!(
                "Exposure {} us is below minimum of {} us",
                exposure.as_micros(),
                self.exp_min.as_micros()
            )));
        } else if exposure > self.exp_max {
            return Err(Error::InvalidValue(format!(
                "Exposure {} is above maximum of {}",
                exposure.as_secs_f32(),
                self.exp_max.as_secs_f32()
            )));
        }
        let capturing = self.capturing.lock().unwrap();
        if *capturing {
            return Err(Error::ExposureInProgress);
        }
        set_control_value(
            self.id.0,
            ASIControlType::Exposure,
            exposure.as_micros() as c_long,
            false,
        )?;
        let (exposure, _is_auto) = get_control_value(self.id.0, ASIControlType::Exposure)?;
        self.exposure = Duration::from_micros(exposure as u64);
        Ok(self.exposure)
    }

    fn set_gain(&mut self, gain: f32) -> Result<f32, Error> {
        if gain < 0.0 || gain > 100.0 {
            return Err(Error::InvalidValue(format!(
                "Gain {} is outside of range 0-100",
                gain
            )));
        }
        let gain =
            (gain * (self.gain_max as f32 - self.gain_min as f32) + self.gain_min as f32) as c_long;
        let gain = self.set_gain_raw(gain)?;
        Ok((gain as f32 - self.gain_min as f32) / (self.gain_max as f32 - self.gain_min as f32))
    }

    fn set_gain_raw(&mut self, gain: i64) -> Result<i64, Error> {
        if gain < self.gain_min {
            return Err(Error::InvalidValue(format!(
                "Gain {} is below minimum of {}",
                gain, self.gain_min
            )));
        } else if gain > self.gain_max {
            return Err(Error::InvalidValue(format!(
                "Gain {} is above maximum of {}",
                gain, self.gain_max
            )));
        }
        let capturing = self.capturing.lock().unwrap();
        if *capturing {
            return Err(Error::ExposureInProgress);
        }
        set_control_value(self.id.0, ASIControlType::Gain, gain as c_long, false)?;
        Ok(self.get_gain_raw())
    }

    fn set_roi(&mut self, roi: &ROI) -> Result<&ROI, Error> {
        if roi.bin_x != roi.bin_y {
            return Err(Error::InvalidValue(
                "Bin X and Bin Y must be equal".to_owned(),
            ));
        }

        if roi.bin_x < 1 {
            return Err(Error::InvalidValue(format!(
                "Bin {} is below minimum of 1",
                roi.bin_x
            )));
        }

        if !self.props.supported_bins.contains(&roi.bin_x) {
            return Err(Error::InvalidValue(format!(
                "Bin {} is not supported by camera",
                roi.bin_x
            )));
        }

        let mut roi = *roi;
        roi.bin_y = roi.bin_x;

        if roi.x_max <= 0 {
            roi.x_max = self.props.max_width as i32;
        }
        if roi.y_max <= 0 {
            roi.y_max = self.props.max_height as i32;
        }

        roi.x_min /= roi.bin_x;
        roi.x_max /= roi.bin_x;
        roi.y_min /= roi.bin_y;
        roi.y_max /= roi.bin_y;

        let mut width = roi.x_max - roi.x_min;
        width -= width % 8;
        let mut height = roi.y_max - roi.y_min;
        height -= height % 2;

        roi.x_max = roi.x_min + width;
        roi.y_max = roi.y_min + height;

        if width < 0 || height < 0 {
            return Err(Error::InvalidValue(
                "ROI width and height must be positive".to_owned(),
            ));
        }

        if roi.x_max > self.props.max_width as i32 / roi.bin_x {
            return Err(Error::OutOfBounds(
                "ROI x_max is greater than max width".to_owned(),
            ));
        }

        if roi.y_max > self.props.max_height as i32 / roi.bin_y {
            return Err(Error::OutOfBounds(
                "ROI y_max is greater than max height".to_owned(),
            ));
        }

        if !self.props.is_usb3_camera && self.camera_name().contains("ASI120") {
            if width * height % 1024 != 0 {
                return Err(Error::InvalidValue(
                    "ASI120 cameras require ROI width * height to be a multiple of 1024".to_owned(),
                ));
            }
        }

        let capturing = self.capturing.lock().unwrap();
        if *capturing {
            return Err(Error::ExposureInProgress);
        }

        let mut roi_md = self.get_roi_format()?;
        let (xs, ys) = self.get_start_pos()?;
        let roi_md_old = roi_md.clone();

        info!(
            "Current ROI: {} x {}, Bin: {}, Format: {:#?}",
            roi_md.width, roi_md.height, roi_md.bin, roi_md.fmt
        );
        info!("New ROI: {} x {}, Bin: {}", width, height, roi.bin_x);

        roi_md.width = width;
        roi_md.height = height;
        roi_md.bin = roi.bin_x;

        self.set_roi_format(&roi_md)?;

        if self.set_start_pos(roi.x_min, roi.y_min).is_err() {
            self.set_roi_format(&roi_md_old)?;
        }
        self.roi = roi;
        Ok(&self.roi)
    }

    fn set_shutter_open(&mut self, open: bool) -> Result<bool, Error> {
        let capturing = self.capturing.lock().unwrap();
        if *capturing {
            return Err(Error::ExposureInProgress);
        }
        if !self.props.mechanical_shutter {
            return Err(Error::InvalidControlType(
                "Camera does not have mechanical shutter".to_owned(),
            ));
        }
        self.is_dark_frame = !open;
        Ok(open)
    }
}

impl Default for ASIControlCaps {
    fn default() -> Self {
        ASIControlCaps {
            id: ASIControlType::Gain,
            name: [0; 64],
            description: [0; 128],
            min_value: 0,
            max_value: 0,
            default_value: 0,
            is_auto_supported: false,
            is_writable: false,
        }
    }
}

impl Display for ASIControlCaps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Control: {} - {:#?})\n
            Description: {}\n
            \tRange: {} - {}\n
            \tDefault: {}\n
            \tAuto: {}, Writable: {}\n
            ",
            String::from_utf8_lossy(&self.name),
            self.id,
            String::from_utf8_lossy(&self.description),
            self.min_value,
            self.max_value,
            self.default_value,
            self.is_auto_supported,
            self.is_writable,
        )
    }
}

impl ASIExposureStatus {
    fn from_u32(val: u32) -> Result<Self, Error> {
        match val {
            ASI_EXPOSURE_STATUS_ASI_EXP_IDLE => Ok(ASIExposureStatus::Idle),
            ASI_EXPOSURE_STATUS_ASI_EXP_WORKING => Ok(ASIExposureStatus::Working),
            ASI_EXPOSURE_STATUS_ASI_EXP_SUCCESS => Ok(ASIExposureStatus::Success),
            ASI_EXPOSURE_STATUS_ASI_EXP_FAILED => Ok(ASIExposureStatus::Failed),
            _ => Err(Error::InvalidMode(format!(
                "Invalid exposure status: {}",
                val
            ))),
        }
    }
}

impl Display for ASIExposureStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ASIExposureStatus::Idle => write!(f, "Idle"),
            ASIExposureStatus::Working => write!(f, "Working"),
            ASIExposureStatus::Success => write!(f, "Success"),
            ASIExposureStatus::Failed => write!(f, "Failed"),
        }
    }
}

impl Display for ASICameraProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Camera {}\n
            \tID: {} UUID: {}\n
            \tDetector: {} x {}\n
            \tColor: {}, Shutter: {}, Cooler: {}, USB3: {}, Trigger: {}\n
            \tBayer Pattern: {:#?}\n
            \tBins: {:#?}\n
            \tPixel Size: {} um, e/ADU: {}, Bit Depth: {}\n
            ",
            self.name,
            self.id,
            String::from_utf8_lossy(&self.uuid),
            self.max_width,
            self.max_height,
            self.is_color_cam,
            self.mechanical_shutter,
            self.is_cooler_cam,
            self.is_usb3_camera,
            self.is_trigger_camera,
            self.bayer_pattern,
            self.supported_bins,
            self.pixel_size,
            self.bit_depth,
            self.e_per_adu,
        )
    }
}

impl ASIControlType {
    fn from_u32(val: u32) -> Option<Self> {
        match val {
            ASI_CONTROL_TYPE_ASI_GAIN => Some(ASIControlType::Gain),
            ASI_CONTROL_TYPE_ASI_EXPOSURE => Some(ASIControlType::Exposure),
            ASI_CONTROL_TYPE_ASI_GAMMA => Some(ASIControlType::Gamma),
            ASI_CONTROL_TYPE_ASI_WB_R => Some(ASIControlType::WhiteBal_R),
            ASI_CONTROL_TYPE_ASI_WB_B => Some(ASIControlType::WhiteBal_B),
            ASI_CONTROL_TYPE_ASI_OFFSET => Some(ASIControlType::Offset),
            ASI_CONTROL_TYPE_ASI_BANDWIDTHOVERLOAD => Some(ASIControlType::BWOvld),
            ASI_CONTROL_TYPE_ASI_OVERCLOCK => Some(ASIControlType::Overclock),
            ASI_CONTROL_TYPE_ASI_TEMPERATURE => Some(ASIControlType::Temperature),
            ASI_CONTROL_TYPE_ASI_FLIP => Some(ASIControlType::Flip),
            ASI_CONTROL_TYPE_ASI_AUTO_MAX_GAIN => Some(ASIControlType::AutoExpMaxGain),
            ASI_CONTROL_TYPE_ASI_AUTO_MAX_EXP => Some(ASIControlType::AutoExpMaxExp),
            ASI_CONTROL_TYPE_ASI_AUTO_TARGET_BRIGHTNESS => {
                Some(ASIControlType::AutoExpTgtBrightness)
            }
            ASI_CONTROL_TYPE_ASI_HARDWARE_BIN => Some(ASIControlType::HWBin),
            ASI_CONTROL_TYPE_ASI_HIGH_SPEED_MODE => Some(ASIControlType::HighSpeedMode),
            ASI_CONTROL_TYPE_ASI_COOLER_POWER_PERC => Some(ASIControlType::CoolerPowerPercent),
            ASI_CONTROL_TYPE_ASI_TARGET_TEMP => Some(ASIControlType::TargetTemp),
            ASI_CONTROL_TYPE_ASI_COOLER_ON => Some(ASIControlType::CoolerOn),
            ASI_CONTROL_TYPE_ASI_MONO_BIN => Some(ASIControlType::MonoBin),
            ASI_CONTROL_TYPE_ASI_FAN_ON => Some(ASIControlType::FanOn),
            ASI_CONTROL_TYPE_ASI_PATTERN_ADJUST => Some(ASIControlType::PatternAdjust),
            ASI_CONTROL_TYPE_ASI_ANTI_DEW_HEATER => Some(ASIControlType::AntiDewHeater),
            _ => None,
        }
    }
}

impl ASIBayerPattern {
    fn from_u32(val: u32) -> Option<Self> {
        match val {
            ASI_BAYER_PATTERN_ASI_BAYER_RG => Some(ASIBayerPattern::Bayer_RG),
            ASI_BAYER_PATTERN_ASI_BAYER_BG => Some(ASIBayerPattern::Bayer_BG),
            ASI_BAYER_PATTERN_ASI_BAYER_GR => Some(ASIBayerPattern::Bayer_GR),
            ASI_BAYER_PATTERN_ASI_BAYER_GB => Some(ASIBayerPattern::Bayer_GB),
            _ => None,
        }
    }
}

impl ASIImageFormat {
    fn from_u32(val: u32) -> Option<Self> {
        match val as i32 {
            ASI_IMG_TYPE_ASI_IMG_RAW8 => Some(ASIImageFormat::Image_RAW8),
            ASI_IMG_TYPE_ASI_IMG_RGB24 => Some(ASIImageFormat::Image_RGB24),
            ASI_IMG_TYPE_ASI_IMG_RAW16 => Some(ASIImageFormat::Image_RAW16),
            _ => None,
        }
    }
}

#[repr(u32)]
#[derive(Debug, PartialEq, Clone, Copy)]
enum ASIBayerPattern {
    Bayer_RG = ASI_BAYER_PATTERN_ASI_BAYER_RG,
    Bayer_BG = ASI_BAYER_PATTERN_ASI_BAYER_BG,
    Bayer_GR = ASI_BAYER_PATTERN_ASI_BAYER_GR,
    Bayer_GB = ASI_BAYER_PATTERN_ASI_BAYER_GB,
}

#[repr(i32)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ASIImageFormat {
    Image_RAW8 = ASI_IMG_TYPE_ASI_IMG_RAW8,
    Image_RGB24 = ASI_IMG_TYPE_ASI_IMG_RGB24,
    Image_RAW16 = ASI_IMG_TYPE_ASI_IMG_RAW16,
}

#[repr(i32)]
#[derive(Debug, PartialEq, Clone, Copy)]
enum ASIControlType {
    Gain = ASI_CONTROL_TYPE_ASI_GAIN as i32,
    Exposure = ASI_CONTROL_TYPE_ASI_EXPOSURE as i32,
    Gamma = ASI_CONTROL_TYPE_ASI_GAMMA as i32,
    WhiteBal_R = ASI_CONTROL_TYPE_ASI_WB_R as i32,
    WhiteBal_B = ASI_CONTROL_TYPE_ASI_WB_B as i32,
    Offset = ASI_CONTROL_TYPE_ASI_OFFSET as i32,
    BWOvld = ASI_CONTROL_TYPE_ASI_BANDWIDTHOVERLOAD as i32,
    Overclock = ASI_CONTROL_TYPE_ASI_OVERCLOCK as i32,
    Temperature = ASI_CONTROL_TYPE_ASI_TEMPERATURE as i32,
    Flip = ASI_CONTROL_TYPE_ASI_FLIP as i32,
    AutoExpMaxGain = ASI_CONTROL_TYPE_ASI_AUTO_MAX_GAIN as i32,
    AutoExpMaxExp = ASI_CONTROL_TYPE_ASI_AUTO_MAX_EXP as i32,
    AutoExpTgtBrightness = ASI_CONTROL_TYPE_ASI_AUTO_TARGET_BRIGHTNESS as i32,
    HWBin = ASI_CONTROL_TYPE_ASI_HARDWARE_BIN as i32,
    HighSpeedMode = ASI_CONTROL_TYPE_ASI_HIGH_SPEED_MODE as i32,
    CoolerPowerPercent = ASI_CONTROL_TYPE_ASI_COOLER_POWER_PERC as i32,
    TargetTemp = ASI_CONTROL_TYPE_ASI_TARGET_TEMP as i32,
    CoolerOn = ASI_CONTROL_TYPE_ASI_COOLER_ON as i32,
    MonoBin = ASI_CONTROL_TYPE_ASI_MONO_BIN as i32,
    FanOn = ASI_CONTROL_TYPE_ASI_FAN_ON as i32,
    PatternAdjust = ASI_CONTROL_TYPE_ASI_PATTERN_ADJUST as i32,
    AntiDewHeater = ASI_CONTROL_TYPE_ASI_ANTI_DEW_HEATER as i32,
}

#[repr(u32)]
#[derive(Clone, PartialEq, Copy)]
enum ASIExposureStatus {
    Idle = ASI_EXPOSURE_STATUS_ASI_EXP_IDLE,
    Working = ASI_EXPOSURE_STATUS_ASI_EXP_WORKING,
    Success = ASI_EXPOSURE_STATUS_ASI_EXP_SUCCESS,
    Failed = ASI_EXPOSURE_STATUS_ASI_EXP_FAILED,
}

#[derive(Clone)]
struct ASIControlCaps {
    id: ASIControlType,
    name: [u8; 64],
    description: [u8; 128],
    min_value: i64,
    max_value: i64,
    default_value: i64,
    is_auto_supported: bool,
    is_writable: bool,
}

#[derive(Clone)]
struct ASIRoiMode {
    width: i32,
    height: i32,
    bin: i32,
    fmt: ASIImageFormat,
}

#[derive(Clone, PartialEq, PartialOrd, Eq)]
struct ASICamId(i32);

impl Drop for ASICamId {
    fn drop(&mut self) {
        let res = unsafe { ASIStopExposure(self.0) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            warn!("Invalid camera ID: {}", self.0);
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            warn!("Camera {} is closed", self.0);
        }
        let res = unsafe { ASICloseCamera(self.0) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            warn!("Invalid camera ID: {}", self.0);
        }
    }
}

fn get_control_caps(id: i32) -> Result<Vec<ASIControlCaps>, Error> {
    let mut num_caps: i32 = 0;
    let res = unsafe { ASIGetNumOfControls(id, &mut num_caps) };
    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
        return Err(Error::InvalidId(id));
    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
        return Err(Error::CameraClosed);
    }
    let mut caps = Vec::<ASIControlCaps>::with_capacity(num_caps as usize);

    for i in 0..num_caps {
        let cap = MaybeUninit::<ASI_CONTROL_CAPS>::zeroed();
        let mut cap = unsafe { cap.assume_init() };
        let res = unsafe { ASIGetControlCaps(id, i, &mut cap) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        let cap = ASIControlCaps {
            id: ASIControlType::from_u32(cap.ControlType).unwrap(),
            name: unsafe { std::mem::transmute_copy::<[i8; 64], [u8; 64]>(&cap.Name) },
            description: unsafe {
                std::mem::transmute_copy::<[i8; 128], [u8; 128]>(&cap.Description)
            },
            min_value: cap.MinValue,
            max_value: cap.MaxValue,
            default_value: cap.DefaultValue,
            is_auto_supported: cap.IsAutoSupported == ASI_BOOL_ASI_TRUE,
            is_writable: cap.IsWritable == ASI_BOOL_ASI_TRUE,
        };
        caps.push(cap);
    }

    Ok(caps)
}

fn get_gain_minmax(caps: &Vec<ASIControlCaps>) -> (i64, i64) {
    let minmax = get_controlcap_minmax(caps, ASIControlType::Gain);
    if let Some((min, max)) = minmax {
        return (min, max);
    }
    (0, 0)
}

fn get_exposure_minmax(caps: &Vec<ASIControlCaps>) -> (Duration, Duration) {
    let minmax = get_controlcap_minmax(caps, ASIControlType::Exposure);
    if let Some((min, max)) = minmax {
        return (
            Duration::from_micros(min as u64),
            Duration::from_micros(max as u64),
        );
    }
    (Duration::from_micros(1000 as u64), Duration::from_secs(200))
}

fn get_controlcap_minmax(caps: &Vec<ASIControlCaps>, id: ASIControlType) -> Option<(i64, i64)> {
    for cap in caps {
        if cap.id == id {
            return Some((cap.min_value, cap.max_value));
        }
    }
    None
}

fn sys_cancel_capture(id: i32) -> Result<(), Error> {
    let res = unsafe { ASIStopExposure(id) };
    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
        return Err(Error::InvalidId(id));
    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
        return Err(Error::CameraClosed);
    }
    Ok(())
}

fn get_control_value(id: i32, ctyp: ASIControlType) -> Result<(c_long, bool), Error> {
    let mut val: c_long = 0;
    let mut auto_val: i32 = ASI_BOOL_ASI_FALSE as i32;
    let res = unsafe { ASIGetControlValue(id, ctyp as i32, &mut val, &mut auto_val) };
    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
        return Err(Error::InvalidId(id));
    } else if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_CONTROL_TYPE as i32 {
        return Err(Error::InvalidControlType(format!("{:#?}", ctyp)));
    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
        return Err(Error::CameraClosed);
    }
    Ok((val, auto_val == ASI_BOOL_ASI_TRUE as i32))
}

fn set_control_value(id: i32, ctyp: ASIControlType, val: c_long, auto: bool) -> Result<(), Error> {
    let res = unsafe {
        ASISetControlValue(
            id,
            ctyp as i32,
            val,
            if auto {
                ASI_BOOL_ASI_TRUE as i32
            } else {
                ASI_BOOL_ASI_FALSE as i32
            },
        )
    };
    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
        return Err(Error::InvalidId(id));
    } else if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_CONTROL_TYPE as i32 {
        return Err(Error::InvalidControlType(format!("{:#?}", ctyp)));
    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
        return Err(Error::CameraClosed);
    } else if res == ASI_ERROR_CODE_ASI_ERROR_GENERAL_ERROR as i32 {
        return Err(Error::Message(format!(
            "Could not set control value for type {:#?}",
            ctyp
        )));
    }
    Ok(())
}

fn set_temperature(id: i32, temperature: f32, is_cooler_cam: bool) -> Result<f32, Error> {
    if !is_cooler_cam {
        return Err(Error::InvalidControlType(
            "Camera does not have cooler".to_owned(),
        ));
    }
    if temperature < -80.0 {
        return Err(Error::InvalidValue(format!(
            "Temperature {} is below minimum of -80",
            temperature
        )));
    } else if temperature > 20.0 {
        return Err(Error::InvalidValue(format!(
            "Temperature {} is above maximum of 20",
            temperature
        )));
    }
    let temperature = temperature as c_long;
    set_control_value(id, ASIControlType::TargetTemp, temperature, false)?;
    Ok(temperature as f32)
}

fn get_cooler_power(id: i32) -> Option<f32> {
    let res = get_control_value(id, ASIControlType::CoolerPowerPercent);
    if let Ok((val, _)) = res {
        return Some(val as f32);
    }
    None
}

fn get_temperature(id: i32) -> Option<f32> {
    let res = get_control_value(id, ASIControlType::Temperature);
    if let Ok((val, _)) = res {
        return Some(val as f32 / 10.0);
    }
    None
}