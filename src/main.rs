use anyhow::Result;
use clap::Parser;
use gst::prelude::*;

mod fps_counter;
mod tasbot_eyes_sink;

#[derive(Parser)]
struct Opts {
    #[clap(long, default_value_t = 24)]
    num_leds: u64,

    #[clap(long)]
    target_fps: Option<i32>,

    #[clap(long)]
    queue_leaky: Option<String>,
}

fn main() -> Result<()> {
    env_logger::init();
    gst::init()?;

    let opts = Opts::parse();

    let pipeline = gst::Pipeline::new(None);

    let videotestsrc = gst::ElementFactory::make("videotestsrc", None)?;
    let capsfilter = gst::ElementFactory::make("capsfilter", None)?;
    let gammafilter = gst::ElementFactory::make("gamma", None)?;
    let fpscounter1 = fps_counter::FpsCounter::new(Some("fps_before_queue"))?;
    let queue = gst::ElementFactory::make("queue", None)?;
    let fpscounter2 = fps_counter::FpsCounter::new(Some("fps_after_queue"))?;
    let tassink = tasbot_eyes_sink::TasbotEyesSink::new(None)?;

    if let Some(target_fps) = opts.target_fps {
        capsfilter.set_property(
            "caps",
            &gst::Caps::builder("video/x-raw")
                .field("framerate", &gst::Fraction::new(target_fps, 1))
                .build(),
        );
    }
    gammafilter.set_property("gamma", 2.0);
    if let Some(queue_leaky) = opts.queue_leaky {
        queue.set_property_from_str("leaky", &queue_leaky);
    }
    tassink.set_property("num-leds", opts.num_leds);
    // rpisink.set_property("map", "todo"); // not implemented yet

    pipeline.add_many(&[
        &videotestsrc,
        &capsfilter,
        &gammafilter,
        fpscounter1.upcast_ref(),
        &queue,
        fpscounter2.upcast_ref(),
        tassink.upcast_ref(),
    ])?;
    gst::Element::link_many(&[
        &videotestsrc,
        &capsfilter,
        &gammafilter,
        fpscounter1.upcast_ref(),
        &queue,
        fpscounter2.upcast_ref(),
        tassink.upcast_ref(),
    ])?;

    log::info!("Setting state of pipeline to PLAYING");
    pipeline.set_state(gst::State::Playing)?;
    log::info!("Set state of pipeline to PLAYING");

    let bus = pipeline.bus().unwrap();

    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(err) => {
                log::error!("{err:?}");

                break;
            }
            gst::MessageView::StateChanged(st) => log::info!(
                "State changed: {}: {:?} -> {:?}",
                st.src().unwrap(),
                st.old(),
                st.current(),
            ),
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;

    Ok(())
}
