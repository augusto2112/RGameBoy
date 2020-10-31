
pub struct Timer {
    divider_register: u8, // div
    timer_counter: u8, // tima
    timer_modulo: u8, // tma
    timer_control: u8, // tac
    internal_counter: u32,
    divider_counter: u32,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            divider_register: 0,
            timer_counter: 0,
            timer_modulo: 0,
            timer_control: 0,
            internal_counter: 0,
            divider_counter: 0,
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF04 => self.divider_register,
            0xFF05 => self.timer_counter,
            0xFF06 => self.timer_modulo,
            0xFF07 => self.timer_control,
            _ => unreachable!("Invalid address accessed in timer: {}", address)
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0xFF04 =>
                self.divider_register = value,
            0xFF05 =>
                self.timer_counter = value,
            0xFF06 =>
                self.timer_modulo = value,
            0xFF07 =>
                self.timer_control = value,
            _ => unreachable!("Invalid address accessed in timer: {}", address)
        }
    }

    pub fn tick(&mut self, elapsed: u32) -> bool {
        self.divider_counter += elapsed;
        self.divider_register = self.divider_register.wrapping_add((self.divider_counter / 256) as u8);
        self.divider_counter %= 256;

        if self.is_enabled() {
            self.internal_counter += elapsed;
            let (updated, overflow) = self.timer_counter.overflowing_add((self.internal_counter / self.get_speed()) as u8);
            self.internal_counter %= self.get_speed();

            self.timer_counter = if overflow { self.timer_modulo } else { updated };

            if overflow {
                return true
            }
        }
        false
    }

    fn is_enabled(&self) -> bool {
        self.timer_control & 0b100 != 0
    }

    fn get_speed(&self) -> u32 {
        match self.timer_control & 0b11 {
            0b00 => 256,
            0b01 => 16,
            0b10 => 4,
            0b11 => 64,
            _ => unreachable!()
        }
    }

}