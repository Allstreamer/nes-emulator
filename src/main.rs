mod cpu;
use cpu::Cpu;

fn main() {
    let raw_rom = include_bytes!("../TestRoms/5_Instructions1.nes");
    let mut cpu = Cpu::new();
    cpu.reset(raw_rom);

    while !cpu.halted {
        cpu.step();
    }
}
