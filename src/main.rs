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

// TODO(Manishearth): Should support a set of constraints
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

struct MediaStreamConstraints {
    audio: Option<MediaTrackConstraintSet>,
    video: Option<MediaTrackConstraintSet>,
}

struct GstMediaDevices {
    monitor: DeviceMonitor,
}

impl GstMediaDevices {
    pub fn new() -> Self {
        Self {
            monitor: DeviceMonitor::new()
        }
    }

    fn get_track(&self, video: bool, constraints: MediaTrackConstraintSet) -> Option<GstMediaTrack> {
        let (format, filter) = if video { ("video/x-raw", "Video/Source") } else { ("audio/x-raw", "Audio/Source") };
        let caps = constraints.into_caps(format);
        println!("{:?}", caps);
        let f = self.monitor.add_filter(filter, &caps);
        let devices = self.monitor.get_devices();
        self.monitor.remove_filter(f);
        if let Some(d) = devices.get(0) {
            let element = d.create_element(None)?;
            Some(GstMediaTrack {
                element, video
            })
        } else {
            None
        }
    }

    pub fn get_user_media(&self, constraints: MediaStreamConstraints) -> GstMediaStream {
        GstMediaStream {
            video: constraints.video.and_then(|v| self.get_track(true, v)),
            audio: constraints.audio.and_then(|a| self.get_track(false, a))
        }
    }
}

struct GstMediaStream {
    video: Option<GstMediaTrack>,
    audio: Option<GstMediaTrack>,
}

struct GstMediaTrack {
    element: gst::Element,
    video: bool,
}

impl GstMediaTrack {
    fn play(&self) {
        let pipeline = gst::Pipeline::new(None);
        let (convert, sink) = if self.video {
            let convert = gst::ElementFactory::make("videoconvert", None).unwrap();
            let sink = gst::ElementFactory::make("autovideosink", None).unwrap();
            (convert, sink)
        } else {
            let convert = gst::ElementFactory::make("audioconvert", None).unwrap();
            let sink = gst::ElementFactory::make("autoaudiosink", None).unwrap();
            (convert, sink)
        };
        pipeline.add_many(&[&self.element, &convert, &sink]);
        gst::Element::link_many(&[&self.element, &convert, &sink]);
        pipeline.set_state(gst::State::Playing);
    }
}

fn main() {
    gstreamer::init();
    let main_loop = glib::MainLoop::new(None, false);
    let manager = GstMediaDevices::new();
    let av = manager.get_user_media(MediaStreamConstraints {
        // video: Some(MediaTrackConstraintSet { width: Some(Constrain::Value(1000000000)), .. Default::default() }),
        audio: Some(Default::default()),
        video: Some(Default::default()),
    });
    av.audio.map(|t| t.play());
    av.video.map(|t| t.play());
    main_loop.run();
}
