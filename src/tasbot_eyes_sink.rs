use glib::StaticType;

glib::wrapper! {
    pub struct TasbotEyesSink(ObjectSubclass<imp::TasbotEyesSink>) @extends gst_video::VideoSink, gst::Element, gst::Object;
}

impl TasbotEyesSink {
    pub fn new(name: Option<&str>) -> Result<TasbotEyesSink, glib::BoolError> {
        glib::Object::new(&[("name", &name)])
    }
}

pub(crate) fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "tasboteyessink",
        gst::Rank::None,
        TasbotEyesSink::static_type(),
    )
}

mod imp {
    use std::sync::Mutex;

    use glib::ToValue;
    use gst_video::subclass::prelude::*;
    use once_cell::sync::Lazy;

    use rppal::spi::Spi;

    struct State {
        spi: Spi,
        buffer: Vec<u8>,
    }

    #[derive(Default)]
    struct Settings {
        num_leds: usize,
    }

    #[derive(Default)]
    pub struct TasbotEyesSink {
        state: Mutex<Option<State>>,
        settings: Mutex<Settings>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TasbotEyesSink {
        const NAME: &'static str = "RsTasbotEyesSink";
        type Type = super::TasbotEyesSink;
        type ParentType = gst_video::VideoSink;
    }

    impl ObjectImpl for TasbotEyesSink {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<[glib::ParamSpec; 1]> = Lazy::new(|| {
                [glib::ParamSpecUInt64::builder("num-leds")
                    .default_value(24)
                    .minimum(1)
                    .build()]
            });

            &*PROPERTIES
        }

        fn set_property(
            &self,
            _element: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "num-leds" => {
                    let mut settings = self.settings.lock().unwrap();

                    settings.num_leds = value.get::<u64>().unwrap() as usize;
                }
                _ => unimplemented!(),
            }
        }

        fn property(
            &self,
            _element: &Self::Type,
            _id: usize,
            pspec: &glib::ParamSpec,
        ) -> glib::Value {
            match pspec.name() {
                "num-leds" => {
                    let settings = self.settings.lock().unwrap();

                    (settings.num_leds as u64).to_value()
                }
                _ => unimplemented!(),
            }
        }
    }

    impl GstObjectImpl for TasbotEyesSink {}

    impl ElementImpl for TasbotEyesSink {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
                gst::subclass::ElementMetadata::new(
                    "TASBot Eyes Sink",
                    "Video/Sink",
                    "GStreamer sink for TASBot's eye display",
                    "me (someone@example.com)",
                )
            });

            Some(&*ELEMENT_METADATA)
        }

        fn pad_templates() -> &'static [gst::PadTemplate] {
            static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
                // if the user wants a different format they can convert it themselves.
                let caps = gst::Caps::builder("video/x-raw")
                    .field("format", "RGB")
                    .build();

                vec![gst::PadTemplate::new(
                    "sink",
                    gst::PadDirection::Sink,
                    gst::PadPresence::Always,
                    &caps,
                )
                .unwrap()]
            });

            PAD_TEMPLATES.as_ref()
        }
    }

    impl BaseSinkImpl for TasbotEyesSink {
        fn caps(&self, element: &Self::Type, filter: Option<&gst::Caps>) -> Option<gst::Caps> {
            let result = self.parent_caps(element, filter);

            result
        }

        fn start(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
            let settings = self.settings.lock().unwrap();
            let mut state = self.state.lock().unwrap();

            let buffer = Vec::with_capacity(settings.num_leds * BITS_PER_BIT);

            let spi = Spi::new(
                rppal::spi::Bus::Spi0,
                rppal::spi::SlaveSelect::Ss0,
                6_400_000,
                rppal::spi::Mode::Mode0,
            )
            .unwrap();

            *state = Some(State { buffer, spi });

            self.parent_start(element)
        }

        fn stop(&self, _element: &Self::Type) -> Result<(), gst::ErrorMessage> {
            let mut state = self.state.lock().unwrap();

            std::mem::drop(state.take());

            Ok(())
        }
    }

    impl VideoSinkImpl for TasbotEyesSink {
        fn show_frame(
            &self,
            _element: &Self::Type,
            buffer: &gst::Buffer,
        ) -> Result<gst::FlowSuccess, gst::FlowError> {
            let map = buffer.map_readable().unwrap();
            let data = map.as_slice();

            let mut state = self.state.lock().unwrap();
            let state = state.as_mut().unwrap();

            let settings = self.settings.lock().unwrap();

            state.buffer.clear();
            state.buffer.extend(
                data.iter()
                    .take(settings.num_leds)
                    .flat_map(|&byte| convert_to_spi_format(byte)),
            );

            state.spi.write(state.buffer.as_slice()).unwrap();

            Ok(gst::FlowSuccess::Ok)
        }
    }

    const BITS_PER_BIT: usize = 8;

    const ZERO_BIT_PATTERN: u8 = 0b1000_0000;
    const ONE_BIT_PATTERN: u8 = 0b1111_0000;

    fn convert_to_spi_format(byte: u8) -> [u8; BITS_PER_BIT] {
        fn bit_to_spi_byte(byte: u8, bit: u8) -> u8 {
            if byte & (1 << (7 - bit)) > 0 {
                ONE_BIT_PATTERN
            } else {
                ZERO_BIT_PATTERN
            }
        }

        [
            bit_to_spi_byte(byte, 0),
            bit_to_spi_byte(byte, 1),
            bit_to_spi_byte(byte, 2),
            bit_to_spi_byte(byte, 3),
            bit_to_spi_byte(byte, 4),
            bit_to_spi_byte(byte, 5),
            bit_to_spi_byte(byte, 6),
            bit_to_spi_byte(byte, 7),
        ]
    }
}
