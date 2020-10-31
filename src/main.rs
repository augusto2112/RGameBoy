mod register;
mod cpu;
mod opcode;
mod memory;
mod interupt;
mod timer;

use cpu::CPU;

fn main() {
    let rom = std::fs::read("rom").unwrap();
    let mut cpu = CPU::new();
    cpu.load_rom(&rom);
    cpu.memory.stub();
    loop {
        cpu.tick();
    }
}
