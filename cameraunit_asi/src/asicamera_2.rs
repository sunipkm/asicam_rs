#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!("asicamera2_bindings.rs");

use std::{
    ffi::{c_long, c_uchar, CStr},
    fmt::Display,
    mem::MaybeUninit,
    thread::sleep,
    time::{Duration, SystemTime},
};

use cameraunit::{CameraUnit, Error, ROI};
use image::DynamicImage;
use imagedata::{ImageData, ImageMetaData};
use log::warn;

#[repr(u32)]
#[derive(Debug, PartialEq, Clone)]
enum ASIBayerPattern {
    Bayer_RG = 0,
    Bayer_BG = 1,
    Bayer_GR = 2,
    Bayer_GB = 3,
}

impl ASIBayerPattern {
    fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(ASIBayerPattern::Bayer_RG),
            1 => Some(ASIBayerPattern::Bayer_BG),
            2 => Some(ASIBayerPattern::Bayer_GR),
            3 => Some(ASIBayerPattern::Bayer_GB),
            _ => None,
        }
    }
}

#[repr(i32)]
#[derive(Debug, PartialEq, Clone)]
enum ASIImageFormat {
    Image_RAW8 = 0,
    Image_RGB24 = 1,
    Image_RAW16 = 2,
}

impl ASIImageFormat {
    fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(ASIImageFormat::Image_RAW8),
            1 => Some(ASIImageFormat::Image_RGB24),
            2 => Some(ASIImageFormat::Image_RAW16),
            _ => None,
        }
    }
}

#[repr(i32)]
#[derive(Debug, PartialEq, Clone)]
enum ASIControlType {
    Gain = 0,
    Exposure = 1,
    Gamma = 2,
    WhiteBal_R = 3,
    WhiteBal_B = 4,
    Offset = 5,
    BWOvld = 6,
    Overclock = 7,
    Temperature = 8,
    Flip = 9,
    AutoExpMaxGain = 10,
    AutoExpMaxExp = 11,
    AutoExpTgtBrightness = 12,
    HWBin = 13,
    HighSpeedMode = 14,
    CoolerPowerPercent = 15,
    TargetTemp = 16,
    CoolerOn = 17,
    MonoBin = 18,
    FanOn = 19,
    PatternAdjust = 20,
    AntiDewHeater = 21,
}

impl ASIControlType {
    fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(ASIControlType::Gain),
            1 => Some(ASIControlType::Exposure),
            2 => Some(ASIControlType::Gamma),
            3 => Some(ASIControlType::WhiteBal_R),
            4 => Some(ASIControlType::WhiteBal_B),
            5 => Some(ASIControlType::Offset),
            6 => Some(ASIControlType::BWOvld),
            7 => Some(ASIControlType::Overclock),
            8 => Some(ASIControlType::Temperature),
            9 => Some(ASIControlType::Flip),
            10 => Some(ASIControlType::AutoExpMaxGain),
            11 => Some(ASIControlType::AutoExpMaxExp),
            12 => Some(ASIControlType::AutoExpTgtBrightness),
            13 => Some(ASIControlType::HWBin),
            14 => Some(ASIControlType::HighSpeedMode),
            15 => Some(ASIControlType::CoolerPowerPercent),
            16 => Some(ASIControlType::TargetTemp),
            17 => Some(ASIControlType::CoolerOn),
            18 => Some(ASIControlType::MonoBin),
            19 => Some(ASIControlType::FanOn),
            20 => Some(ASIControlType::PatternAdjust),
            21 => Some(ASIControlType::AntiDewHeater),
            _ => None,
        }
    }
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

impl ASICameraProps {}

#[derive(Clone, PartialEq)]
enum ASIExposureStatus {
    Idle = 0,
    Working = 1,
    Success = 2,
    Failed = 3,
}

impl ASIExposureStatus {
    fn from_u32(val: u32) -> Result<Self, Error> {
        match val {
            0 => Ok(ASIExposureStatus::Idle),
            1 => Ok(ASIExposureStatus::Working),
            2 => Ok(ASIExposureStatus::Success),
            3 => Ok(ASIExposureStatus::Failed),
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

#[derive(Clone)]
pub struct ASIControlCaps {
    id: ASIControlType,
    name: [u8; 64],
    description: [u8; 128],
    min_value: i64,
    max_value: i64,
    default_value: i64,
    is_auto_supported: bool,
    is_writable: bool,
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

#[derive(Clone)]
struct ASIRoiMode {
    width: i32,
    height: i32,
    bin: i32,
    fmt: ASIImageFormat,
}

#[derive(Clone)]
pub struct CameraUnit_ASI {
    id: i32,
    capturing: bool,
    props: Box<ASICameraProps>,
    control_caps: Vec<ASIControlCaps>,
    gain_min: i64,
    gain_max: i64,
    exp_min: Duration,
    exp_max: Duration,
    exposure: Duration,
    is_dark_frame: bool,
    image_fmt: ASIImageFormat,
    roi: ROI,
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
    for cap in caps {
        if cap.id == ASIControlType::Gain {
            return (cap.min_value, cap.max_value);
        }
    }
    (0, 0)
}

fn get_exposure_minmax(caps: &Vec<ASIControlCaps>) -> (Duration, Duration) {
    for cap in caps {
        if cap.id == ASIControlType::Exposure {
            return (
                Duration::from_micros(cap.min_value as u64),
                Duration::from_micros(cap.max_value as u64),
            );
        }
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

impl CameraUnit_ASI {
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
                self.id,
                &mut roi.width,
                &mut roi.height,
                &mut roi.bin,
                &mut fmt,
            )
        };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id));
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
            unsafe { ASISetROIFormat(self.id, roi.width, roi.height, roi.bin, roi.fmt as i32) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        Ok(())
    }

    fn get_control_value(&self, ctyp: ASIControlType) -> Result<(c_long, bool), Error> {
        let mut val: c_long = 0;
        let mut auto_val: i32 = ASI_BOOL_ASI_FALSE as i32;
        let res = unsafe { ASIGetControlValue(self.id, ctyp as i32, &mut val, &mut auto_val) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_CONTROL_TYPE as i32 {
            return Err(Error::InvalidControlType(format!(
                "{:#?}",
                self.control_caps[0].id
            )));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        Ok((val, auto_val == ASI_BOOL_ASI_TRUE as i32))
    }

    pub fn num_cameras() -> i32 {
        unsafe { ASIGetNumOfConnectedCameras() }
    }

    pub fn get_camera_ids() -> Option<Vec<i32>> {
        let num_cameras = Self::num_cameras();
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

    pub fn open_camera(id: i32) -> Result<Self, Error> {
        if let Some(cam_ids) = Self::get_camera_ids() {
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
                id: id,
                capturing: false,
                props: Box::new(prop),
                control_caps: ccaps,
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

            cobj.set_roi_format(&ASIRoiMode {
                width: cobj.roi.x_max - cobj.roi.x_min,
                height: cobj.roi.y_max - cobj.roi.y_min,
                bin: cobj.roi.bin_x,
                fmt: cobj.image_fmt,
            })?;

            return Ok(cobj);
        } else {
            return Err(Error::NoCamerasAvailable);
        }
    }

    pub fn open_camera_by_index(index: u32) -> Result<Self, Error> {
        if index >= Self::num_cameras() as u32 {
            return Err(Error::InvalidIndex(index as i32));
        }
        let info = MaybeUninit::<ASI_CAMERA_INFO>::zeroed();
        let mut info = unsafe { info.assume_init() };
        let res = unsafe { ASIGetCameraProperty(&mut info, index as i32) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_INDEX as i32 {
            return Err(Error::InvalidIndex(index as i32));
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
            let res = unsafe { ASIGetID(prop.id, &mut cid) };
            if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                return Err(Error::InvalidId(prop.id));
            } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                return Err(Error::CameraClosed);
            }
            prop.uuid = cid.id;
        }

        let res = unsafe { ASIOpenCamera(prop.id) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(prop.id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_REMOVED as i32 {
            return Err(Error::CameraRemoved);
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
            id: prop.id,
            capturing: false,
            props: Box::new(prop),
            control_caps: ccaps,
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

        cobj.set_roi_format(&ASIRoiMode {
            width: cobj.roi.x_max - cobj.roi.x_min,
            height: cobj.roi.y_max - cobj.roi.y_min,
            bin: cobj.roi.bin_x,
            fmt: cobj.image_fmt,
        })?;

        Ok(cobj)
    }

    pub fn set_uuid(&mut self, uuid: &[u8; 8]) -> Result<(), Error> {
        if self.props.uuid == *uuid {
            Ok(())
        } else {
            let cid = ASI_ID { id: *uuid };
            let res = unsafe { ASISetID(self.id, cid) };
            if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                return Err(Error::InvalidId(self.id));
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
        let res = unsafe { ASIGetSerialNumber(self.id, &mut ser) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_GENERAL_ERROR as i32 {
            return Err(Error::GeneralError(
                "Camera does not have serial number.".to_owned(),
            ));
        }
        let ser = u64::from_be_bytes(ser.id);
        Ok(ser)
    }

    fn get_exposure_status(&self) -> Result<ASIExposureStatus, Error> {
        let stat = MaybeUninit::<ASI_EXPOSURE_STATUS>::zeroed();
        let mut stat = unsafe { stat.assume_init() };
        let res = unsafe { ASIGetExpStatus(self.id, &mut stat) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        ASIExposureStatus::from_u32(stat)
    }
}

impl Drop for CameraUnit_ASI {
    fn drop(&mut self) {
        if self.capturing {
            if let Err(var) = self.cancel_capture() {
                warn!("Error while cancelling capture: {}", var);
            }
        }
        let res = unsafe { ASICloseCamera(self.id) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            warn!("Camera already closed");
        }
    }
}

impl CameraUnit for CameraUnit_ASI {
    fn get_vendor(&self) -> &str {
        "ZWO"
    }

    fn get_handle(&self) -> Option<&dyn std::any::Any> {
        Some(&self.id)
    }

    fn get_uuid(&self) -> Option<String> {
        Some(String::from_utf8_lossy(&self.props.uuid).to_string())
    }

    fn cancel_capture(&self) -> Result<(), Error> {
        if !self.capturing {
            return Ok(());
        }
        let res = unsafe { ASIStopExposure(self.id) };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            return Err(Error::InvalidId(self.id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            return Err(Error::CameraClosed);
        }
        Ok(())
    }

    fn is_capturing(&self) -> bool {
        self.capturing
    }

    fn camera_ready(&self) -> bool {
        true
    }

    fn camera_name(&self) -> &str {
        &self.props.name
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
        let stat = self.get_exposure_status()?;
        if stat == ASIExposureStatus::Working {
            self.capturing = true;
            return Err(Error::ExposureInProgress);
        } else if stat == ASIExposureStatus::Failed {
            return Err(Error::ExposureFailed("Unknown".to_owned()));
        }
        self.capturing = false;
        let roi = self.get_roi_format()?;
        self.capturing = true;
        let start_time = SystemTime::now();
        let res = unsafe {
            ASIStartExposure(
                self.id,
                if self.is_dark_frame {
                    ASI_BOOL_ASI_TRUE as i32
                } else {
                    ASI_BOOL_ASI_TRUE as i32
                },
            )
        };
        if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
            self.capturing = false;
            return Err(Error::InvalidId(self.id));
        } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
            self.capturing = false;
            return Err(Error::CameraClosed);
        } else if res == ASI_ERROR_CODE_ASI_ERROR_VIDEO_MODE_ACTIVE as i32 {
            self.capturing = true;
            return Err(Error::GeneralError("Video mode active".to_owned()));
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
        if stat == ASIExposureStatus::Failed {
            self.capturing = false;
            return Err(Error::ExposureFailed("Unknown".to_owned()));
        } else if stat == ASIExposureStatus::Idle {
            self.capturing = false;
            return Err(Error::ExposureFailed(
                "Successful exposure but no available data".to_owned(),
            ));
        } else if stat == ASIExposureStatus::Working {
            self.cancel_capture()?;
            return Err(Error::ExposureFailed("Exposure timed out".to_owned()));
        } else {
            let img = match roi.fmt {
                ASIImageFormat::Image_RAW8 => {
                    let mut data = vec![0u8; (roi.width * roi.height) as usize];
                    let res = unsafe {
                        ASIGetDataAfterExp(
                            self.id,
                            data.as_mut_ptr() as *mut c_uchar,
                            (roi.width * roi.height) as c_long,
                        )
                    };
                    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                        self.capturing = false;
                        return Err(Error::InvalidId(self.id));
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                        self.capturing = false;
                        return Err(Error::CameraClosed);
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_TIMEOUT as i32 {
                        self.capturing = false;
                        return Err(Error::TimedOut);
                    }
                    let mut img =
                        DynamicImage::new_luma8(roi.width as u32, roi.height as u32).into_luma8();
                    img.copy_from_slice(&data);
                    DynamicImage::from(img)
                }
                ASIImageFormat::Image_RAW16 => {
                    let mut data = vec![0u16; (roi.width * roi.height) as usize];
                    let res = unsafe {
                        ASIGetDataAfterExp(
                            self.id,
                            data.as_mut_ptr() as *mut c_uchar,
                            (roi.width * roi.height * 2) as c_long,
                        )
                    };
                    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                        self.capturing = false;
                        return Err(Error::InvalidId(self.id));
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                        self.capturing = false;
                        return Err(Error::CameraClosed);
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_TIMEOUT as i32 {
                        self.capturing = false;
                        return Err(Error::TimedOut);
                    }
                    let mut img =
                        DynamicImage::new_luma16(roi.width as u32, roi.height as u32).into_luma16();
                    img.copy_from_slice(&data);
                    DynamicImage::from(img)
                }
                ASIImageFormat::Image_RGB24 => {
                    let mut data = vec![0u8; (roi.width * roi.height * 3) as usize];
                    let res = unsafe {
                        ASIGetDataAfterExp(
                            self.id,
                            data.as_mut_ptr() as *mut c_uchar,
                            (roi.width * roi.height * 3) as c_long,
                        )
                    };
                    if res == ASI_ERROR_CODE_ASI_ERROR_INVALID_ID as i32 {
                        self.capturing = false;
                        return Err(Error::InvalidId(self.id));
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED as i32 {
                        self.capturing = false;
                        return Err(Error::CameraClosed);
                    } else if res == ASI_ERROR_CODE_ASI_ERROR_TIMEOUT as i32 {
                        self.capturing = false;
                        return Err(Error::TimedOut);
                    }
                    let mut img =
                        DynamicImage::new_rgb8(roi.width as u32, roi.height as u32).into_rgb8();
                    img.copy_from_slice(&data);
                    DynamicImage::from(img)
                }
            };
            let meta = ImageMetaData::full_builder(
                self.get_bin_x() as u32,
                self.get_bin_y() as u32,
                self.roi.y_min as u32,
                self.roi.x_min as u32,
                self.get_temperature().unwrap(),
                self.exposure,
                start_time,
                self.camera_name(),
                self.get_gain_raw(),
                self.get_offset() as i64,
                self.get_min_gain().unwrap() as i32,
                self.get_max_gain().unwrap() as i32,
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

    fn get_cooler_power(&self) -> Option<f32> {
        let res = self.get_control_value(ASIControlType::CoolerPowerPercent);
        if let Ok((val, _)) = res {
            return Some(val as f32);
        }
        None
    }

    fn get_exposure(&self) -> Duration {
        self.exposure
    }

    fn get_gain(&self) -> f32 {
        let res = self.get_control_value(ASIControlType::Gain);
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
        let res = self.get_control_value(ASIControlType::Gain);
        if let Ok((val, _)) = res {
            return val;
        }
        0
    }

    fn get_offset(&self) -> i32 {
        let res = self.get_control_value(ASIControlType::Offset);
        if let Ok((val, _)) = res {
            return val as i32;
        }
        0
    }
    fn get_last_image(&self) -> Option<ImageData> {}

    fn get_shutter_open(&self) -> Result<bool, Error> {}

    fn get_temperature(&self) -> Option<f32> {}

    fn set_cooler_power(&self, power: f32) -> Result<f32, Error> {}

    fn set_exposure(&self, exposure: Duration) -> Result<Duration, Error> {}

    fn set_gain(&self, gain: f32) -> Result<f32, Error> {}

    fn set_gain_raw(&self, gain: i64) -> Result<i64, Error> {}

    fn set_offset(&self, offset: i32) -> Result<i32, Error> {}

    fn set_roi(&self, roi: &ROI) -> Result<&ROI, Error> {}

    fn set_shutter_open(&self, open: bool) -> Result<bool, Error> {}

    fn set_temperature(&self, temperature: f32) -> Result<f32, Error> {}
}
