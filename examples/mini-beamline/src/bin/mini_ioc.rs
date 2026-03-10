use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use asyn_rs::trace::TraceManager;
use epics_base_rs::error::CaResult;
use epics_base_rs::server::ioc_app::IocApplication;
use epics_base_rs::server::iocsh::registry::*;

use ad_core::plugin::channel::NDArrayOutput;
use ad_core::plugin::registry::ParamRegistry;

use motor_rs::builder::{MotorBuilder, MotorSetup};
use motor_rs::device_support::MotorDeviceSupport;
use motor_rs::sim_motor::SimMotor;

use mini_beamline::beam_current::{self, BeamCurrentValue};
use mini_beamline::beam_current::ioc_support::BeamCurrentDeviceSupport;
use mini_beamline::physics::{DetectorMode, BeamCurrentConfig, MovingDotImageConfig};
use mini_beamline::point_detector::{self, PointDetectorRuntime};
use mini_beamline::point_detector::ioc_support::{
    PointDetectorDeviceSupport,
    build_param_registry as build_pd_registry,
};
use mini_beamline::moving_dot::driver::{MovingDotRuntime, create_moving_dot_with_config};
use mini_beamline::moving_dot::ioc_support::{
    MovingDotDeviceSupport,
    build_param_registry as build_md_registry,
};

/// Read an f64 from an environment variable, returning `default` if unset or unparseable.
fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Read a u64 from an environment variable, returning `default` if unset or unparseable.
fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Read an i32 from an environment variable, returning `default` if unset or unparseable.
fn env_i32(name: &str, default: i32) -> i32 {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Motor info stored for device support wiring.
struct MotorInfo {
    device_support: Option<MotorDeviceSupport>,
}

/// Shared state between miniBeamlineConfig and device support factories.
struct BeamlineHolder {
    beam_value: Arc<BeamCurrentValue>,
    beam_rx: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    pd_runtimes: std::sync::Mutex<HashMap<String, (PointDetectorRuntime, Arc<ParamRegistry>)>>,
    md_runtime: std::sync::Mutex<Option<(MovingDotRuntime, Arc<ParamRegistry>)>>,
    motors: std::sync::Mutex<HashMap<String, MotorInfo>>,
    trace: Arc<TraceManager>,
}

impl BeamlineHolder {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            beam_value: Arc::new(BeamCurrentValue::new()),
            beam_rx: std::sync::Mutex::new(None),
            pd_runtimes: std::sync::Mutex::new(HashMap::new()),
            md_runtime: std::sync::Mutex::new(None),
            motors: std::sync::Mutex::new(HashMap::new()),
            trace: Arc::new(TraceManager::new()),
        })
    }
}

#[tokio::main]
async fn main() -> CaResult<()> {
    let args: Vec<String> = std::env::args().collect();

    if std::env::var_os("MINI_BEAMLINE").is_none() {
        unsafe { std::env::set_var("MINI_BEAMLINE", env!("CARGO_MANIFEST_DIR")) };
    }

    let script = if args.len() > 1 && !args[1].starts_with('-') {
        args[1].clone()
    } else {
        eprintln!("Usage: mini_ioc <st.cmd>");
        std::process::exit(1);
    };

    let holder = BeamlineHolder::new();

    let mut app = IocApplication::new();
    app = app.port(
        std::env::var("EPICS_CA_SERVER_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5064),
    );

    // ===== miniBeamlineConfig startup command =====
    // Creates beam current, point detectors, and moving dot.
    // Motors are created as inline records (below) since MotorRecord is special.
    {
        let h = holder.clone();
        app = app.register_startup_command(CommandDef::new(
            "miniBeamlineConfig",
            vec![],
            "miniBeamlineConfig - Configure mini beamline IOC",
            move |_args: &[ArgValue], _ctx: &CommandContext| {
                println!("miniBeamlineConfig: setting up beamline...");

                // 1. Start beam current thread (configurable via env)
                let beam_config = BeamCurrentConfig {
                    offset: env_f64("BEAM_OFFSET", 500.0),
                    amplitude: env_f64("BEAM_AMPLITUDE", 25.0),
                    period: env_f64("BEAM_PERIOD", 4.0),
                };
                let beam_interval = env_u64("BEAM_UPDATE_MS", 100);
                println!("  Beam config: offset={}, amp={}, period={}s, interval={}ms",
                    beam_config.offset, beam_config.amplitude, beam_config.period, beam_interval);

                let (_beam_jh, beam_rx) = beam_current::start_beam_current_thread(
                    h.beam_value.clone(), beam_config, beam_interval,
                );
                *h.beam_rx.lock().unwrap() = Some(beam_rx);
                println!("  Beam current thread started");

                // 2. Create 3 point detectors
                let pd_configs = [
                    ("PD_PH", DetectorMode::PinHole),
                    ("PD_EDGE", DetectorMode::Edge),
                    ("PD_SLIT", DetectorMode::Slit),
                ];
                for (port, mode) in &pd_configs {
                    let rt = point_detector::create_point_detector(port, *mode)
                        .map_err(|e| format!("failed to create PointDetector {port}: {e}"))?;
                    let registry = Arc::new(build_pd_registry(&rt.params));
                    let port_handle = rt.port_handle().clone();
                    asyn_rs::asyn_record::register_port(port, port_handle, h.trace.clone());
                    h.pd_runtimes.lock().unwrap().insert(port.to_string(), (rt, registry));
                    println!("  PointDetector '{port}' created");
                }

                // 3. Create 1 MovingDot detector (configurable via env)
                let dot_size_x = env_i32("DOT_SIZE_X", 640);
                let dot_size_y = env_i32("DOT_SIZE_Y", 480);
                let dot_max_mem = env_u64("DOT_MAX_MEMORY", 50_000_000) as usize;
                let dot_image_config = MovingDotImageConfig {
                    sigma_x: env_f64("DOT_SIGMA_X", 50.0),
                    sigma_y: env_f64("DOT_SIGMA_Y", 25.0),
                    background: env_f64("DOT_BACKGROUND", 1000.0),
                    n_per_i_per_s: env_f64("DOT_N_PER_I_PER_S", 200.0),
                };
                println!("  MovingDot config: {}x{}, sigma=({},{}), bg={}, N/I/s={}",
                    dot_size_x, dot_size_y,
                    dot_image_config.sigma_x, dot_image_config.sigma_y,
                    dot_image_config.background, dot_image_config.n_per_i_per_s);

                let dot_output = NDArrayOutput::new();
                let dot_rt = create_moving_dot_with_config(
                    "DOT", dot_size_x, dot_size_y, dot_max_mem, dot_output, dot_image_config,
                ).map_err(|e| format!("failed to create MovingDot: {e}"))?;
                let dot_registry = Arc::new(build_md_registry(&dot_rt.ad_params, &dot_rt.dot_params));
                let dot_handle = dot_rt.port_handle().clone();
                asyn_rs::asyn_record::register_port("DOT", dot_handle, h.trace.clone());
                *h.md_runtime.lock().unwrap() = Some((dot_rt, dot_registry));
                println!("  MovingDot detector created");

                println!("miniBeamlineConfig: done");
                Ok(CommandOutcome::Continue)
            },
        ));
    }

    // ===== Device support: beam current =====
    {
        let h = holder.clone();
        app = app.register_device_support("miniBeamCurrent", move || {
            let rx = h.beam_rx.lock().unwrap().take()
                .expect("miniBeamlineConfig must be called before iocInit");
            Box::new(BeamCurrentDeviceSupport::new(h.beam_value.clone(), rx))
        });
    }

    // ===== Device support: point detectors =====
    {
        let h = holder.clone();
        app = app.register_device_support("asynPointDet_PH", move || {
            let runtimes = h.pd_runtimes.lock().unwrap();
            let (rt, registry) = runtimes.get("PD_PH").expect("PD_PH not configured");
            Box::new(PointDetectorDeviceSupport::from_handle(
                rt.port_handle().clone(), registry.clone(), "asynPointDet_PH",
            ))
        });
    }
    {
        let h = holder.clone();
        app = app.register_device_support("asynPointDet_EDGE", move || {
            let runtimes = h.pd_runtimes.lock().unwrap();
            let (rt, registry) = runtimes.get("PD_EDGE").expect("PD_EDGE not configured");
            Box::new(PointDetectorDeviceSupport::from_handle(
                rt.port_handle().clone(), registry.clone(), "asynPointDet_EDGE",
            ))
        });
    }
    {
        let h = holder.clone();
        app = app.register_device_support("asynPointDet_SLIT", move || {
            let runtimes = h.pd_runtimes.lock().unwrap();
            let (rt, registry) = runtimes.get("PD_SLIT").expect("PD_SLIT not configured");
            Box::new(PointDetectorDeviceSupport::from_handle(
                rt.port_handle().clone(), registry.clone(), "asynPointDet_SLIT",
            ))
        });
    }

    // ===== Device support: moving dot =====
    {
        let h = holder.clone();
        app = app.register_device_support("asynMovingDot", move || {
            let guard = h.md_runtime.lock().unwrap();
            let (rt, registry) = guard.as_ref().expect("MovingDot not configured");
            Box::new(MovingDotDeviceSupport::from_handle(
                rt.port_handle().clone(), registry.clone(),
            ))
        });
    }

    // ===== Motor device support: dynamic dispatch =====
    {
        let h = holder.clone();
        app = app.register_dynamic_device_support(move |dtyp_name| {
            let mut motors = h.motors.lock().unwrap();
            if let Some(info) = motors.get_mut(dtyp_name) {
                if let Some(ds) = info.device_support.take() {
                    return Some(Box::new(ds) as Box<dyn epics_base_rs::server::device_support::DeviceSupport>);
                }
            }
            None
        });
    }

    // ===== Create motors as inline records (configurable via env) =====
    {
        let mtr_velo = env_f64("MOTOR_VELO", 1.0);
        let mtr_accl = env_f64("MOTOR_ACCL", 0.5);
        let mtr_hlm = env_f64("MOTOR_HLM", 100.0);
        let mtr_llm = env_f64("MOTOR_LLM", -100.0);
        let mtr_mres = env_f64("MOTOR_MRES", 0.001);
        let mtr_poll_ms = env_u64("MOTOR_POLL_MS", 100);

        let motor_defs = [
            ("mini:ph:mtr", "ph_mtr"),
            ("mini:edge:mtr", "edge_mtr"),
            ("mini:slit:mtr", "slit_mtr"),
            ("mini:dot:mtrx", "dot_mtrx"),
            ("mini:dot:mtry", "dot_mtry"),
        ];

        for (pv_name, motor_id) in &motor_defs {
            let motor: Arc<Mutex<dyn asyn_rs::interfaces::motor::AsynMotor>> =
                Arc::new(Mutex::new(SimMotor::new().with_limits(mtr_llm, mtr_hlm)));

            let setup = MotorBuilder::new(motor)
                .poll_interval(Duration::from_millis(mtr_poll_ms))
                .configure_record(move |rec| {
                    rec.vel.velo = mtr_velo;
                    rec.vel.accl = mtr_accl;
                    rec.limits.dhlm = mtr_hlm;
                    rec.limits.dllm = mtr_llm;
                    rec.conv.mres = mtr_mres;
                    rec.disp.prec = 4;
                })
                .build();

            let dtyp = format!("miniMotor_{motor_id}");
            let MotorSetup { record, device_support, poll_loop, poll_cmd_tx: _ } = setup;

            holder.motors.lock().unwrap().insert(dtyp.clone(), MotorInfo {
                device_support: Some(device_support),
            });

            // Spawn poll loop (runs for IOC lifetime)
            std::mem::forget(tokio::spawn(poll_loop.run()));

            app = app.record(pv_name, record);
        }
    }

    // ===== No-op commands =====
    for cmd in &["set_requestfile_path", "set_savefile_path", "startPVAServer"] {
        let name = *cmd;
        app = app.register_startup_command(CommandDef::new(
            name,
            vec![
                ArgDesc { name: "arg1", arg_type: ArgType::String, optional: true },
                ArgDesc { name: "arg2", arg_type: ArgType::String, optional: true },
            ],
            &format!("{name} [args...] - no-op"),
            move |_args: &[ArgValue], _ctx: &CommandContext| {
                Ok(CommandOutcome::Continue)
            },
        ));
    }

    app.startup_script(&script)
        .run()
        .await
}
