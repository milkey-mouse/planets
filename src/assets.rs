#![allow(irrefutable_let_patterns)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

pub enum Asset {
    Ogg(&'static [u8]),
    Txt(&'static [u8]),
    Wav(&'static [u8]),
}

impl Asset {
    pub fn ogg_data(&self) -> &'static [u8] {
        if let Asset::Ogg(data) = self {
            data
        } else {
            panic!("unwrapped asset as wrong file type");
        }
    }

    pub fn txt_data(&self) -> &'static [u8] {
        if let Asset::Txt(data) = self {
            data
        } else {
            panic!("unwrapped asset as wrong file type");
        }
    }

    pub fn wav_data(&self) -> &'static [u8] {
        if let Asset::Wav(data) = self {
            data
        } else {
            panic!("unwrapped asset as wrong file type");
        }
    }
}

pub const credits: Asset = Asset::Txt(include_bytes!("../assets/credits.txt"));
pub const menu1: Asset = Asset::Wav(include_bytes!("../assets/menu1.wav"));
pub const menu2: Asset = Asset::Wav(include_bytes!("../assets/menu2.wav"));
pub const vlem0: Asset = Asset::Ogg(include_bytes!("../assets/vlem0.ogg"));
pub const vlem1: Asset = Asset::Ogg(include_bytes!("../assets/vlem1.ogg"));
pub const vlem2: Asset = Asset::Ogg(include_bytes!("../assets/vlem2.ogg"));
pub const vlem3: Asset = Asset::Ogg(include_bytes!("../assets/vlem3.ogg"));
pub const vlem4: Asset = Asset::Ogg(include_bytes!("../assets/vlem4.ogg"));
pub const vlem5: Asset = Asset::Ogg(include_bytes!("../assets/vlem5.ogg"));
pub const vlem6: Asset = Asset::Ogg(include_bytes!("../assets/vlem6.ogg"));
pub const vlem7: Asset = Asset::Ogg(include_bytes!("../assets/vlem7.ogg"));
