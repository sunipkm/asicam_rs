mod imagedata;

pub use imagedata::{ImageData, ImageMetaData};

#[cfg(test)]
mod tests {
    use std::{path::Path, time::{Duration, UNIX_EPOCH, SystemTime}};

    use super::*;
    use image::{DynamicImage, ImageBuffer};
    use rand::Rng;

    fn get_timestamp_millis(tstamp: SystemTime) -> u64 {
        tstamp.duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0)).as_millis() as u64
    }

    #[test]
    fn test_write_image() {
        let mut img = {
            let mut meta: ImageMetaData = Default::default();
            meta.timestamp = SystemTime::now();
            meta.camera_name = "ZWO ASI533MM Pro".to_string();
            meta.add_extended_attrib("TEST", "TEST");
            let img = DynamicImage::from(ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(800, 600));
            imagedata::ImageData::new(img, meta)
        };
        let bimg = img.get_image_mut().as_mut_luma16().unwrap();
        let mut rng = rand::thread_rng();
        let vals: Vec<u16> = (0..bimg.width() * bimg.height())
            .map(|_| rng.gen_range(0..255 * 255))
            .collect();
        bimg.copy_from_slice(&vals);
        img.save_fits(Path::new("."), "test", "testprog", true, true)
            .unwrap();
        img.get_image().save(format!("test_{}.png", get_timestamp_millis(img.get_metadata().timestamp))).unwrap();
    }
}
