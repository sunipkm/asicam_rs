mod imagedata;

pub use imagedata::{ImageData, ImageMetaData};

#[cfg(test)]
mod tests {
    use std::{path::Path, time::Duration};

    use super::*;
    use image::{DynamicImage, ImageBuffer};
    use rand::Rng;

    #[test]
    fn test_write_image() {
        let mut img = {
            let mut meta = imagedata::ImageMetaData {
                bin_x: 1,
                bin_y: 1,
                img_top: 0,
                img_left: 0,
                temperature: 0.0,
                exposure: Duration::from_secs(1),
                timestamp: 1694548231100,
                camera_name: "Test".to_string(),
                gain: 0,
                offset: 0,
                min_gain: 0,
                max_gain: 0,
                extended_metadata: Vec::new(),
            };
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
        img.get_image().save(format!("test_{}.png", img.get_metadata().timestamp)).unwrap();
    }
}
