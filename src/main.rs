#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

extern crate espeak_sys;
extern crate xous_tts_backend;
use xous_tts_backend::*;
use xous::{SID, CID};
use xous_ipc::Buffer;

pub mod bindings;
pub use bindings::*;
mod logger;
use logger::*;

use num_traits::*;

static mut CB: Option<Callback> = None;

extern fn tts_cb(samples: *const c_ushort, count: c_int, _event: espeak_EVENT) -> i32 {
    if let Some(cb) = unsafe{CB} {
        // log::info!("cb: {}", count);
        let mut tts_data = TtsBackendData {
            data: [0u16; MAX_WAV_BUF_SAMPLES],
            len: count as u32,
            control: None,
        };
        if count > 0 {
            let samps: &[u16] = unsafe {
                core::slice::from_raw_parts::<u16>(samples, count as usize)
            };
            for (&src, dst) in samps.iter().zip(tts_data.data.iter_mut()) {
                *dst = src;
            }
        } else {
            tts_data.control = Some(TtsBeControl::End);
        }
        let buf = Buffer::into_buf(tts_data).expect("couldn't convert buffer");
        buf.lend(cb.cid, cb.op).expect("couldn't transmit memory message");
    }
    0
}

#[derive(Copy, Clone)]
struct Callback {
    pub sid: SID,
    pub cid: CID,
    pub op: u32,
    pub samples_per_cb: Option<u32>,
}

#[xous::xous_main]
fn xmain() -> ! {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Trace)).unwrap();
    log::info!("my PID is {}", xous::process::id());

    log::info!("registering with xous names");
    let xns = xous_names::XousNames::new().unwrap();

    let sid = xns.register_name(xous_tts_backend::SERVER_NAME_TTS_EXEC, None).expect("can't register server");
    log::info!("registered with NS -- {:?}", sid);

    loop {
        let msg = xous::receive_message(sid).unwrap();
            match FromPrimitive::from_usize(msg.body.id()) {
            Some(TtsBeOpcode::StrToWav) => {
                let buffer = unsafe { Buffer::from_memory_message(msg.body.memory_message().unwrap()) };
                let msg = buffer.to_original::<TtsBackendMsg, _>().unwrap();
                log::debug!("converting string {}", msg.text.as_str().unwrap());
                if unsafe{CB.is_some()} {
                    let msg_len = msg.text.len();
                    let text = msg.text.to_str().to_string();
                    let cstr = std::ffi::CString::new(text).expect("couldn't convert String to Cstring");
                    log::info!("espeak setup call");
                    unsafe {
                        espeak_ffi_setup(
                            tts_cb,
                        )
                    };
                    log::info!("espeak sample rate: {}", unsafe {espeak_ng_GetSampleRate()});
                    unsafe {
                        espeak_ffi_synth(
                        cstr.as_ptr(),
                        msg_len as c_uint,
                        ::core::ptr::null::<c_void>() as *mut c_void,
                    );}
                    log::info!("espeak sync");
                    unsafe {
                        espeak_ffi_sync();
                        espeak_ng_Terminate();
                    }
                    log::info!("espeak done");
                }
            },
            Some(TtsBeOpcode::RegisterCb) => {
                let buffer = unsafe { Buffer::from_memory_message(msg.body.memory_message().unwrap()) };
                let config = buffer.to_original::<TtsBackendConfig, _>().unwrap();
                unsafe {
                    CB = Some(Callback {
                        sid: SID::from_array(config.sid),
                        cid: xous::connect(SID::from_array(config.sid)).unwrap(),
                        op: config.op,
                        samples_per_cb: config.samples_per_cb,
                    });
                }
            },
            Some(TtsBeOpcode::Quit) => {
                log::warn!("server quitting");
                xous::return_scalar(msg.sender, 1).unwrap();
                break;
            }
            None => {
                log::error!("couldn't convert opcode: {:?}", msg);
            }
        }
    }
    // clean up our program
    log::trace!("main loop exit, destroying servers");
    xns.unregister_server(sid).unwrap();
    xous::destroy_server(sid).unwrap();
    log::trace!("quitting");
    xous::terminate_process(0)
}
