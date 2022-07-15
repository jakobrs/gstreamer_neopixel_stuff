use glib::StaticType;

glib::wrapper! {
    pub struct FpsCounter(ObjectSubclass<imp::FpsCounter>) @extends gst_base::BaseTransform, gst::Element, gst::Object;
}

impl FpsCounter {
    pub fn new(name: Option<&str>) -> Result<FpsCounter, glib::BoolError> {
        glib::Object::new(&[("name", &name)])
    }
}

pub(crate) fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "fpscounter",
        gst::Rank::None,
        FpsCounter::static_type(),
    )
}

mod imp {
    use std::{
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc, Mutex,
        },
        thread::JoinHandle,
    };

    use glib::{subclass::prelude::*, ObjectExt};
    use gst::subclass::prelude::*;
    use gst_base::subclass::prelude::*;
    use once_cell::sync::Lazy;

    struct State {
        thread_handle: JoinHandle<()>,
    }

    #[derive(Default)]
    pub struct FpsCounter {
        state: Mutex<Option<State>>,
        counter: Arc<AtomicU64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FpsCounter {
        const NAME: &'static str = "RsFpsCounter";
        type Type = super::FpsCounter;
        type ParentType = gst_base::BaseTransform;
    }

    impl ObjectImpl for FpsCounter {}

    impl GstObjectImpl for FpsCounter {}

    impl ElementImpl for FpsCounter {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
                gst::subclass::ElementMetadata::new(
                    "FPS",
                    "Video/Filter",
                    "Determines fps",
                    "me (someone@example.com)",
                )
            });

            Some(&*ELEMENT_METADATA)
        }

        fn pad_templates() -> &'static [gst::PadTemplate] {
            static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
                let caps = gst::Caps::builder("video/x-raw").build();

                vec![
                    gst::PadTemplate::new(
                        "sink",
                        gst::PadDirection::Sink,
                        gst::PadPresence::Always,
                        &caps,
                    )
                    .unwrap(),
                    gst::PadTemplate::new(
                        "src",
                        gst::PadDirection::Src,
                        gst::PadPresence::Always,
                        &caps,
                    )
                    .unwrap(),
                ]
            });

            PAD_TEMPLATES.as_ref()
        }
    }

    impl BaseTransformImpl for FpsCounter {
        const MODE: gst_base::subclass::BaseTransformMode =
            gst_base::subclass::BaseTransformMode::AlwaysInPlace;
        const PASSTHROUGH_ON_SAME_CAPS: bool = true;
        const TRANSFORM_IP_ON_PASSTHROUGH: bool = true;

        fn start(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
            let counter = self.counter.clone();
            let name: String = element.property("name");

            let thread_handle = std::thread::spawn(move || loop {
                let now = std::time::Instant::now();
                std::thread::sleep(std::time::Duration::from_secs(5));

                let count = counter.swap(0, Ordering::SeqCst);
                let elapsed = now.elapsed().as_secs_f64();

                if count >= 1_000_000 {
                    break;
                }

                log::info!("Current FPS of {name}: {:?}", count as f64 / elapsed);
            });

            let mut state = self.state.lock().unwrap();

            *state = Some(State { thread_handle });

            Ok(())
        }

        fn stop(&self, _element: &Self::Type) -> Result<(), gst::ErrorMessage> {
            let mut state = self.state.lock().unwrap();

            if let Some(state) = state.take() {
                self.counter.store(1_000_000, Ordering::SeqCst);

                state.thread_handle.join().unwrap();
            }

            Ok(())
        }

        fn transform_ip_passthrough(
            &self,
            _element: &Self::Type,
            _buf: &gst::Buffer,
        ) -> Result<gst::FlowSuccess, gst::FlowError> {
            self.counter.fetch_add(1, Ordering::SeqCst);

            Ok(gst::FlowSuccess::Ok)
        }
    }
}
