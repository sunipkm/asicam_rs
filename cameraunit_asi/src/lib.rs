mod asicamera_2;

pub use asicamera_2::{
    get_camera_ids, num_cameras, open_camera, open_first_camera, ASICameraProps, ASIImageFormat,
    CameraInfo_ASI, CameraUnit_ASI,
};

#[cfg(test)]
mod tests {
    use std::{path::Path, thread::sleep, time::Duration};

    use cameraunit::{CameraUnit, Error};

    use crate::{num_cameras, open_first_camera};

    #[test]
    fn test_write_image() -> () {
        let nc = num_cameras();
        if nc <= 0 {
            ()
        }
        let (mut cam, _) = open_first_camera().unwrap();
        cam.set_exposure(Duration::from_millis(700)).unwrap();
        cam.start_exposure().unwrap();
        while cam
            .image_ready()
            .is_err_and(|x| x == Error::ExposureInProgress)
        {
            sleep(Duration::from_secs(1));
        }
        let img = cam.download_image().unwrap();
        img.save_fits(Path::new("./"), "test", "asicam_test", true, true)
            .unwrap();
    }
}
