extern crate gstreamer;
extern crate glib;
use gstreamer as gst;
use gstreamer::{DeviceMonitor, DeviceMonitorExt, DeviceExt, ElementExt, BinExtManual};
use std::{thread, time};


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

    let element = monitor.get_devices()[0].create_element(None).unwrap();

    let convert = gst::ElementFactory::make("videoconvert", None).unwrap();
    let sink = gst::ElementFactory::make("autovideosink", None).unwrap();
    pipeline.add_many(&[&element, &convert, &sink]);
    gst::Element::link_many(&[&element, &convert, &sink]);
    pipeline.set_state(gst::State::Playing);
    main_loop.run();
}
