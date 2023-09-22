//! Blinks the LED on a Pico board
//!
//! This will blink an LED attached to GP25, which is the pin the Pico uses for the on-board LED.
#![no_std]
#![no_main]

use core::cell::RefCell;
use critical_section::Mutex;
use defmt::*;
use defmt_rtt as _;
use fugit::MicrosDurationU32;
use panic_probe as _;
use rp2040_hal::{
    clocks::init_clocks_and_plls, pac, pac::interrupt, timer::Alarm, timer::Timer,
    watchdog::Watchdog,
};
use rp_pico::entry;

// (high/low)
type TwoNoteState = (bool,);
static mut MODULE_STATE: Mutex<RefCell<TwoNoteState>> = Mutex::new(RefCell::new((false,)));

type NoteAlarm = rp2040_hal::timer::Alarm0;
static mut NOTE_ALARM: Mutex<RefCell<Option<NoteAlarm>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    info!("Program start");
    let pac = pac::Peripherals::take().unwrap();
    let mut resets = pac.RESETS;
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
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

    let mut timer = Timer::new(pac.TIMER, &mut resets, &clocks);

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    }

    critical_section::with(|cs| {
        let mut note_alarm = timer.alarm_0().unwrap();
        let _ = note_alarm.schedule(MicrosDurationU32::millis(500));
        note_alarm.enable_interrupt();

        unsafe {
            NOTE_ALARM.borrow(cs).replace(Some(note_alarm));
        }
    });

    loop {}
}

#[interrupt]
fn TIMER_IRQ_0() {
    critical_section::with(|cs| {
        let mut alarm = unsafe { NOTE_ALARM.borrow(cs).take().unwrap() };
        let (mut hilo,) = unsafe { MODULE_STATE.borrow(cs).take() };
        hilo = !hilo;
        if hilo {
            info!("bing")
        } else {
            info!("bong")
        }
        alarm.clear_interrupt();
        alarm.schedule(MicrosDurationU32::millis(500));
        unsafe { NOTE_ALARM.borrow(cs).replace(Some(alarm)) };
        unsafe { MODULE_STATE.borrow(cs).replace((hilo,)) };
    });
}
