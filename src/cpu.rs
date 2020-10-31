use Option::{Some, None};
use crate::register::{Registers, CPUFlag};
use crate::opcode::Opcode;
use crate::mmu::MMU;

pub struct CPU {
    registers: Registers,
    pub mmu: MMU,
    ime: bool,
    ime_timer: u8,
    low_power_mode: bool
}

impl CPU {
    pub fn new() -> CPU {
        Self {
            registers: Registers::new(),
            mmu: MMU::new(),
            ime: false,
            ime_timer: 0,
            low_power_mode: false
        }
    }

    pub fn load_rom(&mut self, rom: &[u8]) {
        self.mmu.load_rom(rom)
    }

    pub fn tick(&mut self) {
        self.update_timers();
        self.handle_interrupt();
        let elapsed = if self.low_power_mode {
            1
        } else {
            self.execute()
        };
        self.mmu.tick(elapsed);
    }

    fn update_timers(&mut self) {
        if self.ime_timer == 1 {
            self.ime = true;
        }
        if self.ime_timer > 0 {
            self.ime_timer -= 1;
        }
    }

    fn handle_interrupt(&mut self) -> i32 {
        if !self.ime && !self.low_power_mode { return 0; }

        if !self.mmu.is_interrupt_waiting() { return 0; }
        self.low_power_mode = false;

        if !self.ime { return 0; }
        self.ime = false;

        let interrupt = self.mmu.get_first_active_interrupt().unwrap();
        self.mmu.clear_interrupt(&interrupt);
        self.push_stack(self.registers.pc);
        self.registers.pc = interrupt as u16;
        4
    }

    fn fetch_byte(&mut self) -> u8 {
        let value = self.mmu.read_memory(self.registers.pc);
        self.registers.pc = self.registers.pc.wrapping_add(1);
        value
    }

    fn fetch_word(&mut self) -> u16 {
        let (first, second) = self.fetch_split_word();
        (first as u16) | ((second as u16) << 8)
    }

    fn fetch_split_word(&mut self) -> (u8, u8) {
        let first = self.fetch_byte();
        let second = self.fetch_byte();
        (first, second)
    }

    fn write_memory(&mut self, addr: u16, value: u8) {
        if addr == 0x02DD || addr == 0xDD02 {
            print!("")
        }
        self.mmu.write_memory(addr, value)
    }

    fn read_memory(&self, addr: u16) -> u8 {
        self.mmu.read_memory(addr)
    }

    fn set_flags(&mut self, z: Option<bool>, n: Option<bool>, h: Option<bool>, c: Option<bool>) {
        z.map(|z| self.registers.set_flag(CPUFlag::Z, z));
        n.map(|n| self.registers.set_flag(CPUFlag::N, n));
        h.map(|h| self.registers.set_flag(CPUFlag::H, h));
        c.map(|c| self.registers.set_flag(CPUFlag::C, c));
    }

    fn rotate_left_carry(&mut self, value: u8) -> u8 {
        let top = (value & 0x80) >> 7;
        let new = value << 1 | self.registers.get_flag(CPUFlag::C);
        self.set_flags(Some(new == 0), Some(false), Some(false), Some(top != 0));
        new
    }

    fn rotate_left(&mut self, value: u8) -> u8 {
        let top = (value & 0x80) >> 7;
        let new = value << 1 | top;
        self.set_flags(Some(new == 0), Some(false), Some(false), Some(top != 0));
        new
    }

    fn rotate_right_carry(&mut self, value: u8) -> u8 {
        let bottom = (value & 0x1) << 7;
        let new = (value >> 1) | (self.registers.get_flag(CPUFlag::C) << 7);
        self.set_flags(Some(new == 0), Some(false), Some(false), Some(bottom != 0));
        new
    }

    fn rotate_right(&mut self, value: u8) -> u8 {
        let bottom = (value & 0x1) << 7;
        let new = (value >> 1) | bottom;
        self.set_flags(Some(new == 0), Some(false), Some(false), Some(bottom != 0));
        new
    }

    fn shift_left(&mut self, value: u8) -> u8 {
        let top = value & 0x80;
        let new = value << 1;
        self.set_flags(Some(new == 0), Some(false), Some(false), Some(top != 0));
        new
    }

    fn shift_right_arithmetic(&mut self, value: u8) -> u8 {
        let bottom = value & 0x1;
        let new = (value >> 1) | (value & 0x80);
        self.set_flags(Some(new == 0), Some(false), Some(false), Some(bottom != 0));
        new
    }

    fn shift_right_logic(&mut self, value: u8) -> u8 {
        let bottom = value & 0x1;
        let new = value >> 1;
        self.set_flags(Some(new == 0), Some(false), Some(false), Some(bottom != 0));
        new
    }

    fn inc_reg(&mut self, value: u8) -> u8 {
        let incremented = value.wrapping_add(1);
        // since we just incremented by 1, overflow will only have happened if
        // value is exactly 0x10 now.
        self.set_flags(Some(incremented == 0), Some(false), Some((value & 0x0F) + 1 > 0x0F), None);
        incremented
    }

    fn dec_reg(&mut self, value: u8) -> u8 {
        let decremented = value.wrapping_sub(1);
        // since we just incremented by 1, borrow will only have happened if
        // value is exactly 0xff.
        self.set_flags(Some(decremented == 0), Some(true), Some((value & 0x0F) == 0), None);
        decremented
    }

    fn add_hl(&mut self, value: u16) {
        let hl = self.registers.hl();
        let sum = hl.wrapping_add(value);
        self.set_flags(None, Some(false),
                       Some((hl & 0xFFF) + (value & 0xFFF) > 0xFFF),
                       Some(hl > sum));
        self.registers.write_hl(sum);
    }

    fn add_a(&mut self, value: u8, carry: u8) {
        let sum = self.registers.a.wrapping_add(value).wrapping_add(carry);
        self.set_flags(Some(sum == 0), Some(false),
                       Some((self.registers.a & 0xF) + (value & 0xF) + carry > 0xF),
                       Some((self.registers.a as u16) + (value as u16) + (carry as u16) > 0xFF));
        self.registers.a = sum;
    }

    fn sub_a(&mut self, value: u8, carry: u8) {
        let sub = self.registers.a.wrapping_sub(value).wrapping_sub(carry);
        self.set_flags(Some(sub == 0), Some(true),
                       Some((value & 0xF) + carry > self.registers.a & 0xF),
                       Some((value as u16) + (carry as u16) > (self.registers.a as u16)));
        Some((value as u16) + (carry as u16) > (self.registers.a as u16));
        self.registers.a = sub;
    }

    fn and_a(&mut self, value: u8) {
        self.registers.a = self.registers.a & value;
        self.set_flags(Some(self.registers.a == 0), Some(false), Some(true), Some(false));
    }

    fn or_a(&mut self, value: u8) {
        self.registers.a = self.registers.a | value;
        self.set_flags(Some(self.registers.a == 0), Some(false), Some(false), Some(false));
    }

    fn xor_a(&mut self, value: u8) {
        self.registers.a = self.registers.a ^ value;
        self.set_flags(Some(self.registers.a == 0), Some(false), Some(false), Some(false));
    }

    fn cp_a(&mut self, value: u8) {
        let sub = self.registers.a.wrapping_sub(value);
        self.set_flags(Some(sub == 0), Some(true),
                       Some(value & 0xF > self.registers.a & 0xF),
                       Some(value > self.registers.a));
    }

    fn add16(&mut self, value: u16) -> u16 {
        let byte = self.fetch_byte() as i8 as i16 as u16;

        self.set_flags(Some(false), Some(false),
                       Some((value & 0x000F) + (byte & 0x000F) > 0x000F),
                       Some((value & 0x00FF) + (byte & 0x00FF) > 0x00FF));
        value.wrapping_add(byte)
    }

    fn test_bit(&mut self, index: u8, value: u8) {
        assert!(index <= 7);
        let is_not_set = ((0x1 << index) & value) == 0;
        self.set_flags(Some(is_not_set), Some(false), Some(true), None);
    }

    fn zero_bit(&mut self, index: u8, value: u8) -> u8 {
        assert!(index <= 7);
        !(0x1 << index) & value
    }

    fn set_bit(&mut self, index: u8, value: u8) -> u8 {
        assert!(index <= 7);
        (0x1 << index) | value
    }

    fn swap(&mut self, value: u8) -> u8 {
        let swapped = ((value & 0xF) << 4) | (value & 0xF0) >> 4;
        self.set_flags(Some(swapped == 0), Some(false),
                       Some(false),
                       Some(false));
        swapped
    }

    fn jump_relative(&mut self) {
        let offset = self.fetch_byte() as i8;
        self.registers.pc = ((self.registers.pc as u32 as i32) + (offset as i32)) as u16;
    }

    fn push_stack(&mut self, value: u16) {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        self.write_memory(self.registers.sp, (value >> 8) as u8);
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        self.write_memory(self.registers.sp, (value & 0xFF) as u8);
    }

    fn pop_stack(&mut self) -> u16 {
        let low = self.read_memory(self.registers.sp) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(1);
        let high = self.read_memory(self.registers.sp) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(1);
        (high << 8) | low
    }

    fn memory_string(&self) -> String {
        format!("({:02X} {:02X} {:02X} {:02X})",
                self.read_memory(self.registers.pc),
                self.read_memory(self.registers.pc.wrapping_add(1)),
                self.read_memory(self.registers.pc.wrapping_add(2)),
                self.read_memory(self.registers.pc.wrapping_add(3)))
    }

    fn execute(&mut self) -> u32 {
        let opcode = Opcode::from(self.fetch_byte());

        match opcode {
            Opcode::NOP => {
                1
            }
            Opcode::LD_BC_d16 => {
                let (lower, upper) = self.fetch_split_word();
                self.registers.b = upper;
                self.registers.c = lower;
                3
            }
            Opcode::LD_rBC_A => {
                self.write_memory(self.registers.bc(), self.registers.a);
                2
            }
            Opcode::INC_BC => {
                self.registers.write_bc(self.registers.bc().wrapping_add(1));
                2
            }
            Opcode::INC_B => {
                self.registers.b = self.inc_reg(self.registers.b);
                1
            }
            Opcode::DEC_B => {
                self.registers.b = self.dec_reg(self.registers.b);
                1
            }
            Opcode::LD_B_d8 => {
                self.registers.b = self.fetch_byte();
                2
            }
            Opcode::RLCA => {
                // RLCA is the same as RLC, but Z is always set to false
                self.registers.a = self.rotate_left(self.registers.a);
                self.registers.set_flag(CPUFlag::Z, false);
                1
            }
            Opcode::LD_ra16_SP => {
                let first = (self.registers.sp & 0xFF) as u8;
                let second = (self.registers.sp >> 8) as u8;
                let address = self.fetch_word();
                self.write_memory(address, first);
                self.write_memory(address + 1, second);
                5
            }
            Opcode::ADD_HL_BC => {
                self.add_hl(self.registers.bc());
                2
            }
            Opcode::LD_A_rBC => {
                self.registers.a = self.read_memory(self.registers.bc());
                2
            }
            Opcode::DEC_BC => {
                self.registers.write_bc(self.registers.bc().wrapping_sub(1));
                2
            }
            Opcode::INC_C => {
                self.registers.c = self.inc_reg(self.registers.c);
                1
            }
            Opcode::DEC_C => {
                self.registers.c = self.dec_reg(self.registers.c);
                1
            }
            Opcode::LD_C_d8 => {
                self.registers.c = self.fetch_byte();
                2
            }
            Opcode::RRCA => {
                // RRCA is the same as RRC, but Z is always set to false
                self.registers.a = self.rotate_right(self.registers.a);
                self.registers.set_flag(CPUFlag::Z, false);
                1
            }
            Opcode::STOP => { 0 }
            Opcode::LD_DE_d16 => {
                let (lower, upper) = self.fetch_split_word();
                self.registers.d = upper;
                self.registers.e = lower;
                3
            }
            Opcode::LD_rDE_A => {
                self.write_memory(self.registers.de(), self.registers.a);
                2
            }
            Opcode::INC_DE => {
                self.registers.write_de(self.registers.de().wrapping_add(1));
                2
            }
            Opcode::INC_D => {
                self.registers.d = self.inc_reg(self.registers.d);
                1
            }
            Opcode::DEC_D => {
                self.registers.d = self.dec_reg(self.registers.d);
                1
            }
            Opcode::LD_D_d8 => {
                self.registers.d = self.fetch_byte();
                2
            }
            Opcode::RLA => {
                self.registers.a = self.rotate_left_carry(self.registers.a);
                self.registers.set_flag(CPUFlag::Z, false);
                1
            }
            Opcode::JR_r8 => {
                self.jump_relative();
                3
            }
            Opcode::ADD_HL_DE => {
                self.add_hl(self.registers.de());
                2
            }
            Opcode::LD_A_rDE => {
                self.registers.a = self.read_memory(self.registers.de());
                2
            }
            Opcode::DEC_DE => {
                self.registers.write_de(self.registers.de().wrapping_sub(1));
                2
            }
            Opcode::INC_E => {
                self.registers.e = self.inc_reg(self.registers.e);
                1
            }
            Opcode::DEC_E => {
                self.registers.e = self.dec_reg(self.registers.e);
                1
            }
            Opcode::LD_E_d8 => {
                self.registers.e = self.fetch_byte();
                2
            }
            Opcode::RRA => {
                self.registers.a = self.rotate_right_carry(self.registers.a);
                self.registers.set_flag(CPUFlag::Z, false);
                1
            }
            Opcode::JR_NZ_r8 => {
                if self.registers.get_flag(CPUFlag::Z) == 0 {
                    self.jump_relative();
                    3
                } else {
                    self.registers.pc += 1;
                    2
                }
            }
            Opcode::LD_HL_d16 => {
                let (lower, upper) = self.fetch_split_word();
                self.registers.h = upper;
                self.registers.l = lower;
                3
            }
            Opcode::LD_rHLI_A => {
                self.write_memory(self.registers.hl(), self.registers.a);
                self.registers.write_hl(self.registers.hl().wrapping_add(1));
                2
            }
            Opcode::INC_HL => {
                self.registers.write_hl(self.registers.hl().wrapping_add(1));
                2
            }
            Opcode::INC_H => {
                self.registers.h = self.inc_reg(self.registers.h);
                1
            }
            Opcode::DEC_H => {
                self.registers.h = self.dec_reg(self.registers.h);
                1
            }
            Opcode::LD_H_d8 => {
                self.registers.h = self.fetch_byte();
                2
            }
            Opcode::DAA => {
                let n = self.registers.get_flag(CPUFlag::N);
                let c = self.registers.get_flag(CPUFlag::C);
                let h = self.registers.get_flag(CPUFlag::H);
                let mut a = self.registers.a;
                if n == 0 {
                    if c != 0 || a > 0x99 {
                        a = a.wrapping_add(0x60);
                        self.registers.set_flag(CPUFlag::C, true);
                    }
                    if h != 0 || (a & 0x0f) > 0x09 {
                        a = a.wrapping_add(0x6);
                    }
                } else {
                    if c != 0 {
                        a = a.wrapping_sub(0x60);
                    }
                    if h != 0 {
                        a = a.wrapping_sub(0x6);
                    }
                }
                self.registers.set_flag(CPUFlag::Z, a == 0);
                self.registers.set_flag(CPUFlag::H, false);
                self.registers.a = a;
                1
            }
            Opcode::JR_Z_r8 => {
                if self.registers.get_flag(CPUFlag::Z) == 1 {
                    self.jump_relative();
                    3
                } else {
                    self.registers.pc += 1;
                    2
                }
            }
            Opcode::ADD_HL_HL => {
                self.add_hl(self.registers.hl());
                2
            }
            Opcode::LD_A_rHLI => {
                self.registers.a = self.read_memory(self.registers.hl());
                let tmp = self.registers.hl().wrapping_add(1);
                self.registers.write_hl(tmp);
                2
            }
            Opcode::DEC_HL => {
                self.registers.write_hl(self.registers.hl().wrapping_sub(1));
                2
            }
            Opcode::INC_L => {
                self.registers.l = self.inc_reg(self.registers.l);
                1
            }
            Opcode::DEC_L => {
                self.registers.l = self.dec_reg(self.registers.l);
                1
            }
            Opcode::LD_L_d8 => {
                self.registers.l = self.fetch_byte();
                2
            }
            Opcode::CPL => {
                self.registers.a = !self.registers.a;
                self.registers.set_flag(CPUFlag::N, true);
                self.registers.set_flag(CPUFlag::H, true);
                1
            }
            Opcode::JR_NC_r8 => {
                if self.registers.get_flag(CPUFlag::C) == 0 {
                    self.jump_relative();
                    3
                } else {
                    self.registers.pc += 1;
                    2
                }
            }
            Opcode::LD_SP_d16 => {
                self.registers.sp = self.fetch_word();
                3
            }
            Opcode::LD_rHLD_A => {
                self.write_memory(self.registers.hl(), self.registers.a);
                self.registers.write_hl(self.registers.hl().wrapping_sub(1));
                2
            }
            Opcode::INC_SP => {
                self.registers.sp = self.registers.sp.wrapping_add(1);
                2
            }
            Opcode::INC_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.inc_reg(value);
                self.write_memory(self.registers.hl(), updated);
                3
            }
            Opcode::DEC_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.dec_reg(value);
                self.write_memory(self.registers.hl(), updated);
                3
            }
            Opcode::LD_rHL_d8 => {
                let value = self.fetch_byte();
                self.write_memory(self.registers.hl(), value);
                3
            }
            Opcode::SCF => {
                self.registers.set_flag(CPUFlag::N, false);
                self.registers.set_flag(CPUFlag::H, false);
                self.registers.set_flag(CPUFlag::C, true);
                1
            }
            Opcode::JR_C_r8 => {
                if self.registers.get_flag(CPUFlag::C) == 1 {
                    self.jump_relative();
                    3
                } else {
                    self.registers.pc += 1;
                    2
                }
            }
            Opcode::ADD_HL_SP => {
                self.add_hl(self.registers.sp);
                2
            }
            Opcode::LD_A_rHLD => {
                self.registers.a = self.read_memory(self.registers.hl());
                self.registers.write_hl(self.registers.hl().wrapping_sub(1));
                2
            }
            Opcode::DEC_SP => {
                self.registers.sp = self.registers.sp.wrapping_sub(1);
                2
            }
            Opcode::INC_A => {
                self.registers.a = self.inc_reg(self.registers.a);
                1
            }
            Opcode::DEC_A => {
                self.registers.a = self.dec_reg(self.registers.a);
                1
            }
            Opcode::LD_A_d8 => {
                self.registers.a = self.fetch_byte();
                2
            }
            Opcode::CCF => {
                self.registers.set_flag(CPUFlag::N, false);
                self.registers.set_flag(CPUFlag::H, false);
                let flag = self.registers.get_flag(CPUFlag::C);
                self.registers.set_flag(CPUFlag::C, flag == 0);
                1
            }
            Opcode::LD_B_B => {
                self.registers.b = self.registers.b;
                1
            }
            Opcode::LD_B_C => {
                self.registers.b = self.registers.c;
                1
            }
            Opcode::LD_B_D => {
                self.registers.b = self.registers.d;
                1
            }
            Opcode::LD_B_E => {
                self.registers.b = self.registers.e;
                1
            }
            Opcode::LD_B_H => {
                self.registers.b = self.registers.h;
                1
            }
            Opcode::LD_B_L => {
                self.registers.b = self.registers.l;
                1
            }
            Opcode::LD_B_rHL => {
                self.registers.b = self.read_memory(self.registers.hl());
                2
            }
            Opcode::LD_B_A => {
                self.registers.b = self.registers.a;
                1
            }
            Opcode::LD_C_B => {
                self.registers.c = self.registers.b;
                1
            }
            Opcode::LD_C_C => {
                self.registers.c = self.registers.c;
                1
            }
            Opcode::LD_C_D => {
                self.registers.c = self.registers.d;
                1
            }
            Opcode::LD_C_E => {
                self.registers.c = self.registers.e;
                1
            }
            Opcode::LD_C_H => {
                self.registers.c = self.registers.h;
                1
            }
            Opcode::LD_C_L => {
                self.registers.c = self.registers.l;
                1
            }
            Opcode::LD_C_rHL => {
                self.registers.c = self.read_memory(self.registers.hl());
                2
            }
            Opcode::LD_C_A => {
                self.registers.c = self.registers.a;
                1
            }
            Opcode::LD_D_B => {
                self.registers.d = self.registers.b;
                1
            }
            Opcode::LD_D_C => {
                self.registers.d = self.registers.c;
                1
            }
            Opcode::LD_D_D => {
                self.registers.d = self.registers.d;
                1
            }
            Opcode::LD_D_E => {
                self.registers.d = self.registers.e;
                1
            }
            Opcode::LD_D_H => {
                self.registers.d = self.registers.h;
                1
            }
            Opcode::LD_D_L => {
                self.registers.d = self.registers.l;
                1
            }
            Opcode::LD_D_rHL => {
                self.registers.d = self.read_memory(self.registers.hl());
                2
            }
            Opcode::LD_D_A => {
                self.registers.d = self.registers.a;
                1
            }
            Opcode::LD_E_B => {
                self.registers.e = self.registers.b;
                1
            }
            Opcode::LD_E_C => {
                self.registers.e = self.registers.c;
                1
            }
            Opcode::LD_E_D => {
                self.registers.e = self.registers.d;
                1
            }
            Opcode::LD_E_E => {
                self.registers.e = self.registers.e;
                1
            }
            Opcode::LD_E_H => {
                self.registers.e = self.registers.h;
                1
            }
            Opcode::LD_E_L => {
                self.registers.e = self.registers.l;
                1
            }
            Opcode::LD_E_rHL => {
                self.registers.e = self.read_memory(self.registers.hl());
                2
            }
            Opcode::LD_E_A => {
                self.registers.e = self.registers.a;
                1
            }
            Opcode::LD_H_B => {
                self.registers.h = self.registers.b;
                1
            }
            Opcode::LD_H_C => {
                self.registers.h = self.registers.c;
                1
            }
            Opcode::LD_H_D => {
                self.registers.h = self.registers.d;
                1
            }
            Opcode::LD_H_E => {
                self.registers.h = self.registers.e;
                1
            }
            Opcode::LD_H_H => {
                self.registers.h = self.registers.h;
                1
            }
            Opcode::LD_H_L => {
                self.registers.h = self.registers.l;
                1
            }
            Opcode::LD_H_rHL => {
                self.registers.h = self.read_memory(self.registers.hl());
                2
            }
            Opcode::LD_H_A => {
                self.registers.h = self.registers.a;
                1
            }
            Opcode::LD_L_B => {
                self.registers.l = self.registers.b;
                1
            }
            Opcode::LD_L_C => {
                self.registers.l = self.registers.c;
                1
            }
            Opcode::LD_L_D => {
                self.registers.l = self.registers.d;
                1
            }
            Opcode::LD_L_E => {
                self.registers.l = self.registers.e;
                1
            }
            Opcode::LD_L_H => {
                self.registers.l = self.registers.h;
                1
            }
            Opcode::LD_L_L => {
                self.registers.l = self.registers.l;
                1
            }
            Opcode::LD_L_rHL => {
                self.registers.l = self.read_memory(self.registers.hl());
                2
            }
            Opcode::LD_L_A => {
                self.registers.l = self.registers.a;
                1
            }
            Opcode::LD_rHL_B => {
                self.write_memory(self.registers.hl(), self.registers.b);
                2
            }
            Opcode::LD_rHL_C => {
                self.write_memory(self.registers.hl(), self.registers.c);
                2
            }
            Opcode::LD_rHL_D => {
                self.write_memory(self.registers.hl(), self.registers.d);
                2
            }
            Opcode::LD_rHL_E => {
                self.write_memory(self.registers.hl(), self.registers.e);
                2
            }
            Opcode::LD_rHL_H => {
                self.write_memory(self.registers.hl(), self.registers.h);
                2
            }
            Opcode::LD_rHL_L => {
                self.write_memory(self.registers.hl(), self.registers.l);
                2
            }
            Opcode::HALT => {
                self.low_power_mode = true;
                1
            }
            Opcode::LD_rHL_A => {
                self.write_memory(self.registers.hl(), self.registers.a);
                2
            }
            Opcode::LD_A_B => {
                self.registers.a = self.registers.b;
                1
            }
            Opcode::LD_A_C => {
                self.registers.a = self.registers.c;
                1
            }
            Opcode::LD_A_D => {
                self.registers.a = self.registers.d;
                1
            }
            Opcode::LD_A_E => {
                self.registers.a = self.registers.e;
                1
            }
            Opcode::LD_A_H => {
                self.registers.a = self.registers.h;
                1
            }
            Opcode::LD_A_L => {
                self.registers.a = self.registers.l;
                1
            }
            Opcode::LD_A_rHL => {
                self.registers.a = self.read_memory(self.registers.hl());
                2
            }
            Opcode::LD_A_A => {
                self.registers.a = self.registers.a;
                1
            }
            Opcode::ADD_A_B => {
                self.add_a(self.registers.b, 0);

                1
            }
            Opcode::ADD_A_C => {
                self.add_a(self.registers.c, 0);

                1
            }
            Opcode::ADD_A_D => {
                self.add_a(self.registers.d, 0);

                1
            }
            Opcode::ADD_A_E => {
                self.add_a(self.registers.e, 0);

                1
            }
            Opcode::ADD_A_H => {
                self.add_a(self.registers.h, 0);

                1
            }
            Opcode::ADD_A_L => {
                self.add_a(self.registers.l, 0);

                1
            }
            Opcode::ADD_A_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.add_a(value, 0);
                2
            }
            Opcode::ADD_A_A => {
                self.add_a(self.registers.a, 0);
                1
            }
            Opcode::ADC_A_B => {
                self.add_a(self.registers.b, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::ADC_A_C => {
                self.add_a(self.registers.c, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::ADC_A_D => {
                self.add_a(self.registers.d, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::ADC_A_E => {
                self.add_a(self.registers.e, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::ADC_A_H => {
                self.add_a(self.registers.h, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::ADC_A_L => {
                self.add_a(self.registers.l, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::ADC_A_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.add_a(value, self.registers.get_flag(CPUFlag::C));
                2
            }
            Opcode::ADC_A_A => {
                self.add_a(self.registers.a, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::SUB_B => {
                self.sub_a(self.registers.b, 0);
                1
            }
            Opcode::SUB_C => {
                self.sub_a(self.registers.c, 0);
                1
            }
            Opcode::SUB_D => {
                self.sub_a(self.registers.d, 0);
                1
            }
            Opcode::SUB_E => {
                self.sub_a(self.registers.e, 0);
                1
            }
            Opcode::SUB_H => {
                self.sub_a(self.registers.h, 0);
                1
            }
            Opcode::SUB_L => {
                self.sub_a(self.registers.l, 0);
                1
            }
            Opcode::SUB_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.sub_a(value, 0);
                2
            }
            Opcode::SUB_A => {
                self.sub_a(self.registers.a, 0);
                1
            }
            Opcode::SBC_A_B => {
                self.sub_a(self.registers.b, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::SBC_A_C => {
                self.sub_a(self.registers.c, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::SBC_A_D => {
                self.sub_a(self.registers.d, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::SBC_A_E => {
                self.sub_a(self.registers.e, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::SBC_A_H => {
                self.sub_a(self.registers.h, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::SBC_A_L => {
                self.sub_a(self.registers.l, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::SBC_A_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.sub_a(value, self.registers.get_flag(CPUFlag::C));
                2
            }
            Opcode::SBC_A_A => {
                self.sub_a(self.registers.a, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::AND_B => {
                self.and_a(self.registers.b);
                1
            }
            Opcode::AND_C => {
                self.and_a(self.registers.c);
                1
            }
            Opcode::AND_D => {
                self.and_a(self.registers.d);
                1
            }
            Opcode::AND_E => {
                self.and_a(self.registers.e);
                1
            }
            Opcode::AND_H => {
                self.and_a(self.registers.h);
                1
            }
            Opcode::AND_L => {
                self.and_a(self.registers.l);
                1
            }
            Opcode::AND_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.and_a(value);
                2
            }
            Opcode::AND_A => {
                self.and_a(self.registers.a);
                1
            }
            Opcode::XOR_B => {
                self.xor_a(self.registers.b);
                1
            }
            Opcode::XOR_C => {
                self.xor_a(self.registers.c);
                1
            }
            Opcode::XOR_D => {
                self.xor_a(self.registers.d);
                1
            }
            Opcode::XOR_E => {
                self.xor_a(self.registers.e);
                1
            }
            Opcode::XOR_H => {
                self.xor_a(self.registers.h);
                1
            }
            Opcode::XOR_L => {
                self.xor_a(self.registers.l);
                1
            }
            Opcode::XOR_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.xor_a(value);
                2
            }
            Opcode::XOR_A => {
                self.xor_a(self.registers.a);
                1
            }
            Opcode::OR_B => {
                self.or_a(self.registers.b);
                1
            }
            Opcode::OR_C => {
                self.or_a(self.registers.c);
                1
            }
            Opcode::OR_D => {
                self.or_a(self.registers.d);
                1
            }
            Opcode::OR_E => {
                self.or_a(self.registers.e);
                1
            }
            Opcode::OR_H => {
                self.or_a(self.registers.h);
                1
            }
            Opcode::OR_L => {
                self.or_a(self.registers.l);
                1
            }
            Opcode::OR_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.or_a(value);
                2
            }
            Opcode::OR_A => {
                self.or_a(self.registers.a);
                1
            }
            Opcode::CP_B => {
                self.cp_a(self.registers.b);
                1
            }
            Opcode::CP_C => {
                self.cp_a(self.registers.c);
                1
            }
            Opcode::CP_D => {
                self.cp_a(self.registers.d);
                1
            }
            Opcode::CP_E => {
                self.cp_a(self.registers.e);
                1
            }
            Opcode::CP_H => {
                self.cp_a(self.registers.h);
                1
            }
            Opcode::CP_L => {
                self.cp_a(self.registers.l);
                1
            }
            Opcode::CP_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.cp_a(value);
                2
            }
            Opcode::CP_A => {
                self.cp_a(self.registers.a);
                1
            }
            Opcode::RET_NZ => {
                if self.registers.get_flag(CPUFlag::Z) == 0 {
                    self.registers.pc = self.pop_stack();
                    5
                } else {
                    2
                }
            }
            Opcode::POP_BC => {
                let value = self.pop_stack();
                self.registers.write_bc(value);
                3
            }
            Opcode::JP_NZ_a16 => {
                if self.registers.get_flag(CPUFlag::Z) == 0 {
                    self.registers.pc = self.fetch_word();
                    4
                } else {
                    self.registers.pc += 2;
                    3
                }
            }
            Opcode::JP_a16 => {
                self.registers.pc = self.fetch_word();
                4
            }
            Opcode::CALL_NZ_a16 => {
                if self.registers.get_flag(CPUFlag::Z) == 0 {
                    let addr = self.fetch_word();
                    self.push_stack(self.registers.pc);
                    self.registers.pc = addr;
                    6
                } else {
                    self.registers.pc += 2;
                    3
                }
            }
            Opcode::PUSH_BC => {
                self.push_stack(self.registers.bc());
                4
            }
            Opcode::ADD_A_d8 => {
                let value = self.fetch_byte();
                self.add_a(value, 0);
                1
            }
            Opcode::RST_00H => {
                self.push_stack(self.registers.pc);
                self.registers.pc = 0x00;
                4
            }
            Opcode::RET_Z => {
                if self.registers.get_flag(CPUFlag::Z) == 1 {
                    self.registers.pc = self.pop_stack();
                    5
                } else {
                    2
                }
            }
            Opcode::RET => {
                self.registers.pc = self.pop_stack();
                4
            }
            Opcode::JP_Z_a16 => {
                if self.registers.get_flag(CPUFlag::Z) == 1 {
                    self.registers.pc = self.fetch_word();
                    4
                } else {
                    self.registers.pc += 2;
                    3
                }
            }
            Opcode::PREFIX => { self.execute_extended() }
            Opcode::CALL_Z_a16 => {
                if self.registers.get_flag(CPUFlag::Z) == 1 {
                    let addr = self.fetch_word();
                    self.push_stack(self.registers.pc);
                    self.registers.pc = addr;
                    6
                } else {
                    self.registers.pc += 2;
                    3
                }
            }
            Opcode::CALL_a16 => {
                let addr = self.fetch_word();
                self.push_stack(self.registers.pc);
                self.registers.pc = addr;
                6
            }
            Opcode::ADC_A_d8 => {
                let value = self.fetch_byte();
                self.add_a(value, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::RST_08H => {
                self.push_stack(self.registers.pc);
                self.registers.pc = 0x08;
                4
            }
            Opcode::RET_NC => {
                if self.registers.get_flag(CPUFlag::C) == 0 {
                    self.registers.pc = self.pop_stack();
                    5
                } else {
                    2
                }
            }
            Opcode::POP_DE => {
                let value = self.pop_stack();
                self.registers.write_de(value);
                3
            }
            Opcode::JP_NC_a16 => {
                if self.registers.get_flag(CPUFlag::C) == 0 {
                    self.registers.pc = self.fetch_word();
                    4
                } else {
                    self.registers.pc += 2;
                    3
                }
            }
            Opcode::CALL_NC_a16 => {
                if self.registers.get_flag(CPUFlag::C) == 0 {
                    let addr = self.fetch_word();
                    self.push_stack(self.registers.pc);
                    self.registers.pc = addr;
                    6
                } else {
                    self.registers.pc += 2;
                    3
                }
            }
            Opcode::PUSH_DE => {
                self.push_stack(self.registers.de());
                4
            }
            Opcode::SUB_d8 => {
                let value = self.fetch_byte();
                self.sub_a(value, 0);
                1
            }
            Opcode::RST_10H => {
                self.push_stack(self.registers.pc);
                self.registers.pc = 0x10;
                4
            }
            Opcode::RET_C => {
                if self.registers.get_flag(CPUFlag::C) == 1 {
                    self.registers.pc = self.pop_stack();
                    5
                } else {
                    2
                }
            }
            Opcode::RETI => {
                self.ime = true;
                self.registers.pc = self.pop_stack();
                4
            }
            Opcode::JP_C_a16 => {
                if self.registers.get_flag(CPUFlag::C) == 1 {
                    self.registers.pc = self.fetch_word();
                    4
                } else {
                    self.registers.pc += 2;
                    3
                }
            }
            Opcode::CALL_C_a16 => {
                if self.registers.get_flag(CPUFlag::C) == 1 {
                    let addr = self.fetch_word();
                    self.push_stack(self.registers.pc);
                    self.registers.pc = addr;
                    6
                } else {
                    self.registers.pc += 2;
                    3
                }
            }
            Opcode::SBC_A_d8 => {
                let value = self.fetch_byte();
                self.sub_a(value, self.registers.get_flag(CPUFlag::C));
                1
            }
            Opcode::RST_18H => {
                self.push_stack(self.registers.pc);
                self.registers.pc = 0x18;
                4
            }
            Opcode::LDH_ra8_A => {
                let addr = 0xFF00 as u16 + self.fetch_byte() as u16;
                self.write_memory(addr, self.registers.a);
                3
            }
            Opcode::POP_HL => {
                let value = self.pop_stack();
                self.registers.write_hl(value);
                3
            }
            Opcode::LD_rC_A => {
                let addr = 0xFF00 | (self.registers.c as u16);
                self.write_memory(addr, self.registers.a);
                3
            }
            Opcode::PUSH_HL => {
                self.push_stack(self.registers.hl());
                4
            }
            Opcode::AND_d8 => {
                let value = self.fetch_byte();
                self.and_a(value);
                1
            }
            Opcode::RST_20H => {
                self.push_stack(self.registers.pc);
                self.registers.pc = 0x20;
                4
            }
            Opcode::ADD_SP_r8 => {
                self.registers.sp = self.add16(self.registers.sp);
                4
            }
            Opcode::JP_HL => {
                self.registers.pc = self.registers.hl();
                1
            }
            Opcode::LD_ra16_A => {
                let addr = self.fetch_word();
                self.write_memory(addr, self.registers.a);
                4
            }
            Opcode::XOR_d8 => {
                let value = self.fetch_byte();
                self.xor_a(value);
                1
            }
            Opcode::RST_28H => {
                self.push_stack(self.registers.pc);
                self.registers.pc = 0x28;
                4
            }
            Opcode::LDH_A_ra8 => {
                let addr = 0xFF00 as u16 + self.fetch_byte() as u16;
                self.registers.a = self.read_memory(addr);
                3
            }
            Opcode::POP_AF => {
                let value = self.pop_stack() & 0xFFF0;
                self.registers.write_af(value);
                3
            }
            Opcode::LD_A_rC => {
                let addr = 0xFF00 | (self.registers.c as u16);
                self.registers.a = self.read_memory(addr);
                3
            }
            Opcode::DI => {
                self.ime = false;
                1
            }
            Opcode::PUSH_AF => {
                self.push_stack(self.registers.af());
                4
            }
            Opcode::OR_d8 => {
                let value = self.fetch_byte();
                self.or_a(value);
                1
            }
            Opcode::RST_30H => {
                self.push_stack(self.registers.pc);
                self.registers.pc = 0x30;
                4
            }
            Opcode::LD_HL_SPI => {
                let value = self.add16(self.registers.sp);
                self.registers.write_hl(value);
                3
            }
            Opcode::LD_SP_HL => {
                self.registers.sp = self.registers.hl();
                2
            }
            Opcode::LD_A_ra16 => {
                let addr = self.fetch_word();
                self.registers.a = self.read_memory(addr);
                4
            }
            Opcode::EI => {
                self.ime_timer = 2;
                1
            }
            Opcode::CP_d8 => {
                let value = self.fetch_byte();
                self.cp_a(value);
                2
            }
            Opcode::RST_38H => {
                self.push_stack(self.registers.pc);
                self.registers.pc = 0x38;
                4
            }
            _ => panic!("Unexpected opcode: {:#?}", opcode)
        }
    }
    fn execute_extended(&mut self) -> u32 {
        let opcode = Opcode::from_extended(self.fetch_byte());
        match opcode {
            Opcode::RLC_B => {
                self.registers.b = self.rotate_left(self.registers.b);
                2
            }
            Opcode::RLC_C => {
                self.registers.c = self.rotate_left(self.registers.c);
                2
            }
            Opcode::RLC_D => {
                self.registers.d = self.rotate_left(self.registers.d);
                2
            }
            Opcode::RLC_E => {
                self.registers.e = self.rotate_left(self.registers.e);
                2
            }
            Opcode::RLC_H => {
                self.registers.h = self.rotate_left(self.registers.h);
                2
            }
            Opcode::RLC_L => {
                self.registers.l = self.rotate_left(self.registers.l);
                2
            }
            Opcode::RLC_rHL => {
                let value = self.read_memory(self.registers.hl());
                let rotated = self.rotate_left(value);
                self.write_memory(self.registers.hl(), rotated);
                4
            }
            Opcode::RLC_A => {
                self.registers.a = self.rotate_left(self.registers.a);
                2
            }
            Opcode::RRC_B => {
                self.registers.b = self.rotate_right(self.registers.b);
                2
            }
            Opcode::RRC_C => {
                self.registers.c = self.rotate_right(self.registers.c);
                2
            }
            Opcode::RRC_D => {
                self.registers.d = self.rotate_right(self.registers.d);
                2
            }
            Opcode::RRC_E => {
                self.registers.e = self.rotate_right(self.registers.e);
                2
            }
            Opcode::RRC_H => {
                self.registers.h = self.rotate_right(self.registers.h);
                2
            }
            Opcode::RRC_L => {
                self.registers.l = self.rotate_right(self.registers.l);
                2
            }
            Opcode::RRC_rHL => {
                let value = self.read_memory(self.registers.hl());
                let rotated = self.rotate_right(value);
                self.write_memory(self.registers.hl(), rotated);
                4
            }
            Opcode::RRC_A => {
                self.registers.a = self.rotate_right(self.registers.a);
                2
            }
            Opcode::RL_B => {
                self.registers.b = self.rotate_left_carry(self.registers.b);
                2
            }
            Opcode::RL_C => {
                self.registers.c = self.rotate_left_carry(self.registers.c);
                2
            }
            Opcode::RL_D => {
                self.registers.d = self.rotate_left_carry(self.registers.d);
                2
            }
            Opcode::RL_E => {
                self.registers.e = self.rotate_left_carry(self.registers.e);
                2
            }
            Opcode::RL_H => {
                self.registers.h = self.rotate_left_carry(self.registers.h);
                2
            }
            Opcode::RL_L => {
                self.registers.l = self.rotate_left_carry(self.registers.l);
                2
            }
            Opcode::RL_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.rotate_left_carry(value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RL_A => {
                self.registers.a = self.rotate_left_carry(self.registers.a);
                2
            }
            Opcode::RR_B => {
                self.registers.b = self.rotate_right_carry(self.registers.b);
                2
            }
            Opcode::RR_C => {
                self.registers.c = self.rotate_right_carry(self.registers.c);
                2
            }
            Opcode::RR_D => {
                self.registers.d = self.rotate_right_carry(self.registers.d);
                2
            }
            Opcode::RR_E => {
                self.registers.e = self.rotate_right_carry(self.registers.e);
                2
            }
            Opcode::RR_H => {
                self.registers.h = self.rotate_right_carry(self.registers.h);
                2
            }
            Opcode::RR_L => {
                self.registers.l = self.rotate_right_carry(self.registers.l);
                2
            }
            Opcode::RR_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.rotate_right_carry(value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RR_A => {
                self.registers.a = self.rotate_right_carry(self.registers.a);
                2
            }
            Opcode::SLA_B => {
                self.registers.b = self.shift_left(self.registers.b);
                2
            }
            Opcode::SLA_C => {
                self.registers.c = self.shift_left(self.registers.c);
                2
            }
            Opcode::SLA_D => {
                self.registers.d = self.shift_left(self.registers.d);
                2
            }
            Opcode::SLA_E => {
                self.registers.e = self.shift_left(self.registers.e);
                2
            }
            Opcode::SLA_H => {
                self.registers.h = self.shift_left(self.registers.h);
                2
            }
            Opcode::SLA_L => {
                self.registers.l = self.shift_left(self.registers.l);
                2
            }
            Opcode::SLA_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.shift_left(value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SLA_A => {
                self.registers.a = self.shift_left(self.registers.a);
                2
            }
            Opcode::SRA_B => {
                self.registers.b = self.shift_right_arithmetic(self.registers.b);
                2
            }
            Opcode::SRA_C => {
                self.registers.c = self.shift_right_arithmetic(self.registers.c);
                2
            }
            Opcode::SRA_D => {
                self.registers.d = self.shift_right_arithmetic(self.registers.d);
                2
            }
            Opcode::SRA_E => {
                self.registers.e = self.shift_right_arithmetic(self.registers.e);
                2
            }
            Opcode::SRA_H => {
                self.registers.h = self.shift_right_arithmetic(self.registers.h);
                2
            }
            Opcode::SRA_L => {
                self.registers.l = self.shift_right_arithmetic(self.registers.l);
                2
            }
            Opcode::SRA_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.shift_right_arithmetic(value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SRA_A => {
                self.registers.a = self.shift_right_arithmetic(self.registers.a);
                2
            }
            Opcode::SWAP_B => {
                self.registers.b = self.swap(self.registers.b);
                2
            }
            Opcode::SWAP_C => {
                self.registers.c = self.swap(self.registers.c);
                2
            }
            Opcode::SWAP_D => {
                self.registers.d = self.swap(self.registers.d);
                2
            }
            Opcode::SWAP_E => {
                self.registers.e = self.swap(self.registers.e);
                2
            }
            Opcode::SWAP_H => {
                self.registers.h = self.swap(self.registers.h);
                2
            }
            Opcode::SWAP_L => {
                self.registers.l = self.swap(self.registers.l);
                2
            }
            Opcode::SWAP_rHL => {
                let value = self.read_memory(self.registers.hl());
                let swapped = self.swap(value);
                self.write_memory(self.registers.hl(), swapped);
                4
            }
            Opcode::SWAP_A => {
                self.registers.a = self.swap(self.registers.a);
                2
            }
            Opcode::SRL_B => {
                self.registers.b = self.shift_right_logic(self.registers.b);
                2
            }
            Opcode::SRL_C => {
                self.registers.c = self.shift_right_logic(self.registers.c);
                2
            }
            Opcode::SRL_D => {
                self.registers.d = self.shift_right_logic(self.registers.d);
                2
            }
            Opcode::SRL_E => {
                self.registers.e = self.shift_right_logic(self.registers.e);
                2
            }
            Opcode::SRL_H => {
                self.registers.h = self.shift_right_logic(self.registers.h);
                2
            }
            Opcode::SRL_L => {
                self.registers.l = self.shift_right_logic(self.registers.l);
                2
            }
            Opcode::SRL_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.shift_right_logic(value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SRL_A => {
                self.registers.a = self.shift_right_logic(self.registers.a);
                2
            }
            Opcode::BIT_0_B => {
                self.test_bit(0, self.registers.b);
                2
            }
            Opcode::BIT_0_C => {
                self.test_bit(0, self.registers.c);
                2
            }
            Opcode::BIT_0_D => {
                self.test_bit(0, self.registers.d);
                2
            }
            Opcode::BIT_0_E => {
                self.test_bit(0, self.registers.e);
                2
            }
            Opcode::BIT_0_H => {
                self.test_bit(0, self.registers.h);
                2
            }
            Opcode::BIT_0_L => {
                self.test_bit(0, self.registers.l);
                2
            }
            Opcode::BIT_0_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.test_bit(0, value);
                3
            }
            Opcode::BIT_0_A => {
                self.test_bit(0, self.registers.a);
                2
            }
            Opcode::BIT_1_B => {
                self.test_bit(1, self.registers.b);
                2
            }
            Opcode::BIT_1_C => {
                self.test_bit(1, self.registers.c);
                2
            }
            Opcode::BIT_1_D => {
                self.test_bit(1, self.registers.d);
                2
            }
            Opcode::BIT_1_E => {
                self.test_bit(1, self.registers.e);
                2
            }
            Opcode::BIT_1_H => {
                self.test_bit(1, self.registers.h);
                2
            }
            Opcode::BIT_1_L => {
                self.test_bit(1, self.registers.l);
                2
            }
            Opcode::BIT_1_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.test_bit(1, value);
                3
            }
            Opcode::BIT_1_A => {
                self.test_bit(1, self.registers.a);
                2
            }
            Opcode::BIT_2_B => {
                self.test_bit(2, self.registers.b);
                2
            }
            Opcode::BIT_2_C => {
                self.test_bit(2, self.registers.c);
                2
            }
            Opcode::BIT_2_D => {
                self.test_bit(2, self.registers.d);
                2
            }
            Opcode::BIT_2_E => {
                self.test_bit(2, self.registers.e);
                2
            }
            Opcode::BIT_2_H => {
                self.test_bit(2, self.registers.h);
                2
            }
            Opcode::BIT_2_L => {
                self.test_bit(2, self.registers.l);
                2
            }
            Opcode::BIT_2_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.test_bit(2, value);
                3
            }
            Opcode::BIT_2_A => {
                self.test_bit(2, self.registers.a);
                2
            }
            Opcode::BIT_3_B => {
                self.test_bit(3, self.registers.b);
                2
            }
            Opcode::BIT_3_C => {
                self.test_bit(3, self.registers.c);
                2
            }
            Opcode::BIT_3_D => {
                self.test_bit(3, self.registers.d);
                2
            }
            Opcode::BIT_3_E => {
                self.test_bit(3, self.registers.e);
                2
            }
            Opcode::BIT_3_H => {
                self.test_bit(3, self.registers.h);
                2
            }
            Opcode::BIT_3_L => {
                self.test_bit(3, self.registers.l);
                2
            }
            Opcode::BIT_3_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.test_bit(3, value);
                3
            }
            Opcode::BIT_3_A => {
                self.test_bit(3, self.registers.a);
                2
            }
            Opcode::BIT_4_B => {
                self.test_bit(4, self.registers.b);
                2
            }
            Opcode::BIT_4_C => {
                self.test_bit(4, self.registers.c);
                2
            }
            Opcode::BIT_4_D => {
                self.test_bit(4, self.registers.d);
                2
            }
            Opcode::BIT_4_E => {
                self.test_bit(4, self.registers.e);
                2
            }
            Opcode::BIT_4_H => {
                self.test_bit(4, self.registers.h);
                2
            }
            Opcode::BIT_4_L => {
                self.test_bit(4, self.registers.l);
                2
            }
            Opcode::BIT_4_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.test_bit(4, value);
                3
            }
            Opcode::BIT_4_A => {
                self.test_bit(4, self.registers.a);
                2
            }
            Opcode::BIT_5_B => {
                self.test_bit(5, self.registers.b);
                2
            }
            Opcode::BIT_5_C => {
                self.test_bit(5, self.registers.c);
                2
            }
            Opcode::BIT_5_D => {
                self.test_bit(5, self.registers.d);
                2
            }
            Opcode::BIT_5_E => {
                self.test_bit(5, self.registers.e);
                2
            }
            Opcode::BIT_5_H => {
                self.test_bit(5, self.registers.h);
                2
            }
            Opcode::BIT_5_L => {
                self.test_bit(5, self.registers.l);
                2
            }
            Opcode::BIT_5_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.test_bit(5, value);
                3
            }
            Opcode::BIT_5_A => {
                self.test_bit(5, self.registers.a);
                2
            }
            Opcode::BIT_6_B => {
                self.test_bit(6, self.registers.b);
                2
            }
            Opcode::BIT_6_C => {
                self.test_bit(6, self.registers.c);
                2
            }
            Opcode::BIT_6_D => {
                self.test_bit(6, self.registers.d);
                2
            }
            Opcode::BIT_6_E => {
                self.test_bit(6, self.registers.e);
                2
            }
            Opcode::BIT_6_H => {
                self.test_bit(6, self.registers.h);
                2
            }
            Opcode::BIT_6_L => {
                self.test_bit(6, self.registers.l);
                2
            }
            Opcode::BIT_6_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.test_bit(6, value);
                3
            }
            Opcode::BIT_6_A => {
                self.test_bit(6, self.registers.a);
                2
            }
            Opcode::BIT_7_B => {
                self.test_bit(7, self.registers.b);
                2
            }
            Opcode::BIT_7_C => {
                self.test_bit(7, self.registers.c);
                2
            }
            Opcode::BIT_7_D => {
                self.test_bit(7, self.registers.d);
                2
            }
            Opcode::BIT_7_E => {
                self.test_bit(7, self.registers.e);
                2
            }
            Opcode::BIT_7_H => {
                self.test_bit(7, self.registers.h);
                2
            }
            Opcode::BIT_7_L => {
                self.test_bit(7, self.registers.l);
                2
            }
            Opcode::BIT_7_rHL => {
                let value = self.read_memory(self.registers.hl());
                self.test_bit(7, value);
                3
            }
            Opcode::BIT_7_A => {
                self.test_bit(7, self.registers.a);
                2
            }
            Opcode::RES_0_B => {
                self.registers.b = self.zero_bit(0, self.registers.b);
                2
            }
            Opcode::RES_0_C => {
                self.registers.c = self.zero_bit(0, self.registers.c);
                2
            }
            Opcode::RES_0_D => {
                self.registers.d = self.zero_bit(0, self.registers.d);
                2
            }
            Opcode::RES_0_E => {
                self.registers.e = self.zero_bit(0, self.registers.e);
                2
            }
            Opcode::RES_0_H => {
                self.registers.h = self.zero_bit(0, self.registers.h);
                2
            }
            Opcode::RES_0_L => {
                self.registers.l = self.zero_bit(0, self.registers.l);
                2
            }
            Opcode::RES_0_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.zero_bit(0, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RES_0_A => {
                self.registers.a = self.zero_bit(0, self.registers.a);
                2
            }
            Opcode::RES_1_B => {
                self.registers.b = self.zero_bit(1, self.registers.b);
                2
            }
            Opcode::RES_1_C => {
                self.registers.c = self.zero_bit(1, self.registers.c);
                2
            }
            Opcode::RES_1_D => {
                self.registers.d = self.zero_bit(1, self.registers.d);
                2
            }
            Opcode::RES_1_E => {
                self.registers.e = self.zero_bit(1, self.registers.e);
                2
            }
            Opcode::RES_1_H => {
                self.registers.h = self.zero_bit(1, self.registers.h);
                2
            }
            Opcode::RES_1_L => {
                self.registers.l = self.zero_bit(1, self.registers.l);
                2
            }
            Opcode::RES_1_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.zero_bit(1, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RES_1_A => {
                self.registers.a = self.zero_bit(1, self.registers.a);
                2
            }
            Opcode::RES_2_B => {
                self.registers.b = self.zero_bit(2, self.registers.b);
                2
            }
            Opcode::RES_2_C => {
                self.registers.c = self.zero_bit(2, self.registers.c);
                2
            }
            Opcode::RES_2_D => {
                self.registers.d = self.zero_bit(2, self.registers.d);
                2
            }
            Opcode::RES_2_E => {
                self.registers.e = self.zero_bit(2, self.registers.e);
                2
            }
            Opcode::RES_2_H => {
                self.registers.h = self.zero_bit(2, self.registers.h);
                2
            }
            Opcode::RES_2_L => {
                self.registers.l = self.zero_bit(2, self.registers.l);
                2
            }
            Opcode::RES_2_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.zero_bit(2, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RES_2_A => {
                self.registers.a = self.zero_bit(2, self.registers.a);
                2
            }
            Opcode::RES_3_B => {
                self.registers.b = self.zero_bit(3, self.registers.b);
                2
            }
            Opcode::RES_3_C => {
                self.registers.c = self.zero_bit(3, self.registers.c);
                2
            }
            Opcode::RES_3_D => {
                self.registers.d = self.zero_bit(3, self.registers.d);
                2
            }
            Opcode::RES_3_E => {
                self.registers.e = self.zero_bit(3, self.registers.e);
                2
            }
            Opcode::RES_3_H => {
                self.registers.h = self.zero_bit(3, self.registers.h);
                2
            }
            Opcode::RES_3_L => {
                self.registers.l = self.zero_bit(3, self.registers.l);
                2
            }
            Opcode::RES_3_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.zero_bit(3, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RES_3_A => {
                self.registers.a = self.zero_bit(3, self.registers.a);
                2
            }
            Opcode::RES_4_B => {
                self.registers.b = self.zero_bit(4, self.registers.b);
                2
            }
            Opcode::RES_4_C => {
                self.registers.c = self.zero_bit(4, self.registers.c);
                2
            }
            Opcode::RES_4_D => {
                self.registers.d = self.zero_bit(4, self.registers.d);
                2
            }
            Opcode::RES_4_E => {
                self.registers.e = self.zero_bit(4, self.registers.e);
                2
            }
            Opcode::RES_4_H => {
                self.registers.h = self.zero_bit(4, self.registers.h);
                2
            }
            Opcode::RES_4_L => {
                self.registers.l = self.zero_bit(4, self.registers.l);
                2
            }
            Opcode::RES_4_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.zero_bit(4, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RES_4_A => {
                self.registers.a = self.zero_bit(4, self.registers.a);
                2
            }
            Opcode::RES_5_B => {
                self.registers.b = self.zero_bit(5, self.registers.b);
                2
            }
            Opcode::RES_5_C => {
                self.registers.c = self.zero_bit(5, self.registers.c);
                2
            }
            Opcode::RES_5_D => {
                self.registers.d = self.zero_bit(5, self.registers.d);
                2
            }
            Opcode::RES_5_E => {
                self.registers.e = self.zero_bit(5, self.registers.e);
                2
            }
            Opcode::RES_5_H => {
                self.registers.h = self.zero_bit(5, self.registers.h);
                2
            }
            Opcode::RES_5_L => {
                self.registers.l = self.zero_bit(5, self.registers.l);
                2
            }
            Opcode::RES_5_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.zero_bit(5, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RES_5_A => {
                self.registers.a = self.zero_bit(5, self.registers.a);
                2
            }
            Opcode::RES_6_B => {
                self.registers.b = self.zero_bit(6, self.registers.b);
                2
            }
            Opcode::RES_6_C => {
                self.registers.c = self.zero_bit(6, self.registers.c);
                2
            }
            Opcode::RES_6_D => {
                self.registers.d = self.zero_bit(6, self.registers.d);
                2
            }
            Opcode::RES_6_E => {
                self.registers.e = self.zero_bit(6, self.registers.e);
                2
            }
            Opcode::RES_6_H => {
                self.registers.h = self.zero_bit(6, self.registers.h);
                2
            }
            Opcode::RES_6_L => {
                self.registers.l = self.zero_bit(6, self.registers.l);
                2
            }
            Opcode::RES_6_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.zero_bit(6, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RES_6_A => {
                self.registers.a = self.zero_bit(6, self.registers.a);
                2
            }
            Opcode::RES_7_B => {
                self.registers.b = self.zero_bit(7, self.registers.b);
                2
            }
            Opcode::RES_7_C => {
                self.registers.c = self.zero_bit(7, self.registers.c);
                2
            }
            Opcode::RES_7_D => {
                self.registers.d = self.zero_bit(7, self.registers.d);
                2
            }
            Opcode::RES_7_E => {
                self.registers.e = self.zero_bit(7, self.registers.e);
                2
            }
            Opcode::RES_7_H => {
                self.registers.h = self.zero_bit(7, self.registers.h);
                2
            }
            Opcode::RES_7_L => {
                self.registers.l = self.zero_bit(7, self.registers.l);
                2
            }
            Opcode::RES_7_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.zero_bit(7, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::RES_7_A => {
                self.registers.a = self.zero_bit(7, self.registers.a);
                2
            }
            Opcode::SET_0_B => {
                self.registers.b = self.set_bit(0, self.registers.b);
                2
            }
            Opcode::SET_0_C => {
                self.registers.c = self.set_bit(0, self.registers.c);
                2
            }
            Opcode::SET_0_D => {
                self.registers.d = self.set_bit(0, self.registers.d);
                2
            }
            Opcode::SET_0_E => {
                self.registers.e = self.set_bit(0, self.registers.e);
                2
            }
            Opcode::SET_0_H => {
                self.registers.h = self.set_bit(0, self.registers.h);
                2
            }
            Opcode::SET_0_L => {
                self.registers.l = self.set_bit(0, self.registers.l);
                2
            }
            Opcode::SET_0_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.set_bit(0, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SET_0_A => {
                self.registers.a = self.set_bit(0, self.registers.a);
                2
            }
            Opcode::SET_1_B => {
                self.registers.b = self.set_bit(1, self.registers.b);
                2
            }
            Opcode::SET_1_C => {
                self.registers.c = self.set_bit(1, self.registers.c);
                2
            }
            Opcode::SET_1_D => {
                self.registers.d = self.set_bit(1, self.registers.d);
                2
            }
            Opcode::SET_1_E => {
                self.registers.e = self.set_bit(1, self.registers.e);
                2
            }
            Opcode::SET_1_H => {
                self.registers.h = self.set_bit(1, self.registers.h);
                2
            }
            Opcode::SET_1_L => {
                self.registers.l = self.set_bit(1, self.registers.l);
                2
            }
            Opcode::SET_1_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.set_bit(1, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SET_1_A => {
                self.registers.a = self.set_bit(1, self.registers.a);
                2
            }
            Opcode::SET_2_B => {
                self.registers.b = self.set_bit(2, self.registers.b);
                2
            }
            Opcode::SET_2_C => {
                self.registers.c = self.set_bit(2, self.registers.c);
                2
            }
            Opcode::SET_2_D => {
                self.registers.d = self.set_bit(2, self.registers.d);
                2
            }
            Opcode::SET_2_E => {
                self.registers.e = self.set_bit(2, self.registers.e);
                2
            }
            Opcode::SET_2_H => {
                self.registers.h = self.set_bit(2, self.registers.h);
                2
            }
            Opcode::SET_2_L => {
                self.registers.l = self.set_bit(2, self.registers.l);
                2
            }
            Opcode::SET_2_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.set_bit(2, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SET_2_A => {
                self.registers.a = self.set_bit(2, self.registers.a);
                2
            }
            Opcode::SET_3_B => {
                self.registers.b = self.set_bit(3, self.registers.b);
                2
            }
            Opcode::SET_3_C => {
                self.registers.c = self.set_bit(3, self.registers.c);
                2
            }
            Opcode::SET_3_D => {
                self.registers.d = self.set_bit(3, self.registers.d);
                2
            }
            Opcode::SET_3_E => {
                self.registers.e = self.set_bit(3, self.registers.e);
                2
            }
            Opcode::SET_3_H => {
                self.registers.h = self.set_bit(3, self.registers.h);
                2
            }
            Opcode::SET_3_L => {
                self.registers.l = self.set_bit(3, self.registers.l);
                2
            }
            Opcode::SET_3_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.set_bit(3, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SET_3_A => {
                self.registers.a = self.set_bit(3, self.registers.a);
                2
            }
            Opcode::SET_4_B => {
                self.registers.b = self.set_bit(4, self.registers.b);
                2
            }
            Opcode::SET_4_C => {
                self.registers.c = self.set_bit(4, self.registers.c);
                2
            }
            Opcode::SET_4_D => {
                self.registers.d = self.set_bit(4, self.registers.d);
                2
            }
            Opcode::SET_4_E => {
                self.registers.e = self.set_bit(4, self.registers.e);
                2
            }
            Opcode::SET_4_H => {
                self.registers.h = self.set_bit(4, self.registers.h);
                2
            }
            Opcode::SET_4_L => {
                self.registers.l = self.set_bit(4, self.registers.l);
                2
            }
            Opcode::SET_4_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.set_bit(4, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SET_4_A => {
                self.registers.a = self.set_bit(4, self.registers.a);
                2
            }
            Opcode::SET_5_B => {
                self.registers.b = self.set_bit(5, self.registers.b);
                2
            }
            Opcode::SET_5_C => {
                self.registers.c = self.set_bit(5, self.registers.c);
                2
            }
            Opcode::SET_5_D => {
                self.registers.d = self.set_bit(5, self.registers.d);
                2
            }
            Opcode::SET_5_E => {
                self.registers.e = self.set_bit(5, self.registers.e);
                2
            }
            Opcode::SET_5_H => {
                self.registers.h = self.set_bit(5, self.registers.h);
                2
            }
            Opcode::SET_5_L => {
                self.registers.l = self.set_bit(5, self.registers.l);
                2
            }
            Opcode::SET_5_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.set_bit(5, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SET_5_A => {
                self.registers.a = self.set_bit(5, self.registers.a);
                2
            }
            Opcode::SET_6_B => {
                self.registers.b = self.set_bit(6, self.registers.b);
                2
            }
            Opcode::SET_6_C => {
                self.registers.c = self.set_bit(6, self.registers.c);
                2
            }
            Opcode::SET_6_D => {
                self.registers.d = self.set_bit(6, self.registers.d);
                2
            }
            Opcode::SET_6_E => {
                self.registers.e = self.set_bit(6, self.registers.e);
                2
            }
            Opcode::SET_6_H => {
                self.registers.h = self.set_bit(6, self.registers.h);
                2
            }
            Opcode::SET_6_L => {
                self.registers.l = self.set_bit(6, self.registers.l);
                2
            }
            Opcode::SET_6_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.set_bit(6, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SET_6_A => {
                self.registers.a = self.set_bit(6, self.registers.a);
                2
            }
            Opcode::SET_7_B => {
                self.registers.b = self.set_bit(7, self.registers.b);
                2
            }
            Opcode::SET_7_C => {
                self.registers.c = self.set_bit(7, self.registers.c);
                2
            }
            Opcode::SET_7_D => {
                self.registers.d = self.set_bit(7, self.registers.d);
                2
            }
            Opcode::SET_7_E => {
                self.registers.e = self.set_bit(7, self.registers.e);
                2
            }
            Opcode::SET_7_H => {
                self.registers.h = self.set_bit(7, self.registers.h);
                2
            }
            Opcode::SET_7_L => {
                self.registers.l = self.set_bit(7, self.registers.l);
                2
            }
            Opcode::SET_7_rHL => {
                let value = self.read_memory(self.registers.hl());
                let updated = self.set_bit(7, value);
                self.write_memory(self.registers.hl(), updated);
                4
            }
            Opcode::SET_7_A => {
                self.registers.a = self.set_bit(7, self.registers.a);
                2
            }
            _ => panic!("Unexpected opcode: {:#?}", opcode)
        }
    }
}