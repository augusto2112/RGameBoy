mod register;
mod cpu;
mod opcode;
mod mmu;
mod interupt;
mod timer;

use cpu::CPU;

fn main() {
    let rom = std::fs::read("rom").unwrap();
    let mut cpu = CPU::new();
    cpu.load_rom(&rom);
    cpu.mmu.stub();
    loop {
        cpu.tick();
    }
}
