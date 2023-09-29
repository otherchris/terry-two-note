//! Blinks the LED on a Pico board
//!
//! This will blink an LED attached to GP25, which is the pin the Pico uses for the on-board LED.
#![no_std]
#![no_main]

use core::cell::RefCell;
use core::f32;
use critical_section::Mutex;
use defmt::*;
use defmt_rtt as _;
use fugit::{MicrosDurationU32, RateExtU32};
use mcp4725::*;
use panic_probe as _;
use rp2040_hal::{
    clocks::init_clocks_and_plls, gpio::bank0::Gpio16, gpio::bank0::Gpio17, gpio::Function,
    gpio::Pin, gpio::Pins, i2c::Controller, pac, pac::interrupt, timer::Alarm, timer::Timer,
    watchdog::Watchdog, Clock, Sio, I2C,
};
use rp_pico::entry;
mod types;
use types::I2CType;

// enum PhaseFlag {
//     HIGH = 1,
//     DOWN = 2,
//     LOW = 3,
//     UP = 4,
// }
static mut PHASE_FLAG: Mutex<RefCell<Option<u32>>> = Mutex::new(RefCell::new(Some(1)));
static mut ELAPSED: Mutex<RefCell<Option<u32>>> = Mutex::new(RefCell::new(Some(0)));

type NoteAlarm = rp2040_hal::timer::Alarm0;
static mut NOTE_ALARM: Mutex<RefCell<Option<NoteAlarm>>> = Mutex::new(RefCell::new(None));

type DACType = MCP4725<I2CType>;
static mut DAC: Mutex<RefCell<Option<DACType>>> = Mutex::new(RefCell::new(None));

const HIGH_VAL: u16 = 0x0599;
const LOW_VAL: u16 = 0x0200;
const VAL_DIFF: u16 = HIGH_VAL - LOW_VAL;
const NOTE_LENGTH_US: u32 = 500_000;
const HOLD_LENGTH_US: u32 = 950_000;
const DECAY_LENGTH_US: u32 = 100_000;
const TIC_LENGTH_US: u32 = 75;

#[entry]
fn main() -> ! {
    info!("Program start");
    let pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut resets = pac.RESETS;
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut resets,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    }

    let pins = Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut resets);
    let mut timer = Timer::new(pac.TIMER, &mut resets, &clocks);
    let scl = pins.gpio17.into_function();
    let sda = pins.gpio16.into_function();
    let i2c = I2C::i2c0(pac.I2C0, sda, scl, 400.kHz(), &mut resets, 125_000_000.Hz());
    let dac = MCP4725::new(i2c, 0b010);

    critical_section::with(|cs| {
        let mut note_alarm = timer.alarm_0().unwrap();
        let _ = note_alarm.schedule(MicrosDurationU32::micros(NOTE_LENGTH_US));
        note_alarm.enable_interrupt();

        unsafe {
            NOTE_ALARM.borrow(cs).replace(Some(note_alarm));
            DAC.borrow(cs).replace(Some(dac));
        }
    });

    loop {}
}

#[interrupt]
fn TIMER_IRQ_0() {
    critical_section::with(|cs| {
        let mut dac = unsafe { DAC.borrow(cs).take().unwrap() };

        let mut alarm = unsafe { NOTE_ALARM.borrow(cs).take().unwrap() };
        alarm.clear_interrupt();

        let mut phase_flag = unsafe { PHASE_FLAG.borrow(cs).take().unwrap() };
        let mut elapsed = unsafe { ELAPSED.borrow(cs).take().unwrap() };

        let mut next_flag = phase_flag;

        match phase_flag {
            1 => {
                next_flag = 2;
                alarm.schedule(MicrosDurationU32::micros(HOLD_LENGTH_US));
                dac.set_dac(PowerDown::Normal, HIGH_VAL);
                elapsed = 0;
                info!("HIGH")
            }
            2 => {
                elapsed += TIC_LENGTH_US;
                alarm.schedule(MicrosDurationU32::micros(TIC_LENGTH_US));
                dac.set_dac(
                    PowerDown::Normal,
                    HIGH_VAL - (VAL_DIFF as u32 * elapsed / DECAY_LENGTH_US) as u16,
                );
                if elapsed >= DECAY_LENGTH_US {
                    next_flag = 3;
                }
            }
            3 => {
                next_flag = 4;
                alarm.schedule(MicrosDurationU32::micros(HOLD_LENGTH_US));
                dac.set_dac(PowerDown::Normal, LOW_VAL);
                elapsed = 0;
                info!("LOW")
            }
            4 => {
                elapsed += TIC_LENGTH_US;
                alarm.schedule(MicrosDurationU32::micros(TIC_LENGTH_US));
                dac.set_dac(
                    PowerDown::Normal,
                    LOW_VAL + (VAL_DIFF as u32 * elapsed / DECAY_LENGTH_US) as u16,
                );
                if elapsed >= DECAY_LENGTH_US {
                    next_flag = 1;
                }
            }
            _ => {}
        }
        unsafe { NOTE_ALARM.borrow(cs).replace(Some(alarm)) };
        unsafe { DAC.borrow(cs).replace(Some(dac)) };
        unsafe { PHASE_FLAG.borrow(cs).replace(Some(next_flag)) };
        unsafe { ELAPSED.borrow(cs).replace(Some(elapsed)) };
    });
}
