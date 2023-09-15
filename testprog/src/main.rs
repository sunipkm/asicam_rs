use std::{path::Path, time::{Duration, SystemTime}, thread::{self, sleep}, sync::{atomic::{AtomicBool, Ordering}, Arc}};

use cameraunit::{CameraUnit, CameraInfo, ROI};
use cameraunit_asi::{ASIImageFormat, num_cameras, open_first_camera};
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
    let num_cameras = num_cameras();
    println!("Found {} cameras", num_cameras);
    if num_cameras <= 0
    {
        return;
    }
    
    let done = Arc::new(AtomicBool::new(false));
    let done_thr = done.clone();
    let done_hdl = done.clone();

    ctrlc::set_handler(move || {
        done_hdl.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let (mut cam, caminfo) = open_first_camera().unwrap();
    let props = cam.get_props();
    println!("{}", props);
    let cfg = ASICamconfig::from_ini("asicam.ini").unwrap();
    cam.set_temperature(cfg.target_temp).unwrap();
    let camthread = thread::spawn(move|| {
        while done_thr.load(Ordering::SeqCst)
        {
            // let caminfo = cam;
            sleep(Duration::from_secs(1));
            let temp = caminfo.get_temperature().unwrap();
            let dtime: DateTime<Local> = SystemTime::now().into();
            print!("[{}] Camera temperature: {:>+05.1} C, Cooler Power: {:>03}\r", dtime.format("%H:%M:%S"), temp, caminfo.get_cooler_power().unwrap());
        }
    });
    cam.set_gain_raw(cfg.gain as i64).unwrap();
    cam.set_roi(&ROI{
        x_min: 300,
        y_min: 800,
        x_max: 2700,
        y_max: 2100,
        bin_x: 1,
        bin_y: 1
    }).unwrap();
    cam.set_image_fmt(ASIImageFormat::Image_RAW16).unwrap();
    cam.set_exposure(Duration::from_millis(100)).unwrap();
    while !done.load(Ordering::SeqCst)
    {
        let img = cam.capture_image().unwrap();
        let val: DateTime<Local> = SystemTime::now().into();
        let dir_prefix = Path::new(&cfg.savedir).join(val.format("%Y%m%d").to_string());
        if !dir_prefix.exists()
        {
            std::fs::create_dir_all(&dir_prefix).unwrap();
        }
        if img.save_fits(&dir_prefix, "comic", &cfg.progname, true, true).is_err()
        {
            println!("[{}] AERO: Error saving image", val.format("%H:%M:%S"));
        }
        else
        {
            println!("[{}] AERO: Saved image, exposure {:.3}", val.format("%H:%M:%S"), cam.get_exposure().as_secs_f32());
        }
        let (exposure, _bin) = img.find_optimum_exposure(cfg.percentile as f32, cfg.target_val as f32, cfg.target_uncertainty as f32, cam.get_min_exposure().unwrap_or(Duration::from_millis(1)), cfg.max_exposure, cfg.max_bin as u16, 100 as u32).unwrap();
        if exposure != cam.get_exposure() {
            cam.set_exposure(exposure).unwrap();
            println!("[{}] AERO: Exposure changed from {:.3} to {:.3}", val.format("%H:%M:%S"), cam.get_exposure().as_secs_f32(), exposure.as_secs_f32());
        }
        let val: SystemTime = val.into();
        if val < SystemTime::now()
        {
            sleep(SystemTime::now().duration_since(val).unwrap());
        }
    }
    camthread.join().unwrap();
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