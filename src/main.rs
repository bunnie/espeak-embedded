#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

extern crate espeak_sys;
extern crate xous_tts_backend;
use xous_tts_backend::*;
use xous::{SID, CID, send_message, Message};
use xous_ipc::Buffer;

pub mod bindings;
pub use bindings::*;
mod logger;
use logger::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use num_traits::*;

static mut CB: Option<Callback> = None;
static TTS_RUNNING: AtomicBool = AtomicBool::new(false);
static TTS_SHOULD_ABORT: AtomicBool = AtomicBool::new(false);


/*
   The callback function is of the form:

int SynthCallback(short *wav, int numsamples, espeak_EVENT *events);

   wav:  is the speech sound data which has been produced.
      NULL indicates that the synthesis has been completed.

   numsamples: is the number of entries in wav.  This number may vary, may be less than
      the value implied by the buflength parameter given in espeak_Initialize, and may
      sometimes be zero (which does NOT indicate end of synthesis).

   events: an array of espeak_EVENT items which indicate word and sentence events, and
      also the occurrence if <mark> and <audio> elements within the text.  The list of
      events is terminated by an event of type = 0.

   Callback returns: 0=continue synthesis,  1=abort synthesis.
*/
extern fn tts_cb(samples: *const c_ushort, count: c_int, _event: espeak_EVENT) -> i32 {
    if let Some(cb) = unsafe{CB} {
        let mut tts_data = TtsBackendData {
            data: [0u16; MAX_WAV_BUF_SAMPLES],
            len: count as u32,
            control: None,
        };
        if samples != ::core::ptr::null::<c_ushort>() {
            if count > 0 {
                let samps: &[u16] = unsafe {
                    core::slice::from_raw_parts::<u16>(samples, count as usize)
                };
                for (&src, dst) in samps.iter().zip(tts_data.data.iter_mut()) {
                    *dst = src;
                }
            } else {
                // we just got a 0-count packet that is probably a sentence metadata event. ignore and move on
            }
        } else {
            // wave is null, which means we hit the end of synthesis
            tts_data.control = Some(TtsBeControl::End);
        }
        // check to see if we should be aborting synthesis, otherwise move on.
        if TTS_SHOULD_ABORT.load(Ordering::SeqCst) {
            // this will override the End signal, but I think that's OK if we Abort in case of an End, they are ultimately the same path
            tts_data.control = Some(TtsBeControl::Abort);
        }
        if count > 0 || tts_data.control.is_some() {
            // only generate a message if we have some data to send, or a control state update
            let buf = Buffer::into_buf(tts_data).expect("couldn't convert buffer");
            buf.lend(cb.cid, cb.op).expect("couldn't transmit memory message");
        }
        match tts_data.control {
            None => 0, // keep synthesizing if no error codes are set
            _ => 1, // abort or end synthesis by returning 1
        }
    } else {
        // even if we have no CB set, check for an abort signal and pass it on
        if TTS_SHOULD_ABORT.load(Ordering::SeqCst) {
            1 // abort synthesis
        } else {
            0 // continue synthensis
        }
    }
}

#[derive(Copy, Clone)]
struct Callback {
    // the sid field probably won't ever be used, but we keep it around because it's impossible to recover once lost
    pub _sid: SID,
    pub cid: CID,
    pub op: u32,
    #[allow(dead_code)] // for now this is a reserved field
    pub samples_per_cb: Option<u32>,
}

#[derive(num_derive::FromPrimitive, num_derive::ToPrimitive, Debug)]
pub enum SynthOp {
    /// New string for synthesis
    NewString,
    /// Exit server
    Quit,
}

#[xous::xous_main]
fn xmain() -> ! {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Info)).unwrap();
    log::info!("my PID is {}", xous::process::id());

    let xns = xous_names::XousNames::new().unwrap();

    let sid = xns.register_name(xous_tts_backend::SERVER_NAME_TTS_EXEC, None).expect("can't register server");
    log::trace!("registered with NS -- {:?}", sid);

    // put the synthesizer in its own thread
    let synth_sid = xous::create_server().unwrap();
    let synth_cid = xous::connect(synth_sid).unwrap();
    let synth_string = Arc::new(Mutex::new(String::new()));
    std::thread::spawn({
        let synth_string = synth_string.clone();
        move || {
            loop {
                let msg = xous::receive_message(synth_sid).unwrap();
                match FromPrimitive::from_usize(msg.body.id()) {
                    Some(SynthOp::NewString) => {
                        if unsafe{CB.is_some()} {
                            // ASSUME: the caller set the TTS_RUNNING lock before making the call
                            let text = synth_string.lock().unwrap().clone();
                            let msg_len = text.len();
                            log::debug!("espeak synth: {}", &text);
                            let cstr = std::ffi::CString::new(text).expect("couldn't convert String to Cstring");
                            unsafe {
                                espeak_ffi_setup(
                                    tts_cb,
                                )
                            };
                            log::debug!("espeak sample rate: {}", unsafe {espeak_ng_GetSampleRate()});
                            unsafe {
                                espeak_ffi_synth(
                                cstr.as_ptr(),
                                msg_len as c_uint,
                                ::core::ptr::null::<c_void>() as *mut c_void,
                            );}
                            log::trace!("espeak sync");
                            unsafe {
                                espeak_ffi_sync();
                                espeak_ng_Terminate();
                            }
                            log::trace!("espeak done");
                            TTS_RUNNING.store(false, Ordering::SeqCst);
                        }
                    }
                    Some(SynthOp::Quit) => {
                        xous::return_scalar(msg.sender, 1).unwrap();
                        break;
                    }
                    None => log::warn!("couldn't interpret opcode: {:?}", msg),
                }
            }
            xous::destroy_server(synth_sid).ok();
        }
    });

    loop {
        let msg = xous::receive_message(sid).unwrap();
        match FromPrimitive::from_usize(msg.body.id()) {
            Some(TtsBeOpcode::StrToWav) => {
                let buffer = unsafe { Buffer::from_memory_message(msg.body.memory_message().unwrap()) };
                let msg = buffer.to_original::<TtsBackendMsg, _>().unwrap();
                log::debug!("outer processing for string {}", msg.text.as_str().unwrap_or("UTF-8 error"));
                if unsafe{CB.is_some()} {
                    // if the synthesizer is running, indicate it should abort, then wait until the abortion is confirmed via
                    // the TTS_RUNNING state changing to false
                    if TTS_RUNNING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                        // we weren't able to get the lock. abort synthesis, until we can get the lock
                        loop {
                            TTS_SHOULD_ABORT.store(true, Ordering::SeqCst);
                            xous::yield_slice(); // we don't have a ticktimer in the FFI land, so a busy-wait is the best we can do until we get a condvar in `libstd`
                            if TTS_RUNNING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                                break;
                            }
                            xous::yield_slice(); // aggressively yield our time
                        }
                    }
                    // at this point TTS_RUNNING must be true, so we're clear to change the state variables
                    synth_string.lock().unwrap().clear();
                    synth_string.lock().unwrap().push_str(msg.text.as_str().unwrap_or("UTF8-error"));
                    send_message(synth_cid,
                        Message::new_scalar(SynthOp::NewString.to_usize().unwrap(), 0, 0, 0, 0)
                    ).expect("couldn't kick off a new string to the synth thread");
                }
            },
            Some(TtsBeOpcode::RegisterCb) => {
                let buffer = unsafe { Buffer::from_memory_message(msg.body.memory_message().unwrap()) };
                let config = buffer.to_original::<TtsBackendConfig, _>().unwrap();
                unsafe {
                    CB = Some(Callback {
                        _sid: SID::from_array(config.sid),
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
