
use image::{GenericImage, ImageBuffer, DynamicImage};
use rand::Rng;
use fitsio::FitsFile;
fn main() {
    let width = 800;
    let height = 600;
    let mut rng = rand::thread_rng();
    let vals: Vec<u16> = (0..width*height).map(|_| rng.gen_range(0..255*255)).collect();
    let mut img = DynamicImage::new_luma16(width, height).into_luma16();
    img.copy_from_slice(&vals);
    img.save("test.png").unwrap();
    let img = ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_raw(width, height, vals).unwrap();
    img.save("test2.png").unwrap();
    DynamicImage::from(img).into_luma8().save("test3.png").unwrap();

    let mut fptr = FitsFile::open("test_0.fits").unwrap();
    for hdu in fptr.iter() {
        println!("HDU: {:?}", hdu);
    }
}
