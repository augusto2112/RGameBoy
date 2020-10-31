//
pub enum Interrupt {
    VBlank = 0x40,
    LCDStatus = 0x48,
    TimeOverflow = 0x50,
    SerialLink = 0x58,
    JoypadPress = 0x60
}

impl Interrupt {
    pub fn first_from(value: u8) -> Option<Interrupt> {
        if value & 1 == 1 {
            Some(Self::VBlank)
        } else if value >> 1 & 1 == 1 {
            Some(Self::LCDStatus)
        } else if value >> 2 & 1 == 1 {
            Some(Self::TimeOverflow)
        } else if value >> 3 & 1 == 1 {
            Some(Self::SerialLink)
        } else if value >> 4 & 1 == 1 {
            Some(Self::JoypadPress)
        } else {
            None
        }
    }

    pub fn get_index(&self) -> u8 {
        match &self {
            Interrupt::VBlank => 0,
            Interrupt::LCDStatus => 1,
            Interrupt::TimeOverflow => 2,
            Interrupt::SerialLink => 3,
            Interrupt::JoypadPress => 4,
        }
    }
}