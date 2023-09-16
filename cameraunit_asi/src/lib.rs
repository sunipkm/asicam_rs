mod asicamera_2;

pub use asicamera_2::{
    get_camera_ids, num_cameras, open_camera, open_first_camera, ASICameraProps, ASIImageFormat,
    CameraInfo_ASI, CameraUnit_ASI,
};

#[cfg(test)]
mod tests {
    use std::{path::Path, thread::sleep, time::Duration};

    use cameraunit::CameraUnit;

    use crate::{num_cameras, open_first_camera};

    #[test]
    fn test_write_image() -> () {
        let nc = num_cameras();
        if nc <= 0 {
            return;
        }
        let (mut cam, _) = open_first_camera()
            .map_err(|x| println!("Opening camera: {}", x.to_string()))
            .unwrap();
        cam.set_exposure(Duration::from_millis(700))
            .map_err(|x| println!("Setting exposure: {}", x.to_string()))
            .unwrap();
        cam.start_exposure()
            .map_err(|x| println!("Start exposure: {}", x.to_string()))
            .unwrap();
        while !cam
            .image_ready()
            .map_err(|x| println!("Check exposure: {}", x.to_string()))
            .unwrap()
        {
            sleep(Duration::from_secs(1));
        }
        let img = cam
            .download_image()
            .map_err(|x| println!("Downloading image: {}", x.to_string()))
            .unwrap();
        img.save_fits(Path::new("./"), "test", "asicam_test", true, true)
            .unwrap();
    }
}
