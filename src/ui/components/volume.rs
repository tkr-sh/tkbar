use {
    crate::conf::CONFIG,
    gtk::{
        Box as GtkBox,
        EventControllerScroll,
        EventControllerScrollFlags,
        Label,
        glib,
        prelude::*,
    },
    gtk4::{self as gtk, GestureClick},
    pipewire::{
        channel::Receiver,
        context::ContextRc,
        device::{Device, DeviceListener},
        keys::{MEDIA_CLASS, NODE_NAME},
        main_loop::MainLoopRc,
        metadata::{Metadata, MetadataListener},
        node::{Node, NodeListener},
        registry::RegistryRc,
        spa::{
            self,
            param::ParamType,
            pod::{
                Pod,
                Property,
                PropertyFlags,
                Value,
                ValueArray,
                deserialize::PodDeserializer,
                serialize::PodSerializer,
            },
            utils::SpaTypes,
        },
        types::ObjectType,
    },
    std::{
        cell::RefCell,
        collections::HashMap,
        io::Cursor,
        rc::{Rc, Weak},
        sync::{Arc, Mutex},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct VolState {
    percent: u32,
    muted: bool,
    nb_channels: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct VolUpdate {
    percent: Option<u32>,
    muted: Option<bool>,
    nb_channels: Option<usize>,
}

pub fn volume() -> GtkBox {
    let container = GtkBox::new(CONFIG.style.position.orientation(), 2);
    container.add_css_class("volume");

    let icon = Label::new(Some("\u{f057f}"));
    icon.add_css_class("icon");
    icon.set_halign(gtk::Align::Center);

    let value = Label::new(Some("--"));
    value.add_css_class("value");
    value.set_halign(gtk::Align::Center);

    container.append(&icon);
    container.append(&value);

    let vol_state = Arc::new(Mutex::new(Option::<VolState>::None));
    let (tx, rx) = pipewire::channel::channel::<Cmd>();
    let (vol_update_tx, vol_update_rx) = async_channel::unbounded::<VolState>();


    launch_pipewire_thread(vol_state, rx, vol_update_tx);

    glib::spawn_future_local(glib::clone!(
        #[weak]
        container,
        #[weak]
        icon,
        #[weak]
        value,
        #[upgrade_or_default]
        async move {
            while let Ok(s) = vol_update_rx.recv().await {
                icon.set_text(icon_for(&s));
                if s.muted {
                    value.set_text("mute");
                    container.add_css_class("muted");
                } else {
                    value.set_text(&s.percent.to_string());
                    container.remove_css_class("muted");
                }
            }
        }
    ));


    let on_scroll = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    let on_scroll_tx = tx.clone();
    on_scroll.connect_scroll(move |_, _dx, dy| {
        if dy < 0.0 {
            let _ = on_scroll_tx.send(Cmd::IncVolume);
        } else {
            let _ = on_scroll_tx.send(Cmd::DecVolume);
        }
        glib::Propagation::Stop
    });

    let on_click = GestureClick::new();
    let on_click_tx = tx.clone();
    on_click.connect_released(move |_, _, _, _| {
        let _ = on_click_tx.send(Cmd::ToggleMute);
    });

    container.add_controller(on_scroll);
    container.add_controller(on_click);

    container
}

fn launch_pipewire_thread(
    vol_state: Arc<Mutex<Option<VolState>>>,
    rx: Receiver<Cmd>,
    vol_update_tx: async_channel::Sender<VolState>,
) {
    std::thread::spawn(move || pipewire_loop(vol_state, rx, &vol_update_tx));
}

fn pipewire_loop(
    vol_state: Arc<Mutex<Option<VolState>>>,
    rx: Receiver<Cmd>,
    vol_update_tx: &async_channel::Sender<VolState>,
) {
    pipewire::init();

    let Ok(main_loop) = MainLoopRc::new(None) else {
        crate::log::warn("volume", "failed to create the PipeWire main loop");
        return;
    };

    let Ok(context) = ContextRc::new(&main_loop, None) else {
        crate::log::warn("volume", "failed to create the PipeWire context");
        return;
    };

    let Ok(core) = context.connect_rc(None) else {
        crate::log::warn("volume", "failed to connect to PipeWire");
        return;
    };

    let Ok(registry) = core.get_registry_rc() else {
        crate::log::warn("volume", "failed to acquire the PipeWire registry");
        return;
    };

    let pw_state = Rc::new(RefCell::new(PwState::default()));


    let _attached = rx.attach(main_loop.loop_(), {
        let pw_state = Rc::clone(&pw_state);
        let vol_state = Arc::clone(&vol_state);
        let vol_update_tx = vol_update_tx.clone();
        move |cmd| {
            let pw = pw_state.borrow();

            let Some(name) = pw.default_sink_name.as_ref() else {
                return;
            };

            let Some(entry) = pw.sinks.get(name) else {
                return;
            };


            let new_vol_state = {
                let mut vol = vol_state.lock().expect("mutex poisoned");
                let step = u32::from(CONFIG.behaviour.on_scroll_volume_step);
                let Some(state) = vol.as_mut() else {
                    return;
                };

                match cmd {
                    Cmd::IncVolume => state.percent = state.percent.saturating_add(step).min(255),
                    Cmd::DecVolume => state.percent = state.percent.saturating_sub(step),
                    Cmd::ToggleMute => state.muted = !state.muted,
                }

                *state
            };

            let _ = vol_update_tx.try_send(new_vol_state);

            let route_target = entry.route.and_then(|route| {
                let device = pw.devices.get(&entry.device_id?)?;
                Some((Rc::clone(&device.device), route))
            });

            if let Some((device, route)) = route_target {
                let bytes = route_pod(&route, new_vol_state);
                let pod = Pod::from_bytes(&bytes).expect("route pod is valid");
                device.set_param(ParamType::Route, 0, pod);
            } else {
                match cmd {
                    Cmd::IncVolume | Cmd::DecVolume => {
                        let bytes = volume_pod(new_vol_state.percent, new_vol_state.nb_channels);
                        let pod =
                            Pod::from_bytes(&bytes).expect("volume_pod output is a valid pod");
                        entry.node.set_param(ParamType::Props, 0, pod);
                    },
                    Cmd::ToggleMute => {
                        let bytes = mute_pod(new_vol_state.muted);
                        let pod = Pod::from_bytes(&bytes).expect("mute_pod output is a valid pod");
                        entry.node.set_param(ParamType::Props, 0, pod);
                    },
                }
            }
        }
    });

    let _reg_listener = registry
        .add_listener_local()
        .global({
            let pw_state_weak = Rc::downgrade(&pw_state);
            let registry_weak = RegistryRc::downgrade(&registry);
            let vol_update_tx = vol_update_tx.clone();
            move |obj| {
                match obj.type_ {
                    ObjectType::Node => {
                        let Some(props) = obj.props else { return };

                        if props.get(*MEDIA_CLASS) != Some("Audio/Sink") {
                            return;
                        }

                        let Some(name) = props.get(*NODE_NAME).map(str::to_owned) else {
                            return;
                        };

                        let Some(registry) = registry_weak.upgrade() else {
                            return;
                        };

                        let node: Node = match registry.bind(obj) {
                            Ok(n) => n,
                            Err(_) => return,
                        };

                        let listener = node
                            .add_listener_local()
                            .info({
                                let pw_state_weak = Weak::clone(&pw_state_weak);
                                let name = name.clone();
                                move |info| {
                                    let Some(props) = info.props() else { return };
                                    let device_id = props
                                        .get("device.id")
                                        .and_then(|id| id.parse::<u32>().ok());
                                    let route_device = props
                                        .get("card.profile.device")
                                        .and_then(|dev| dev.parse::<i32>().ok());
                                    let Some(state) = pw_state_weak.upgrade() else {
                                        return;
                                    };
                                    let device = {
                                        let mut pw = state.borrow_mut();
                                        let Some(sink) = pw.sinks.get_mut(&name) else {
                                            return;
                                        };
                                        sink.device_id = device_id.or(sink.device_id);
                                        sink.route_device = route_device.or(sink.route_device);
                                        device_id.and_then(|id| {
                                            pw.devices.get(&id).map(|d| Rc::clone(&d.device))
                                        })
                                    };
                                    if let Some(device) = device {
                                        device.enum_params(0, Some(ParamType::Route), 0, u32::MAX);
                                    }
                                }
                            })
                            .param({
                                let vol_state_weak = Arc::downgrade(&vol_state);
                                let vol_update_tx = vol_update_tx.clone();
                                move |_seq, id, _index, _next, param| {
                                    if id != ParamType::Props {
                                        return;
                                    }

                                    let Some(param) = param else { return };
                                    let Some(update) = VolUpdate::parse(param) else {
                                        return;
                                    };

                                    if let Some(state) = vol_state_weak.upgrade() {
                                        let snapshot = {
                                            let mut guard = state.lock().expect("mutex poisoned");
                                            let current =
                                                guard.get_or_insert_with(VolState::default);

                                            current.apply_update(update);

                                            *current
                                        };


                                        let _ = vol_update_tx.try_send(snapshot);
                                    }
                                }
                            })
                            .register();

                        node.subscribe_params(&[ParamType::Props]);
                        node.enum_params(0, Some(ParamType::Props), 0, u32::MAX);

                        let device_id =
                            props.get("device.id").and_then(|id| id.parse::<u32>().ok());
                        let route_device = props
                            .get("card.profile.device")
                            .and_then(|dev| dev.parse::<i32>().ok());

                        if let Some(state) = pw_state_weak.upgrade() {
                            state.borrow_mut().sinks.insert(
                                name,
                                SinkEntry {
                                    id: obj.id,
                                    node,
                                    device_id,
                                    route_device,
                                    route: None,
                                    _listener: listener,
                                },
                            );
                        }
                    },
                    ObjectType::Device => {
                        let Some(props) = obj.props else { return };

                        if props.get(*MEDIA_CLASS) != Some("Audio/Device") {
                            return;
                        }

                        let Some(registry) = registry_weak.upgrade() else {
                            return;
                        };

                        let device: Device = match registry.bind(obj) {
                            Ok(d) => d,
                            Err(_) => return,
                        };

                        let device_id = obj.id;

                        let listener = device
                            .add_listener_local()
                            .param({
                                let pw_state_weak = Weak::clone(&pw_state_weak);
                                let vol_state_weak = Arc::downgrade(&vol_state);
                                let vol_update_tx = vol_update_tx.clone();
                                move |_seq, id, _index, _next, param| {
                                    if id != ParamType::Route {
                                        return;
                                    }
                                    let Some(param) = param else { return };
                                    let Some(event) = RouteEvent::parse(param) else {
                                        return;
                                    };
                                    let Some(state) = pw_state_weak.upgrade() else {
                                        return;
                                    };

                                    let mut update = None;
                                    {
                                        let mut pw = state.borrow_mut();
                                        let default = pw.default_sink_name.clone();
                                        for (name, sink) in &mut pw.sinks {
                                            if sink.device_id != Some(device_id) ||
                                                sink.route_device != Some(event.device)
                                            {
                                                continue;
                                            }
                                            sink.route = Some(RouteRef {
                                                index: event.index,
                                                device: event.device,
                                                nb_channels: event.channels,
                                            });
                                            if Some(name) == default.as_ref() {
                                                update = Some(event.update);
                                            }
                                        }
                                    }

                                    if let (Some(update), Some(vol_state)) =
                                        (update, vol_state_weak.upgrade())
                                    {
                                        let snapshot = {
                                            let mut guard =
                                                vol_state.lock().expect("mutex poisoned");
                                            let current =
                                                guard.get_or_insert_with(VolState::default);
                                            current.apply_update(update);
                                            *current
                                        };
                                        let _ = vol_update_tx.try_send(snapshot);
                                    }
                                }
                            })
                            .register();

                        device.subscribe_params(&[ParamType::Route]);
                        device.enum_params(0, Some(ParamType::Route), 0, u32::MAX);

                        if let Some(state) = pw_state_weak.upgrade() {
                            state.borrow_mut().devices.insert(
                                device_id,
                                DeviceEntry {
                                    device: Rc::new(device),
                                    _listener: listener,
                                },
                            );
                        }
                    },
                    ObjectType::Metadata => {
                        let Some(props) = obj.props else { return };

                        if props.get("metadata.name") != Some("default") {
                            return;
                        }

                        let Some(registry) = registry_weak.upgrade() else {
                            return;
                        };

                        let metadata: Metadata = match registry.bind(obj) {
                            Ok(m) => m,
                            Err(_) => return,
                        };

                        let listener = metadata
                            .add_listener_local()
                            .property({
                                let pw_state_weak = Weak::clone(&pw_state_weak);
                                move |_subject, key, _type, value| {
                                    if key == Some("default.audio.sink") &&
                                        let Some(state) = pw_state_weak.upgrade()
                                    {
                                        let name = value.and_then(parse_sink_name);
                                        state.borrow_mut().default_sink_name =
                                            name.map(ToOwned::to_owned);
                                    }
                                    0
                                }
                            })
                            .register();
                        if let Some(state) = pw_state_weak.upgrade() {
                            state.borrow_mut()._metadata = Some((metadata, listener));
                        }
                    },
                    _ => {},
                }
            }
        })
        .global_remove({
            let pw_state_weak = Rc::downgrade(&pw_state);
            move |id| {
                if let Some(state) = pw_state_weak.upgrade() {
                    let mut pw = state.borrow_mut();
                    pw.sinks.retain(|_, e| e.id != id);
                    pw.devices.remove(&id);
                }
            }
        })
        .register();

    main_loop.run();
}


fn volume_pod(percent: u32, channels: usize) -> Vec<u8> {
    let cubic = percent_to_cubic(percent);

    let obj = spa::pod::Value::Object(spa::pod::Object {
        type_: SpaTypes::ObjectParamProps.as_raw(),
        id: ParamType::Props.as_raw(),
        properties: vec![Property {
            key: spa::sys::SPA_PROP_channelVolumes,
            flags: PropertyFlags::empty(),
            value: spa::pod::Value::ValueArray(ValueArray::Float(vec![cubic; channels.max(1)])),
        }],
    });

    let (cursor, _len) =
        PodSerializer::serialize(Cursor::new(Vec::new()), &obj).expect("volume props serialize");

    cursor.into_inner()
}

fn mute_pod(is_muted: bool) -> Vec<u8> {
    let obj = spa::pod::Value::Object(spa::pod::Object {
        type_: SpaTypes::ObjectParamProps.as_raw(),
        id: ParamType::Props.as_raw(),
        properties: vec![Property {
            key: spa::sys::SPA_PROP_mute,
            flags: PropertyFlags::empty(),
            value: spa::pod::Value::Bool(is_muted),
        }],
    });

    let (cursor, _len) =
        PodSerializer::serialize(Cursor::new(Vec::new()), &obj).expect("mute props serialize");

    cursor.into_inner()
}

fn cubic_to_percent(cubic: f32) -> f32 {
    cubic.cbrt() * 100.0
}

#[allow(clippy::as_conversions, reason = "u32 to f32 and clamped")]
fn percent_to_cubic(percent: u32) -> f32 {
    (percent.clamp(0, 255) as f32 / 100.0).powi(3)
}

#[derive(Default)]
struct PwState {
    default_sink_name: Option<String>,
    sinks: HashMap<String, SinkEntry>,
    devices: HashMap<u32, DeviceEntry>,
    // keeps alive
    _metadata: Option<(Metadata, MetadataListener)>,
}

struct SinkEntry {
    id: u32,
    node: Node,
    device_id: Option<u32>,
    route_device: Option<i32>,
    route: Option<RouteRef>,
    // keeps alive
    _listener: NodeListener,
}

#[derive(Clone, Copy)]
struct RouteRef {
    index: i32,
    device: i32,
    nb_channels: usize,
}

struct DeviceEntry {
    device: Rc<Device>,
    // keeps alive
    _listener: DeviceListener,
}



enum Cmd {
    IncVolume,
    DecVolume,
    ToggleMute,
}


fn parse_sink_name(value: &str) -> Option<&str> {
    let mut iter = value.split('"');

    iter.find(|t| *t == "name")?;

    if iter.next().is_none_or(|s| !s.contains(':')) {
        return None;
    }

    iter.next()
}

impl VolState {
    const fn apply_update(&mut self, update: VolUpdate) {
        if let Some(percent) = update.percent {
            self.percent = percent;
        }

        if let Some(muted) = update.muted {
            self.muted = muted;
        }

        if let Some(nb_channels) = update.nb_channels {
            self.nb_channels = nb_channels;
        }
    }
}

impl VolUpdate {
    fn parse(pod: &Pod) -> Option<VolUpdate> {
        let (_rest, value) = PodDeserializer::deserialize_from::<Value>(pod.as_bytes()).ok()?;
        let Value::Object(obj) = value else {
            return None;
        };

        Self::from_properties(&obj.properties)
    }

    fn from_properties(properties: &[Property]) -> Option<VolUpdate> {
        let mut nb_channels: Option<usize> = None;
        let mut channels: Option<f32> = None;
        let mut overall: Option<f32> = None;
        let mut muted: Option<bool> = None;

        for prop in properties {
            if prop.key == spa::sys::SPA_PROP_channelVolumes {
                if let Value::ValueArray(ValueArray::Float(v)) = &prop.value {
                    nb_channels = Some(v.len());
                    channels = v
                        .iter()
                        .copied()
                        .fold(None, |acc, x| Some(acc.map_or(x, |a| a.max(x))));
                }
            } else if prop.key == spa::sys::SPA_PROP_volume &&
                let Value::Float(f) = &prop.value
            {
                overall = Some(*f);
            } else if prop.key == spa::sys::SPA_PROP_mute &&
                let Value::Bool(m) = &prop.value
            {
                muted = Some(*m);
            }
        }

        #[allow(clippy::as_conversions, reason = "f32 to u32, and clamped")]
        let percent = channels
            .or(overall)
            .map(cubic_to_percent)
            .map(|perc| perc.clamp(0., 255.).round() as u32);

        (percent.is_some() || muted.is_some() || nb_channels.is_some()).then_some(VolUpdate {
            percent,
            muted,
            nb_channels,
        })
    }
}

struct RouteEvent {
    index: i32,
    device: i32,
    channels: usize,
    update: VolUpdate,
}

impl RouteEvent {
    fn parse(pod: &Pod) -> Option<RouteEvent> {
        let (_rest, value) = PodDeserializer::deserialize_from::<Value>(pod.as_bytes()).ok()?;
        let Value::Object(obj) = value else {
            return None;
        };

        let mut index = None;
        let mut device = None;
        let mut props = None;

        for prop in &obj.properties {
            if prop.key == spa::sys::SPA_PARAM_ROUTE_index {
                if let Value::Int(i) = &prop.value {
                    index = Some(*i);
                }
            } else if prop.key == spa::sys::SPA_PARAM_ROUTE_device {
                if let Value::Int(i) = &prop.value {
                    device = Some(*i);
                }
            } else if prop.key == spa::sys::SPA_PARAM_ROUTE_props &&
                let Value::Object(o) = &prop.value
            {
                props = Some(o);
            }
        }

        let props = props?;
        let channels = props
            .properties
            .iter()
            .find_map(|prop| {
                if prop.key == spa::sys::SPA_PROP_channelVolumes &&
                    let Value::ValueArray(ValueArray::Float(v)) = &prop.value
                {
                    Some(v.len())
                } else {
                    None
                }
            })
            .unwrap_or(2);

        Some(RouteEvent {
            index: index?,
            device: device?,
            channels: channels.max(1),
            update: VolUpdate::from_properties(&props.properties)?,
        })
    }
}

const fn icon_for(state: &VolState) -> &'static str {
    if state.muted {
        "󰝟"
    } else {
        match state.percent {
            0..=20 => "󰕿",
            21..=50 => "󰖀",
            51..=100 => "󰕾",
            _ => "󱄡",
        }
    }
}

fn route_pod(route: &RouteRef, state: VolState) -> Vec<u8> {
    let cubic = percent_to_cubic(state.percent);

    let obj = Value::Object(spa::pod::Object {
        type_: SpaTypes::ObjectParamRoute.as_raw(),
        id: ParamType::Route.as_raw(),
        properties: vec![
            Property {
                key: spa::sys::SPA_PARAM_ROUTE_index,
                flags: PropertyFlags::empty(),
                value: Value::Int(route.index),
            },
            Property {
                key: spa::sys::SPA_PARAM_ROUTE_device,
                flags: PropertyFlags::empty(),
                value: Value::Int(route.device),
            },
            Property {
                key: spa::sys::SPA_PARAM_ROUTE_props,
                flags: PropertyFlags::empty(),
                value: Value::Object(spa::pod::Object {
                    type_: SpaTypes::ObjectParamProps.as_raw(),
                    id: ParamType::Props.as_raw(),
                    properties: vec![
                        Property {
                            key: spa::sys::SPA_PROP_channelVolumes,
                            flags: PropertyFlags::empty(),
                            value: Value::ValueArray(ValueArray::Float(vec![
                                cubic;
                                route
                                    .nb_channels
                                    .max(1)
                            ])),
                        },
                        Property {
                            key: spa::sys::SPA_PROP_mute,
                            flags: PropertyFlags::empty(),
                            value: Value::Bool(state.muted),
                        },
                    ],
                }),
            },
            Property {
                key: spa::sys::SPA_PARAM_ROUTE_save,
                flags: PropertyFlags::empty(),
                value: Value::Bool(true),
            },
        ],
    });

    let (cursor, _len) =
        PodSerializer::serialize(Cursor::new(Vec::new()), &obj).expect("route pod serialize");

    cursor.into_inner()
}
