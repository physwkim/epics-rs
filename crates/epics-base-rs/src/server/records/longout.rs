use epics_macros_rs::EpicsRecord;

#[derive(EpicsRecord)]
#[record(type = "longout")]
pub struct LongoutRecord {
    #[field(type = "Long")]
    pub val: i32,
    #[field(type = "String")]
    pub egu: String,
    #[field(type = "Long")]
    pub hopr: i32,
    #[field(type = "Long")]
    pub lopr: i32,
    #[field(type = "Long")]
    pub drvh: i32,
    #[field(type = "Long")]
    pub drvl: i32,
    // Alarm thresholds
    #[field(type = "Long")]
    pub hihi: i32,
    #[field(type = "Long")]
    pub high: i32,
    #[field(type = "Long")]
    pub low: i32,
    #[field(type = "Long")]
    pub lolo: i32,
    #[field(type = "Short")]
    pub hhsv: i16,
    #[field(type = "Short")]
    pub hsv: i16,
    #[field(type = "Short")]
    pub lsv: i16,
    #[field(type = "Short")]
    pub llsv: i16,
    #[field(type = "Double")]
    pub hyst: f64,
    #[field(type = "Double")]
    pub lalm: f64,
    // Invalid output
    #[field(type = "Short")]
    pub ivoa: i16,
    #[field(type = "Long")]
    pub ivov: i32,
    // Deadband
    #[field(type = "Double")]
    pub adel: f64,
    #[field(type = "Double")]
    pub mdel: f64,
    #[field(type = "Double")]
    pub alst: f64,
    #[field(type = "Double")]
    pub mlst: f64,
    #[field(type = "Short")]
    pub omsl: i16,
    #[field(type = "String")]
    pub dol: String,
    #[field(type = "Short")]
    pub simm: i16,
    #[field(type = "String")]
    pub siml: String,
    #[field(type = "String")]
    pub siol: String,
    #[field(type = "Short")]
    pub sims: i16,
}

impl Default for LongoutRecord {
    fn default() -> Self {
        Self {
            val: 0,
            egu: String::new(),
            hopr: 0,
            lopr: 0,
            drvh: 0, // C defaults both to 0 (equal = no clamping)
            drvl: 0,
            hihi: 0,
            high: 0,
            low: 0,
            lolo: 0,
            hhsv: 0,
            hsv: 0,
            lsv: 0,
            llsv: 0,
            hyst: 0.0,
            lalm: 0.0,
            ivoa: 0,
            ivov: 0,
            adel: 0.0,
            mdel: 0.0,
            alst: 0.0,
            mlst: 0.0,
            omsl: 0,
            dol: String::new(),
            simm: 0,
            siml: String::new(),
            siol: String::new(),
            sims: 0,
        }
    }
}

impl LongoutRecord {
    pub fn new(val: i32) -> Self {
        Self {
            val,
            ..Default::default()
        }
    }
}
