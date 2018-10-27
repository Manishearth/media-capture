extern crate gstreamer;
extern crate glib;
use gstreamer as gst;
use gstreamer::{DeviceMonitor, DeviceMonitorExt, DeviceExt, ElementExt, BinExtManual};
use gstreamer::{Fraction, FractionRange, List, IntRange};
use gstreamer::caps::{Builder, Caps};
use std::{i32, fmt, thread, time};

enum Constrain<T> {
    Value(T),
    Range(ConstrainRange<T>)
}

impl Constrain<u64> {
    fn add_to_caps(self, name: &str, min: u64, max: u64, builder: Builder) -> Option<Builder> {
        match self {
            Constrain::Value(v) => Some(builder.field(name, &(v as i64 as i32))),
            Constrain::Range(r) => {
                let min = into_i32(r.min.unwrap_or(min));
                let max = into_i32(r.max.unwrap_or(max));
                let range = IntRange::<i32>::new(min, max);
                if let Some(ideal) = r.ideal {
                    let ideal = into_i32(ideal);
                    let array = List::new(&[&ideal, &range]);
                    Some(builder.field(name, &array))
                } else {
                    Some(builder.field(name, &range))
                }
            }
        }
    }
}

fn into_i32(x: u64) -> i32 {
    if x > i32::MAX as u64 {
        i32::MAX
    } else {
        x as i64 as i32
    }
}

impl Constrain<f64> {
    fn add_to_caps(self, name: &str, min: i32, max: i32, builder: Builder) -> Option<Builder> {
        match self {
            Constrain::Value(v) => Some(builder.field("name", &Fraction::approximate_f64(v)?)),
            Constrain::Range(r) => {
                let min = r.min.and_then(|v| Fraction::approximate_f64(v)).unwrap_or(Fraction::new(min, 1));
                let max = r.max.and_then(|v| Fraction::approximate_f64(v)).unwrap_or(Fraction::new(max, 1));
                let range = FractionRange::new(min, max);
                if let Some(ideal) = r.ideal.and_then(|v| Fraction::approximate_f64(v)) {
                    let array = gst::List::new(&[&ideal, &range]);
                    Some(builder.field(name, &array))
                } else {
                    Some(builder.field(name, &range))
                }
            }
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
    fn into_caps(self, format: &str) -> Option<gst::Caps> {
        let mut builder = Caps::builder(format);
        if let Some(w) = self.width {
            builder = w.add_to_caps("width", 0, 1000000, builder)?;
        }
        if let Some(h) = self.height {
            builder = h.add_to_caps("height", 0, 1000000, builder)?;
        }
        if let Some(aspect) = self.aspect {
            builder = aspect.add_to_caps("pixel-aspect-ratio", 0, 1000000, builder)?;
        }
        if let Some(fr) = self.frame_rate {
            builder = fr.add_to_caps("framerate", 0, 1000000, builder)?;
        }
        if let Some(sr) = self.sample_rate {
            builder = sr.add_to_caps("rate", 0, 1000000, builder)?;
        }
        Some(builder.build())
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
        let caps = constraints.into_caps(format)?;
        println!("requesting {:?}", caps);
        let f = self.monitor.add_filter(filter, &caps);
        let devices = self.monitor.get_devices();
        self.monitor.remove_filter(f);
        if let Some(d) = devices.get(0) {
            println!("{:?}", d.get_caps());
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
        video: Some(MediaTrackConstraintSet { width: Some(Constrain::Range(ConstrainRange {min: Some(100), max: Some(1000), ideal: Some(800)})), .. Default::default() }),
        audio: Some(Default::default()),
        // video: Some(Default::default()),
    });
    av.audio.map(|t| t.play());
    av.video.map(|t| t.play());
    main_loop.run();
}
