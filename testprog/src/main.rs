use cameraunit_asi::{ASICameraProps, CameraUnit_ASI};
#[macro_use]
use ini::ini;

fn main() {
    let config = ini!(safe "asicam.ini").unwrap();
    println!("Config: {:?}", config);
}