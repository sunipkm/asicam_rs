use std::{path::Path, time::{Duration, SystemTime}, thread::{self, sleep}, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};

use cameraunit::{CameraUnit, CameraInfo, ROI};
use cameraunit_asi::{num_cameras, open_camera_by_index};
use chrono::{DateTime, Local};
use ini::ini;

#[derive(Debug)]
struct ASICamconfig {
    progname: String,
    savedir: String,
    cadence: Duration,
    max_exposure: Duration,
    percentile: f64,
    max_bin: i32,
    target_val: f32,
    target_uncertainty: f32,
    gain: i32,
    target_temp: f32,
}

fn main() {
    let done = Arc::new(AtomicBool::new(false));
    let done_thr = done.clone();
    let done_hdl = done.clone();

    ctrlc::set_handler(move || {
        done_hdl.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let num_cameras = num_cameras();
    println!("Found {} cameras", num_cameras);
    if num_cameras <= 0
    {
        return;
    }
    let (cam, caminfo) = open_camera_by_index(0).unwrap();
    let props = cam.get_props();
    println!("{}", props);
    let cfg = ASICamconfig::from_ini("asicam.ini").unwrap();
    let camthread = thread::spawn(move|| {
        while done_thr.load(Ordering::SeqCst)
        {
            sleep(Duration::from_secs(1));
            let temp = caminfo.get_temperature().unwrap();
            let dtime: DateTime<Local> = SystemTime::now().into();
            print!("[{}] Camera temperature: {:>+05.1} C, Cooler Power: {:>03}\r", dtime.format("%Y-%m-%d %H:%M:%S"), temp, caminfo.get_cooler_power().unwrap());
        }
    });
    cam.set_temperature(cfg.target_temp).unwrap();
}

impl Default for ASICamconfig {
    fn default() -> Self {
        Self {
            progname: "ASICam".to_string(),
            savedir: "./data".to_string(),
            cadence: Duration::from_secs(20),
            max_exposure: Duration::from_secs(120),
            percentile: 95.0,
            max_bin: 4,
            target_val: 30000.0 / 65536.0,
            target_uncertainty: 2000.0 / 65536.0,
            gain: 100,
            target_temp: -10.0
        }
    }
}

impl ASICamconfig {
    fn from_ini(path: &str) -> Result<ASICamconfig, String> {
        let config = ini!(safe path)?;
        let mut cfg = ASICamconfig::default();
        if config.contains_key("program") {
            if config["program"].contains_key("name")
            {
                cfg.progname = config["program"]["name"].clone().unwrap();
            }
        }
        if !config.contains_key("config")
        {
            return Err("No config section found".to_string());
        }
        if config["config"].contains_key("savedir")
        {
            cfg.savedir = config["config"]["savedir"].clone().unwrap();
        }
        if config["config"].contains_key("cadence")
        {
            cfg.cadence = Duration::from_secs(config["config"]["cadence"].clone().unwrap().parse::<u64>().unwrap());
        }
        if config["config"].contains_key("max_exposure")
        {
            cfg.max_exposure = Duration::from_secs(config["config"]["max_exposure"].clone().unwrap().parse::<u64>().unwrap());
        }
        if config["config"].contains_key("percentile")
        {
            cfg.percentile = config["config"]["percentile"].clone().unwrap().parse::<f64>().unwrap();
        }
        if config["config"].contains_key("maxbin")
        {
            cfg.max_bin = config["config"]["maxbin"].clone().unwrap().parse::<i32>().unwrap();
        }
        if config["config"].contains_key("value")
        {
            cfg.target_val = config["config"]["value"].clone().unwrap().parse::<f32>().unwrap();
            cfg.target_val /= 65536.0;
        }
        if config["config"].contains_key("uncertainty")
        {
            cfg.target_uncertainty = config["config"]["uncertainty"].clone().unwrap().parse::<f32>().unwrap();
            cfg.target_uncertainty /= 65536.0;
        }
        if config["config"].contains_key("gain")
        {
            cfg.gain = config["config"]["gain"].clone().unwrap().parse::<i32>().unwrap();
        }
        if config["config"].contains_key("target_temp")
        {
            cfg.target_temp = config["config"]["target_temp"].clone().unwrap().parse::<f32>().unwrap();
        }
        Ok(cfg)
    }
}