extern crate gstreamer;
extern crate glib;
use gstreamer as gst;
use gstreamer::{DeviceMonitor, DeviceMonitorExt, DeviceExt, ElementExt, BinExtManual};
use std::{u64, fmt, thread, time};

enum Constrain<T> {
    Value(T),
    Range(ConstrainRange<T>)
}

impl<T: Constrainable> Constrain<T> {
    fn into_caps_string(self, min: &str, max: &str) -> Option<String> {
        match self {
            Constrain::Value(v) => Some(format!("{}{}", T::PREFIX, v.into_caps_string()?)),
            Constrain::Range(r) => {
                let (t_min, t_max);
                let min = if let Some(m) = r.min {
                    t_min = m.into_caps_string();
                    t_min.as_ref().map(|t| &**t).unwrap_or(min)
                } else { min };
                let max = if let Some(m) = r.max {
                    t_max = m.into_caps_string();
                    t_max.as_ref().map(|t| &**t).unwrap_or(max)
                } else { max };
                if let Some(ideal) = r.ideal.and_then(|i| i.into_caps_string()) {
                    Some(format!("{}{{ {}, [{}, {}] }}",T::PREFIX, ideal, min, max))
                } else {
                    Some(format!("{}[{}, {}]",T::PREFIX, min, max))
                }
            }
        }
    }
}

trait Constrainable {
    const PREFIX: &'static str;
    fn into_caps_string(self) -> Option<String>;
}

impl Constrainable for u64 {
    const PREFIX: &'static str = "";
    fn into_caps_string(self) -> Option<String> {
        Some(self.to_string())
    }
}


impl Constrainable for f64 {
    const PREFIX: &'static str = "(fraction) ";
    fn into_caps_string(self) -> Option<String> {
        let f = gst::Fraction::approximate_f64(self)?;
        if self <= 0. {
            None
        } else {
            Some(format!("{}/{}", f.0.numer(), f.0.denom()))
        }
    }
}

struct ConstrainRange<T> {
    min: Option<T>,
    max: Option<T>,
    ideal: Option<T>,
}

struct ConstrainString {
    values: Vec<String>,
    ideal: Option<String>,
}

impl ConstrainString {
    fn into_caps_string(self) -> String {
        let mut values = self.values;
        if let Some(ideal) = self.ideal {
            values.insert(0, ideal);
        }
        format!("{:?}", values)
    }
}

enum ConstrainBool {
    Ideal(bool),
    Exact(bool),
}

#[derive(Default)]
struct MediaTrackConstraintSet {
    width: Option<Constrain<u64>>,
    height: Option<Constrain<u64>>,
    aspect: Option<Constrain<f64>>,
    frame_rate: Option<Constrain<f64>>,
    sample_rate: Option<Constrain<f64>>,
}

impl MediaTrackConstraintSet {
    fn into_caps(self, format: &str) -> gst::Caps {
        let mut caps: Vec<(&str, &dyn glib::ToSendValue)> = vec![];
        // temp values for extending lifetimes of strings
        let (tw, th, ta, tfr, tsr): (String, String, String, String, String);
        if let Some(w) = self.width.and_then(|v| v.into_caps_string("0", "100000000000000")) {
            tw = w;
            caps.push(("width", &tw));
        }
        if let Some(h) = self.height.and_then(|v| v.into_caps_string("0", "100000000000000")) {
            th = h;
            caps.push(("height", &th));
        }
        if let Some(aspect) = self.aspect.and_then(|v| v.into_caps_string("0/1", "10000/1")) {
            ta = aspect;
            caps.push(("pixel-aspect-ratio", &ta));
        }
        if let Some(fr) = self.frame_rate.and_then(|v| v.into_caps_string("0/1", "10000000/1")) {
            tfr = fr;
            caps.push(("framerate", &tfr));
        }
        if let Some(sr) = self.sample_rate.and_then(|v| v.into_caps_string("0/1", "10000000/1")) {
            tsr = sr;
            caps.push(("rate", &tsr));
        }
        gst::Caps::new_simple(format, &*caps)
    }
}

fn main() {
    gstreamer::init();
    let main_loop = glib::MainLoop::new(None, false);
    let mut monitor = DeviceMonitor::new();
    let caps = gst::Caps::new_simple(
        "audio/x-raw",
        &[],
    );


    let audio = monitor.add_filter("Audio/Source", &caps);
    let devices = monitor.get_devices();
    let device = &devices[0];
    println!("{:?}", device);
    println!("{:?}", device.get_caps());
    let element = device.create_element(None).unwrap();
    let convert = gst::ElementFactory::make("audioconvert", None).unwrap();
    let sink = gst::ElementFactory::make("autoaudiosink", None).unwrap();
    let pipeline = gst::Pipeline::new(None);
    pipeline.add_many(&[&element, &convert, &sink]);
    gst::Element::link_many(&[&element, &convert, &sink]);

    monitor.remove_filter(audio);
    let caps = gst::Caps::new_simple(
        "video/x-raw",
        &[],
    );
    monitor.add_filter("Video/Source", &caps);

    let devices = monitor.get_devices();
    print!("{:#?}", devices[0].get_caps());
    let element = devices[0].create_element(None).unwrap();

    let convert = gst::ElementFactory::make("videoconvert", None).unwrap();
    let sink = gst::ElementFactory::make("autovideosink", None).unwrap();
    pipeline.add_many(&[&element, &convert, &sink]);
    gst::Element::link_many(&[&element, &convert, &sink]);
    pipeline.set_state(gst::State::Playing);
    main_loop.run();
}
