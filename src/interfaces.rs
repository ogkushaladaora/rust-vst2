//! Function interfaces for VST 2.4 API.

#![doc(hidden)]

use std::ffi::{CStr, CString};
use std::{mem, vec};

use collections::enum_set::CLike;
use libc::{self, c_char, c_void};

use Vst;
use buffer::AudioBuffer;
use enums::{OpCode, CanDo};
use api::consts::*;
use api::{AEffect, Rect, KeyCode};

/// Deprecated process function
pub fn process_deprecated(_effect: *mut AEffect, _inputs_raw: *mut *mut f32, _outputs_raw: *mut *mut f32, _samples: i32) { }

/// VST2.4 replacing function
pub fn process_replacing(effect: *mut AEffect, inputs_raw: *mut *mut f32, outputs_raw: *mut *mut f32, samples: i32) {
    // Handle to the vst
    let mut vst = unsafe { (*effect).get_vst() };

    let buffer = unsafe {
        AudioBuffer::from_raw(inputs_raw,
                              outputs_raw,
                              vst.get_info().inputs as usize,
                              vst.get_info().outputs as usize,
                              samples as usize)
    };

    vst.process(buffer);
}

/// VST2.4 replacing function with `f64` values
pub fn process_replacing_f64(effect: *mut AEffect, inputs_raw: *mut *mut f64, outputs_raw: *mut *mut f64, samples: i32) {
    let mut vst = unsafe { (*effect).get_vst() };

    if vst.get_info().f64_precision {
        let buffer = unsafe {
            AudioBuffer::from_raw(inputs_raw,
                                  outputs_raw,
                                  vst.get_info().inputs as usize,
                                  vst.get_info().outputs as usize,
                                  samples as usize)
        };

        vst.process_f64(buffer);
    }
}

/// VST2.4 set parameter function
pub fn set_parameter(effect: *mut AEffect, index: i32, value: f32) {
    unsafe { (*effect).get_vst() }.set_parameter(index, value);
}

/// VST2.4 get parameter function
pub fn get_parameter(effect: *mut AEffect, index: i32) -> f32 {
    unsafe { (*effect).get_vst() }.get_parameter(index)
}


pub fn dispatch(effect: *mut AEffect, opcode: i32, index: i32, value: isize, ptr: *mut c_void, opt: f32) -> isize {
    // Convert passed in opcode to enum
    let opcode: OpCode = CLike::from_usize(opcode as usize);
    
    // Vst handle
    let mut vst = unsafe { (*effect).get_vst() };

    // Copy a string into the `ptr` buffer
    let copy_string = |string: &String, max: u64| {
        unsafe {
            libc::strncpy(ptr as *mut c_char,
                          CString::new(string.clone()).unwrap().as_ptr(),
                          max);
        }
    };

    // Read a string from the `ptr` buffer
    let read_string = || -> String {
        String::from_utf8(
            vec::as_vec(unsafe { CStr::from_ptr(ptr as *mut c_char).to_bytes() }).clone()
        ).unwrap()
    };

    match opcode {
        OpCode::Initialize => vst.init(),
        OpCode::Shutdown => unsafe { drop(mem::transmute::<*mut AEffect, Box<AEffect>>(effect)) },
        
        OpCode::ChangePreset => vst.set_preset(value as i32),
        OpCode::GetCurrentPresetNum => return vst.get_preset_num() as isize,
        OpCode::SetCurrentPresetName => vst.set_preset_name(read_string()),
        OpCode::GetCurrentPresetName => {
            let num = vst.get_preset_num();
            copy_string(&vst.get_preset_name(num), MAX_PRESET_NAME_LEN);
        }
        OpCode::GetParameterLabel => copy_string(&vst.get_parameter_label(index), MAX_PARAM_STR_LEN),
        OpCode::GetParameterDisplay => copy_string(&vst.get_parameter_text(index), MAX_PARAM_STR_LEN),
        OpCode::GetParameterName => copy_string(&vst.get_parameter_name(index), MAX_PARAM_STR_LEN),
        OpCode::SetSampleRate => vst.sample_rate_changed(opt),
        OpCode::SetBlockSize => vst.block_size_changed(value as i64),

        OpCode::StateChanged => {
            if value == 1 {
                vst.on_resume();
            } else {
                vst.on_suspend();
            }
        }

        OpCode::EditorGetRect => {
            if let Some(editor) = vst.get_editor() {
                let size = editor.size();
                let pos = editor.position();

                unsafe {
                    //given a Rect** structure
                    *(ptr as *mut *mut c_void) =
                        mem::transmute(Box::new(Rect {
                            left: pos.0 as i16, //x coord of position
                            top: pos.1 as i16, //y coord of position
                            right: (pos.0 + size.0) as i16, //x coord of pos + x coord of size
                            bottom: (pos.1 + size.1) as i16 //y coord of pos + y coord of size
                        }));
                }
            }
        }
        OpCode::EditorOpen => {
            if let Some(editor) = vst.get_editor() {
                editor.open(ptr); //ptr is raw window handle, eg HWND* on windows
            }
        }
        OpCode::EditorClose => {
            if let Some(editor) = vst.get_editor() {
                editor.close();
            }
        }
        OpCode::EditorIdle => {
            if let Some(editor) = vst.get_editor() {
                editor.idle();
            }
        }
        OpCode::EditorKeyDown => {
            if let Some(editor) = vst.get_editor() {
                editor.key_up(KeyCode {
                    character: index as u8 as char,
                    key: CLike::from_usize(value as usize),
                    modifier: unsafe { mem::transmute::<f32, i32>(opt) } as u8
                });
            }
        }
        OpCode::EditorKeyUp => { /* As above */ }
        OpCode::EditorSetKnobMode => {
            if let Some(editor) = vst.get_editor() {
                editor.set_knob_mode(CLike::from_usize(value as usize));
            }
        }


        OpCode::CanBeAutomated => return vst.can_be_automated(index) as isize,
        OpCode::GetPresetName => copy_string(&vst.get_preset_name(index), MAX_PRESET_NAME_LEN),

        OpCode::GetCategory => {
            return vst.get_info().category.to_usize() as isize;
        }

        OpCode::GetVendorName => copy_string(&vst.get_info().vendor, MAX_VENDOR_STR_LEN),
        OpCode::GetProductName => copy_string(&vst.get_info().name, MAX_PRODUCT_STR_LEN),
        OpCode::GetVendorVersion => return vst.get_info().version as isize,
        OpCode::VendorSpecific => vst.vendor_specific(index, value, ptr, opt),

        OpCode::CanDo => {
            let can_do: CanDo = match read_string().parse() {
                Ok(c) => c,
                Err(e) => { warn!("{}", e); return 0; }
            };
            return vst.can_do(can_do).ordinal() as isize;
        }
        OpCode::GetTailSize => if vst.get_tail_size() == 0 { return 1; } else { return vst.get_tail_size() },
        //OpCode::GetParamInfo => { /*TODO*/ }
        OpCode::GetApiVersion => return 2400,
        _ => {
            warn!("Unimplemented opcode ({:?})", opcode);
            trace!("Arguments; index: {}, value: {}, ptr: {:?}, opt: {}", index, value, ptr, opt);
        }
    }

    0
}