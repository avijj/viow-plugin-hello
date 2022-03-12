use viow_plugin_api::{
    ViowPlugin,
    ViowPlugin_Ref,
    FiletypeLoader,
    FiletypeLoader_Ref,
    WaveLoadType,
    WaveLoad,
    WaveData,
    SignalSpec,
    SignalType,
    error::*,
};

use abi_stable::{
    rtry,
    sabi_extern_fn,
    export_root_module,
    std_types::{
        RString,
        ROption,
        RResult,
        RVec,
        Tuple2,
        ROk,
        RSome,
        RIoError,
    },
    prefix_type::PrefixTypeTrait,
    sabi_trait::prelude::*,
};
use std::fs::File;
use std::io::Read;
use std::collections::HashMap;


#[export_root_module]
pub fn get_library() -> ViowPlugin_Ref {
    ViowPlugin { 
        get_name,
        get_loader,
    }.leak_into_prefix()
}


#[sabi_extern_fn]
pub fn get_name() -> RString {
    "hello".into()
}

#[sabi_extern_fn]
pub fn get_loader() -> ROption<FiletypeLoader_Ref> {
    RSome(FiletypeLoader {
        open,
        get_suffix,
    }.leak_into_prefix())
}

//
// FileTypeLoader code
//

#[sabi_extern_fn]
pub fn get_suffix() -> RString {
    "hello".into()
}

#[sabi_extern_fn]
fn open(path: &RString, cycle_time_fs: u64) -> RResult<WaveLoadType, Error> {
    let mut file = rtry!(
        File::open(path.as_str())
            .map_err(|err| RIoError::from(err))
    );
    let mut buf = String::new();
    rtry!(
        file.read_to_string(&mut buf)
            .map_err(|err| RIoError::from(err))
    );

    let mut values: Vec<&str> = buf.split(' ').collect();

    let digits = values.pop().unwrap().trim();
    let num_cycles = rtry!(
        u64::from_str_radix(digits, 10)
            .map_err(|err| Error::Plugin(
                    format!("Loading number of cycles from file '{path}' failed on '{digits}': {err}").into()))
    );
    let digits = values.pop().unwrap().trim();
    let num_signals = rtry!(
        u64::from_str_radix(digits, 10)
            .map_err(|err| Error::Plugin(
                    format!("Loading number of signals from file '{path}' failed on '{digits}': {err}").into()))
    );

    let loader = TestLoader {
        num_signals,
        num_cycles,
        sigmap: SigMap::new(),
    };
    let rv = WaveLoadType::from_value(loader, TD_Opaque);

    ROk(rv)
}


type SigMap = HashMap<RString, (u32, SignalType)>;

struct TestLoader {
    num_signals: u64,
    num_cycles: u64,
    sigmap: SigMap,
}

impl WaveLoad for TestLoader {
    fn init_signals(&mut self) -> RResult<RVec<SignalSpec>, Error> {
        let mut rv = Vec::new();

        for i in 0..self.num_signals {
            let name = RString::from(format!("signal_{i}"));
            let typespec = if i % 10 == 0 { SignalType::Vector(15, 0) } else { SignalType::Bit };
            let spec = SignalSpec {
                name: name.clone(),
                typespec: typespec.clone(),
            };

            rv.push(spec);

            self.sigmap.insert(name, ((i % 16) as u32, typespec));
        }

        ROk(RVec::from(rv))
    }

    fn count_cycles(&mut self) -> RResult<u64, Error> {
        ROk(self.num_cycles)
    }

    fn load(&mut self, signals: &RVec<RString>, cycle_range: Tuple2<u64, u64>) -> RResult<WaveData, Error> {
        let existing_signals: Result<Vec<_>, _> = signals.iter()
            .map(|name| {
                self.sigmap.get(name)
                    .ok_or(Error::NotFound(name.clone()))
            })
            .collect();
        let existing_signals = rtry!(existing_signals);

        let mut rv = WaveData::new(
            existing_signals.iter()
                .map(|(_, sigtype)| sigtype),
            cycle_range.0 .. cycle_range.1
        );
        let mut bits = Vec::with_capacity(8);

        for (i, cycle) in (cycle_range.0 .. cycle_range.1).enumerate() {
            for (j, &sig) in existing_signals.iter().enumerate() {
                match sig.1 {
                    SignalType::Bit => {
                        //let bits = vec![cycle.rotate_right(sig.0) & 1 != 0];
                        bits.push(cycle.rotate_right(sig.0) & 1 != 0);
                        rv.set(j as u64, i as u64, &bits);
                    }

                    SignalType::Vector(a, b) => {
                        let sz = (b - a).abs();
                        //let bits: Vec<_> = (0..sz)
                            //.map(|k| (cycle >> (sz - k - 1)) & 1 != 0)
                            //.collect();

                        for k in 0..sz {
                            bits.push((cycle >> (sz - k - 1)) & 1 != 0)
                        }

                        rv.set(j as u64, i as u64, &bits);
                    }
                }

                bits.clear();
            }
        }


        ROk(rv)
    }
}
