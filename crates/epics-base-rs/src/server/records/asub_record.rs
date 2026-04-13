use crate::error::{CaError, CaResult};
use crate::server::record::{FieldDesc, ProcessOutcome, Record};
use crate::types::{DbFieldType, EpicsValue};

/// aSub (array subroutine) record — extends sub with array I/O.
///
/// Input values A-L (scalar f64) read from INPA-INPL links.
/// Output arrays VALA-VALL written by the subroutine.
/// The subroutine is looked up by SNAM and called on process().
pub struct ASubRecord {
    pub val: f64,
    pub snam: String,
    pub inam: String,
    // Input links
    pub inpa: String,
    pub inpb: String,
    pub inpc: String,
    pub inpd: String,
    pub inpe: String,
    pub inpf: String,
    pub inpg: String,
    pub inph: String,
    pub inpi: String,
    pub inpj: String,
    pub inpk: String,
    pub inpl: String,
    // Input values (scalar)
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
    pub g: f64,
    pub h: f64,
    pub i: f64,
    pub j: f64,
    pub k: f64,
    pub l: f64,
    // Output arrays
    pub vala: Vec<f64>,
    pub valb: Vec<f64>,
    pub valc: Vec<f64>,
    pub vald: Vec<f64>,
    pub vale: Vec<f64>,
    pub valf: Vec<f64>,
    pub valg: Vec<f64>,
    pub valh: Vec<f64>,
    pub vali: Vec<f64>,
    pub valj: Vec<f64>,
    pub valk: Vec<f64>,
    pub vall: Vec<f64>,
    // Output links
    pub outa: String,
    pub outb: String,
    pub outc: String,
    pub outd: String,
    pub oute: String,
    pub outf: String,
    pub outg: String,
    pub outh: String,
    pub outi: String,
    pub outj: String,
    pub outk: String,
    pub outl: String,
    // Array sizes (NOA-NOL for inputs, NOVA-NOVL for outputs)
    pub noa: i32,
    pub nob: i32,
    pub noc: i32,
    pub nod: i32,
    pub noe: i32,
    pub nof: i32,
    pub nog: i32,
    pub noh: i32,
    pub noi: i32,
    pub noj: i32,
    pub nok: i32,
    pub nol: i32,
    pub nova: i32,
    pub novb: i32,
    pub novc: i32,
    pub novd: i32,
    pub nove: i32,
    pub novf: i32,
    pub novg: i32,
    pub novh: i32,
    pub novi: i32,
    pub novj: i32,
    pub novk: i32,
    pub novl: i32,
}

impl Default for ASubRecord {
    fn default() -> Self {
        Self {
            val: 0.0,
            snam: String::new(),
            inam: String::new(),
            inpa: String::new(),
            inpb: String::new(),
            inpc: String::new(),
            inpd: String::new(),
            inpe: String::new(),
            inpf: String::new(),
            inpg: String::new(),
            inph: String::new(),
            inpi: String::new(),
            inpj: String::new(),
            inpk: String::new(),
            inpl: String::new(),
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
            g: 0.0,
            h: 0.0,
            i: 0.0,
            j: 0.0,
            k: 0.0,
            l: 0.0,
            vala: Vec::new(),
            valb: Vec::new(),
            valc: Vec::new(),
            vald: Vec::new(),
            vale: Vec::new(),
            valf: Vec::new(),
            valg: Vec::new(),
            valh: Vec::new(),
            vali: Vec::new(),
            valj: Vec::new(),
            valk: Vec::new(),
            vall: Vec::new(),
            outa: String::new(),
            outb: String::new(),
            outc: String::new(),
            outd: String::new(),
            oute: String::new(),
            outf: String::new(),
            outg: String::new(),
            outh: String::new(),
            outi: String::new(),
            outj: String::new(),
            outk: String::new(),
            outl: String::new(),
            noa: 1,
            nob: 1,
            noc: 1,
            nod: 1,
            noe: 1,
            nof: 1,
            nog: 1,
            noh: 1,
            noi: 1,
            noj: 1,
            nok: 1,
            nol: 1,
            nova: 1,
            novb: 1,
            novc: 1,
            novd: 1,
            nove: 1,
            novf: 1,
            novg: 1,
            novh: 1,
            novi: 1,
            novj: 1,
            novk: 1,
            novl: 1,
        }
    }
}

static ASUB_FIELDS: &[FieldDesc] = &[
    FieldDesc {
        name: "VAL",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "SNAM",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INAM",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    // Input links
    FieldDesc {
        name: "INPA",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPB",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPC",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPD",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPE",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPF",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPG",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPH",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPI",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPJ",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPK",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "INPL",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    // Input values
    FieldDesc {
        name: "A",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "B",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "C",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "D",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "E",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "F",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "G",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "H",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "I",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "J",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "K",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "L",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    // Output arrays (as Double arrays via waveform-like access)
    FieldDesc {
        name: "VALA",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALB",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALC",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALD",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALE",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALF",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALG",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALH",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALI",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALJ",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALK",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    FieldDesc {
        name: "VALL",
        dbf_type: DbFieldType::Double,
        read_only: false,
    },
    // Output links
    FieldDesc {
        name: "OUTA",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTB",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTC",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTD",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTE",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTF",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTG",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTH",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTI",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTJ",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTK",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    FieldDesc {
        name: "OUTL",
        dbf_type: DbFieldType::String,
        read_only: false,
    },
    // Array sizes
    FieldDesc {
        name: "NOA",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOB",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOC",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOD",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOE",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOF",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOG",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOH",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOI",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOJ",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOK",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOL",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVA",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVB",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVC",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVD",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVE",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVF",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVG",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVH",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVI",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVJ",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVK",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
    FieldDesc {
        name: "NOVL",
        dbf_type: DbFieldType::Long,
        read_only: false,
    },
];

macro_rules! get_val_array {
    ($self:expr, $field:ident) => {
        if $self.$field.is_empty() {
            Some(EpicsValue::Double(0.0))
        } else if $self.$field.len() == 1 {
            Some(EpicsValue::Double($self.$field[0]))
        } else {
            Some(EpicsValue::DoubleArray($self.$field.clone()))
        }
    };
}

impl Record for ASubRecord {
    fn record_type(&self) -> &'static str {
        "aSub"
    }

    fn process(&mut self) -> CaResult<ProcessOutcome> {
        // Subroutine is called externally via RecordInstance.subroutine
        Ok(ProcessOutcome::complete())
    }

    fn get_field(&self, name: &str) -> Option<EpicsValue> {
        match name {
            "VAL" => Some(EpicsValue::Double(self.val)),
            "SNAM" => Some(EpicsValue::String(self.snam.clone())),
            "INAM" => Some(EpicsValue::String(self.inam.clone())),
            // Input links
            "INPA" => Some(EpicsValue::String(self.inpa.clone())),
            "INPB" => Some(EpicsValue::String(self.inpb.clone())),
            "INPC" => Some(EpicsValue::String(self.inpc.clone())),
            "INPD" => Some(EpicsValue::String(self.inpd.clone())),
            "INPE" => Some(EpicsValue::String(self.inpe.clone())),
            "INPF" => Some(EpicsValue::String(self.inpf.clone())),
            "INPG" => Some(EpicsValue::String(self.inpg.clone())),
            "INPH" => Some(EpicsValue::String(self.inph.clone())),
            "INPI" => Some(EpicsValue::String(self.inpi.clone())),
            "INPJ" => Some(EpicsValue::String(self.inpj.clone())),
            "INPK" => Some(EpicsValue::String(self.inpk.clone())),
            "INPL" => Some(EpicsValue::String(self.inpl.clone())),
            // Input values
            "A" => Some(EpicsValue::Double(self.a)),
            "B" => Some(EpicsValue::Double(self.b)),
            "C" => Some(EpicsValue::Double(self.c)),
            "D" => Some(EpicsValue::Double(self.d)),
            "E" => Some(EpicsValue::Double(self.e)),
            "F" => Some(EpicsValue::Double(self.f)),
            "G" => Some(EpicsValue::Double(self.g)),
            "H" => Some(EpicsValue::Double(self.h)),
            "I" => Some(EpicsValue::Double(self.i)),
            "J" => Some(EpicsValue::Double(self.j)),
            "K" => Some(EpicsValue::Double(self.k)),
            "L" => Some(EpicsValue::Double(self.l)),
            // Output arrays
            "VALA" => get_val_array!(self, vala),
            "VALB" => get_val_array!(self, valb),
            "VALC" => get_val_array!(self, valc),
            "VALD" => get_val_array!(self, vald),
            "VALE" => get_val_array!(self, vale),
            "VALF" => get_val_array!(self, valf),
            "VALG" => get_val_array!(self, valg),
            "VALH" => get_val_array!(self, valh),
            "VALI" => get_val_array!(self, vali),
            "VALJ" => get_val_array!(self, valj),
            "VALK" => get_val_array!(self, valk),
            "VALL" => get_val_array!(self, vall),
            // Output links
            "OUTA" => Some(EpicsValue::String(self.outa.clone())),
            "OUTB" => Some(EpicsValue::String(self.outb.clone())),
            "OUTC" => Some(EpicsValue::String(self.outc.clone())),
            "OUTD" => Some(EpicsValue::String(self.outd.clone())),
            "OUTE" => Some(EpicsValue::String(self.oute.clone())),
            "OUTF" => Some(EpicsValue::String(self.outf.clone())),
            "OUTG" => Some(EpicsValue::String(self.outg.clone())),
            "OUTH" => Some(EpicsValue::String(self.outh.clone())),
            "OUTI" => Some(EpicsValue::String(self.outi.clone())),
            "OUTJ" => Some(EpicsValue::String(self.outj.clone())),
            "OUTK" => Some(EpicsValue::String(self.outk.clone())),
            "OUTL" => Some(EpicsValue::String(self.outl.clone())),
            // Array sizes
            "NOA" => Some(EpicsValue::Long(self.noa)),
            "NOB" => Some(EpicsValue::Long(self.nob)),
            "NOC" => Some(EpicsValue::Long(self.noc)),
            "NOD" => Some(EpicsValue::Long(self.nod)),
            "NOE" => Some(EpicsValue::Long(self.noe)),
            "NOF" => Some(EpicsValue::Long(self.nof)),
            "NOG" => Some(EpicsValue::Long(self.nog)),
            "NOH" => Some(EpicsValue::Long(self.noh)),
            "NOI" => Some(EpicsValue::Long(self.noi)),
            "NOJ" => Some(EpicsValue::Long(self.noj)),
            "NOK" => Some(EpicsValue::Long(self.nok)),
            "NOL" => Some(EpicsValue::Long(self.nol)),
            "NOVA" => Some(EpicsValue::Long(self.nova)),
            "NOVB" => Some(EpicsValue::Long(self.novb)),
            "NOVC" => Some(EpicsValue::Long(self.novc)),
            "NOVD" => Some(EpicsValue::Long(self.novd)),
            "NOVE" => Some(EpicsValue::Long(self.nove)),
            "NOVF" => Some(EpicsValue::Long(self.novf)),
            "NOVG" => Some(EpicsValue::Long(self.novg)),
            "NOVH" => Some(EpicsValue::Long(self.novh)),
            "NOVI" => Some(EpicsValue::Long(self.novi)),
            "NOVJ" => Some(EpicsValue::Long(self.novj)),
            "NOVK" => Some(EpicsValue::Long(self.novk)),
            "NOVL" => Some(EpicsValue::Long(self.novl)),
            _ => None,
        }
    }

    fn put_field(&mut self, name: &str, value: EpicsValue) -> CaResult<()> {
        match name {
            "VAL" => {
                self.val = value
                    .to_f64()
                    .ok_or_else(|| CaError::TypeMismatch(name.into()))?;
                Ok(())
            }
            "SNAM" => match value {
                EpicsValue::String(s) => {
                    self.snam = s;
                    Ok(())
                }
                _ => Err(CaError::TypeMismatch(name.into())),
            },
            "INAM" => match value {
                EpicsValue::String(s) => {
                    self.inam = s;
                    Ok(())
                }
                _ => Err(CaError::TypeMismatch(name.into())),
            },
            // Input links
            "INPA" | "INPB" | "INPC" | "INPD" | "INPE" | "INPF" | "INPG" | "INPH" | "INPI"
            | "INPJ" | "INPK" | "INPL" => {
                let s = match value {
                    EpicsValue::String(s) => s,
                    _ => return Err(CaError::TypeMismatch(name.into())),
                };
                match name {
                    "INPA" => self.inpa = s,
                    "INPB" => self.inpb = s,
                    "INPC" => self.inpc = s,
                    "INPD" => self.inpd = s,
                    "INPE" => self.inpe = s,
                    "INPF" => self.inpf = s,
                    "INPG" => self.inpg = s,
                    "INPH" => self.inph = s,
                    "INPI" => self.inpi = s,
                    "INPJ" => self.inpj = s,
                    "INPK" => self.inpk = s,
                    "INPL" => self.inpl = s,
                    _ => unreachable!(),
                }
                Ok(())
            }
            // Input values
            "A" | "B" | "C" | "D" | "E" | "F" | "G" | "H" | "I" | "J" | "K" | "L" => {
                let v = value
                    .to_f64()
                    .ok_or_else(|| CaError::TypeMismatch(name.into()))?;
                match name {
                    "A" => self.a = v,
                    "B" => self.b = v,
                    "C" => self.c = v,
                    "D" => self.d = v,
                    "E" => self.e = v,
                    "F" => self.f = v,
                    "G" => self.g = v,
                    "H" => self.h = v,
                    "I" => self.i = v,
                    "J" => self.j = v,
                    "K" => self.k = v,
                    "L" => self.l = v,
                    _ => unreachable!(),
                }
                Ok(())
            }
            // Output arrays
            "VALA" | "VALB" | "VALC" | "VALD" | "VALE" | "VALF" | "VALG" | "VALH" | "VALI"
            | "VALJ" | "VALK" | "VALL" => {
                let arr = match value {
                    EpicsValue::DoubleArray(a) => a,
                    EpicsValue::Double(v) => vec![v],
                    other => vec![
                        other
                            .to_f64()
                            .ok_or_else(|| CaError::TypeMismatch(name.into()))?,
                    ],
                };
                match name {
                    "VALA" => self.vala = arr,
                    "VALB" => self.valb = arr,
                    "VALC" => self.valc = arr,
                    "VALD" => self.vald = arr,
                    "VALE" => self.vale = arr,
                    "VALF" => self.valf = arr,
                    "VALG" => self.valg = arr,
                    "VALH" => self.valh = arr,
                    "VALI" => self.vali = arr,
                    "VALJ" => self.valj = arr,
                    "VALK" => self.valk = arr,
                    "VALL" => self.vall = arr,
                    _ => unreachable!(),
                }
                Ok(())
            }
            // Output links
            "OUTA" | "OUTB" | "OUTC" | "OUTD" | "OUTE" | "OUTF" | "OUTG" | "OUTH" | "OUTI"
            | "OUTJ" | "OUTK" | "OUTL" => {
                let s = match value {
                    EpicsValue::String(s) => s,
                    _ => return Err(CaError::TypeMismatch(name.into())),
                };
                match name {
                    "OUTA" => self.outa = s,
                    "OUTB" => self.outb = s,
                    "OUTC" => self.outc = s,
                    "OUTD" => self.outd = s,
                    "OUTE" => self.oute = s,
                    "OUTF" => self.outf = s,
                    "OUTG" => self.outg = s,
                    "OUTH" => self.outh = s,
                    "OUTI" => self.outi = s,
                    "OUTJ" => self.outj = s,
                    "OUTK" => self.outk = s,
                    "OUTL" => self.outl = s,
                    _ => unreachable!(),
                }
                Ok(())
            }
            // Array sizes (NO* and NOV*)
            n if n.starts_with("NO") => {
                let v = match value {
                    EpicsValue::Long(v) => v,
                    _ => return Err(CaError::TypeMismatch(name.into())),
                };
                match name {
                    "NOA" => self.noa = v,
                    "NOB" => self.nob = v,
                    "NOC" => self.noc = v,
                    "NOD" => self.nod = v,
                    "NOE" => self.noe = v,
                    "NOF" => self.nof = v,
                    "NOG" => self.nog = v,
                    "NOH" => self.noh = v,
                    "NOI" => self.noi = v,
                    "NOJ" => self.noj = v,
                    "NOK" => self.nok = v,
                    "NOL" => self.nol = v,
                    "NOVA" => self.nova = v,
                    "NOVB" => self.novb = v,
                    "NOVC" => self.novc = v,
                    "NOVD" => self.novd = v,
                    "NOVE" => self.nove = v,
                    "NOVF" => self.novf = v,
                    "NOVG" => self.novg = v,
                    "NOVH" => self.novh = v,
                    "NOVI" => self.novi = v,
                    "NOVJ" => self.novj = v,
                    "NOVK" => self.novk = v,
                    "NOVL" => self.novl = v,
                    _ => return Err(CaError::FieldNotFound(name.to_string())),
                }
                Ok(())
            }
            _ => Err(CaError::FieldNotFound(name.to_string())),
        }
    }

    fn field_list(&self) -> &'static [FieldDesc] {
        ASUB_FIELDS
    }

    fn multi_input_links(&self) -> &[(&'static str, &'static str)] {
        &[
            ("INPA", "A"),
            ("INPB", "B"),
            ("INPC", "C"),
            ("INPD", "D"),
            ("INPE", "E"),
            ("INPF", "F"),
            ("INPG", "G"),
            ("INPH", "H"),
            ("INPI", "I"),
            ("INPJ", "J"),
            ("INPK", "K"),
            ("INPL", "L"),
        ]
    }
}
