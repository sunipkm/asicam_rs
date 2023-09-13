use fitsio::images::{ImageDescription, ImageType};
use fitsio::FitsFile;
use image::DynamicImage;
use log::{info, warn};
use std::fmt::Display;
use std::fs::remove_file;
use std::path::Path;
use std::time::Duration;

#[derive(Clone)]
pub struct ImageMetaData {
    pub bin_x: i32,
    pub bin_y: i32,
    pub img_top: i32,
    pub img_left: i32,
    pub temperature: f32,
    pub exposure: std::time::Duration,
    pub timestamp: u64,
    pub camera_name: String,
    pub gain: i64,
    pub offset: i64,
    pub min_gain: i32,
    pub max_gain: i32,
    pub extended_metadata: Vec<(String, String)>,
}

impl Display for ImageMetaData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ImageMetaData [{}]:\n
            \tCamera name: {}\n
            \tImage Bin: {} x {}\n
            \tImage Origin: {} x {}
            \tExposure: {} s\n
            \tGain: {}, Offset: {}\n
            \tTemperature: {} C\n",
            self.timestamp,
            self.camera_name,
            self.bin_x,
            self.bin_y,
            self.img_left,
            self.img_top,
            self.exposure.as_secs(),
            self.gain,
            self.offset,
            self.temperature
        )?;
        if self.extended_metadata.len() > 0 {
            write!(f, "\tExtended Metadata:\n")?;
            for obj in self.extended_metadata.iter() {
                write!(f, "\t\t{}: {}\n", obj.0, obj.1)?;
            }
        };
        Ok(())
    }
}

impl ImageMetaData {
    /// Add an extended attribute to the image metadata using [`std::alloc::vec::push()`].
    ///
    /// # Panics
    ///
    /// If the new capacity exceeds `isize::MAX` bytes.
    pub fn add_extended_attrib(&mut self, key: &str, val: &str) {
        self.extended_metadata
            .push((key.to_string(), val.to_string()));
    }
}

#[derive(Clone)]
/// Image data structure
///
/// This structure contains the image data and the metadata associated with it.
pub struct ImageData {
    img: DynamicImage,
    meta: ImageMetaData,
}

impl Display for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.meta)?;
        write!(f, "Size: {} x {}", self.img.width(), self.img.height())
    }
}

impl ImageData {
    /// Create a new image data structure from a `DynamicImage` and `ImageMetaData`.
    pub fn new(img: DynamicImage, meta: ImageMetaData) -> Self {
        Self { img, meta }
    }

    pub fn add_extended_attrib(&mut self, key: &str, val: &str) {
        self.meta.add_extended_attrib(key, val);
    }

    pub fn get_metadata(&self) -> &ImageMetaData {
        &self.meta
    }

    pub fn set_metadata(&mut self, meta: ImageMetaData) {
        self.meta = meta;
    }

    pub fn get_image(&self) -> &DynamicImage {
        &self.img
    }

    pub fn get_image_mut(&mut self) -> &mut DynamicImage {
        &mut self.img
    }

    pub fn find_optimum_exposure(
        &self,
        percentile_pix: f32,
        pixel_tgt: u16,
        max_allowed_exp: Duration,
        max_allowed_bin: u16,
        pixel_exclusion: u32,
        pixel_uncertainty: u16,
    ) -> Result<(Duration, u16), String> {
        let exposure = self.meta.exposure;
        let mut target_exposure;
        let mut change_bin = true;
        if self.meta.bin_x != self.meta.bin_y {
            change_bin = false;
        }
        let mut bin = self.meta.bin_x as u16;
        info!(
            "Input: exposure = {} s, bin = {}",
            exposure.as_millis() as f64 * 1e-3,
            bin
        );
        let mut img = self.img.to_luma16();
        img.sort();
        let mut coord: usize;
        if percentile_pix > 99.9 {
            coord = img.len() - 1 as usize;
        } else {
            coord = (percentile_pix * (img.len() - 1) as f32 * 0.01).floor() as usize;
        }
        if coord < pixel_exclusion as usize {
            coord = img.len() - 1 - pixel_exclusion as usize;
        }
        info!("Pixel coord: {} out of {}", coord, img.len());
        let imgvec = img.to_vec();
        let val = imgvec.get(coord);
        let val = match val {
            Some(v) => *v as f32,
            None => {
                info!("Could not get pixel value at {} percentile", percentile_pix);
                1e-5 as f32
            }
        };

        if (pixel_tgt as f32 - val).abs() < pixel_uncertainty as f32 {
            info!(
                "Target pixel value {} reached at exposure = {} s, bin = {}, unchanged.",
                pixel_tgt,
                exposure.as_millis() as f64 * 1e-3,
                bin
            );
            return Ok((exposure, bin));
        }

        let val = {
            if val <= 1e-5 {
                1e-5
            } else {
                val
            }
        };

        target_exposure = Duration::from_micros(
            ((pixel_tgt as f64 * exposure.as_micros() as f64 * 1e-6 / val as f64) * 1e6).abs()
                as u64,
        );

        if change_bin {
            let mut tgt_exp = target_exposure.as_micros() as f64 * 1e-6;
            let mut bin_ = bin;
            if tgt_exp < max_allowed_exp.as_micros() as f64 * 1e-6 {
                while tgt_exp < max_allowed_exp.as_micros() as f64 && bin_ > 2 {
                    bin_ /= 2;
                    tgt_exp *= 4.0;
                }
            } else {
                while tgt_exp > max_allowed_exp.as_micros() as f64 && bin_ * 2 <= max_allowed_bin {
                    bin_ *= 2;
                    tgt_exp /= 4.0;
                }
            }
            target_exposure = Duration::from_micros((tgt_exp * 1e6 as f64).abs() as u64);
            bin = bin_;
        }

        if target_exposure > max_allowed_exp {
            target_exposure = max_allowed_exp;
        }

        if target_exposure.as_micros() as f64 * 1e-6 < 1e-5 {
            target_exposure = Duration::from_micros(10);
        }

        if bin < 1 {
            bin = 1;
        }
        if bin > max_allowed_bin {
            bin = max_allowed_bin;
        }
        info!(
            "Target exposure = {} s, bin = {}",
            target_exposure.as_millis() as f64 * 1e-3,
            bin
        );

        Ok((target_exposure, bin))
    }

    pub fn save_fits(
        &self,
        dir_prefix: &Path,
        file_prefix: &str,
        progname: &str,
        compress: bool,
        overwrite: bool,
    ) -> Result<(), fitsio::errors::Error> {
        if !dir_prefix.exists() {
            return Err(fitsio::errors::Error::Message(format!(
                "Directory {} does not exist",
                dir_prefix.to_string_lossy()
            )));
        }

        let fpath = dir_prefix.join(Path::new(&format!(
            "{}_{}.fits",
            file_prefix, self.meta.timestamp
        )));

        if fpath.exists() {
            warn!("File {} already exists", fpath.to_string_lossy());
            if !overwrite {
                return Err(fitsio::errors::Error::Message(format!(
                    "File {:?} already exists",
                    fpath
                )));
            } else {
                info!("Overwriting file {:?}", fpath);
                let res = remove_file(fpath.clone());
                if let Err(msg) = res {
                    return Err(fitsio::errors::Error::Message(format!(
                        "Could not remove file {:?}: {:?}",
                        fpath, msg
                    )));
                }
            }
        }

        let imgtype = self.img.color();
        let width = self.img.width();
        let height = self.img.height();
        let imgsize = [width as usize, height as usize];
        let data_type: ImageType;

        match imgtype {
            image::ColorType::L8 => {
                data_type = ImageType::UnsignedByte;
            }
            image::ColorType::L16 => {
                data_type = ImageType::UnsignedShort;
            }
            _ => {
                return Err(fitsio::errors::Error::Message(format!(
                    "Unsupported image type {:?}",
                    imgtype
                )));
            }
        };

        let img_desc = ImageDescription {
            data_type,
            dimensions: &imgsize,
        };
        let path = Path::new(dir_prefix).join(Path::new(&format!(
            "{}_{}.fits{}",
            file_prefix,
            self.meta.timestamp,
            if compress { "[compress]" } else { "" }
        )));
        let mut fptr = FitsFile::create(path).open()?;

        let hdu = fptr.create_image("IMAGE".to_string(), &img_desc)?;
        match imgtype {
            image::ColorType::L8 => {
                hdu.write_image(&mut fptr, self.img.to_luma8().as_raw())?;
            }
            image::ColorType::L16 => {
                hdu.write_image(&mut fptr, self.img.to_luma16().as_raw())?;
            }
            _ => {
                return Err(fitsio::errors::Error::Message(format!(
                    "Unsupported image type {:?}",
                    imgtype
                )));
            }
        }
        hdu.write_key(&mut fptr, "PROGRAM", progname)?;
        hdu.write_key(&mut fptr, "CAMERA", self.meta.camera_name.as_str())?;
        hdu.write_key(&mut fptr, "TIMESTAMP", self.meta.timestamp)?;
        hdu.write_key(&mut fptr, "CCDTEMP", self.meta.temperature)?;
        hdu.write_key(
            &mut fptr,
            "EXPOSURE_US",
            self.meta.exposure.as_micros() as u64,
        )?;
        hdu.write_key(&mut fptr, "ORIGIN_X", self.meta.img_left)?;
        hdu.write_key(&mut fptr, "ORIGIN_Y", self.meta.img_top)?;
        hdu.write_key(&mut fptr, "BINX", self.meta.bin_x)?;
        hdu.write_key(&mut fptr, "BINY", self.meta.bin_y)?;
        hdu.write_key(&mut fptr, "GAIN", self.meta.gain)?;
        hdu.write_key(&mut fptr, "OFFSET", self.meta.offset)?;
        hdu.write_key(&mut fptr, "GAIN_MIN", self.meta.min_gain)?;
        hdu.write_key(&mut fptr, "GAIN_MAX", self.meta.max_gain)?;
        for obj in self.meta.extended_metadata.iter() {
            hdu.write_key(&mut fptr, &obj.0, obj.1.as_str())?;
        }

        Ok(())
    }
}
